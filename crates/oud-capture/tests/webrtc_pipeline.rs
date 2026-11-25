//! Integration and benchmark tests for `oud-capture` covering:
//! - Host-side `SessionGrant` frame gating (`v0.1` Asks-First bounds)
//! - P2P default + Cloudflare TURN / `coturn` fallback ICE configs
//! - Hardware H.264 discovery + automatic `OpenH264` software fallback
//! - Live Windows capture probe (`#[cfg(target_os = "windows")]`)
//! - Real `webrtc-rs` 1080p Auto loopback latency & Allow-to-first-frame SLOs

use std::time::{Duration, Instant};

use chrono::{Duration as ChronoDuration, Utc};
use oud_capture::{
    chunk_file_bytes, discover_hardware_h264_adapters, AdaptiveH264Pipeline, CaptureError,
    EncodedVideoFrame, EncoderKind, HostFrameGatekeeper, HostInboundMessage, HostMediaPeer,
    IceConfig, ProtectedSurfaceKind, QualityPreset, RawFrame, Rect, ScreenCapturer,
    SessionTelemetry, SignedSdpDescription, SoftwareH264Encoder, SyntheticFrameCapturer,
    TransportMode, TurnFallbackConfig, VerifiedPeerContext, VideoEncoder, ViewerMediaPeer,
};
use oud_protocol::audit::{verify_chain, AuditChain, AuditEvent, AuditEventKind};
use oud_protocol::grant::{
    ClipboardDirection, FrameKind, GrantDuration, GrantPermissions, SessionGrant,
};
use oud_protocol::handshake::{InitiatorPending, ResponderPending};
use oud_protocol::identity::IdentityKeyPair;
use tokio::time::{sleep, timeout};
use uuid::Uuid;
use webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState;

struct FailingHardwareEncoder {
    fail_with_no_hw: bool,
    kind: EncoderKind,
}

impl FailingHardwareEncoder {
    fn new(fail_with_no_hw: bool) -> Self {
        Self {
            fail_with_no_hw,
            kind: EncoderKind::HardwareH264Mft {
                adapter_name: "Test-NVENC-MFT".to_owned(),
            },
        }
    }
}

impl VideoEncoder for FailingHardwareEncoder {
    fn kind(&self) -> &EncoderKind {
        &self.kind
    }

    fn force_keyframe(&mut self) {}

    fn encode(
        &mut self,
        _frame: &RawFrame,
        _preset: QualityPreset,
        _rtt_ms: f64,
        _loss_ratio: f64,
    ) -> Result<EncodedVideoFrame, CaptureError> {
        if self.fail_with_no_hw {
            Err(CaptureError::NoHardwareEncoder(
                "hardware MFT surface unavailable".to_owned(),
            ))
        } else {
            Err(CaptureError::EncoderFailed(
                "simulated MFT encode session failure".to_owned(),
            ))
        }
    }
}

fn contains_annex_b_start_code(nalus: &[u8]) -> bool {
    nalus.windows(4).any(|w| w == [0, 0, 0, 1]) || nalus.windows(3).any(|w| w == [0, 0, 1])
}

#[test]
fn gatekeeper_enforces_session_grant_v01_bounds_and_mid_session_revocation() {
    let host = IdentityKeyPair::generate();
    let viewer = IdentityKeyPair::generate().public_key();
    let session_id = Uuid::now_v7();
    let now = Utc::now();

    let initial_grant = SessionGrant::issue(
        &host,
        session_id,
        viewer,
        "support.example.org",
        GrantPermissions::default(),
        GrantDuration::OneHour,
        now,
    )
    .expect("valid v0.1 screen-only grant");

    let mut gatekeeper = HostFrameGatekeeper::from_verified_grant(
        initial_grant.clone(),
        host.public_key(),
        session_id,
        viewer,
        "support.example.org",
        now,
    )
    .expect("gatekeeper activates verified grant");

    assert!(gatekeeper
        .authorize_frame(FrameKind::ScreenView, now)
        .is_ok());
    assert!(matches!(
        gatekeeper.authorize_frame(FrameKind::MouseKeyboard, now),
        Err(CaptureError::FrameBlockedByGrant {
            frame_kind: "mouse_keyboard"
        })
    ));
    assert!(matches!(
        gatekeeper.authorize_frame(FrameKind::ClipboardSync, now),
        Err(CaptureError::FrameBlockedByGrant {
            frame_kind: "clipboard_sync"
        })
    ));
    assert!(matches!(
        gatekeeper.authorize_frame(FrameKind::FileTransfer, now),
        Err(CaptureError::FrameBlockedByGrant {
            frame_kind: "file_transfer"
        })
    ));
    assert!(matches!(
        gatekeeper.authorize_frame(FrameKind::BlankScreen, now),
        Err(CaptureError::FrameBlockedByGrant {
            frame_kind: "blank_screen"
        })
    ));

    // Mid-session permission grant update enables input immediately, while keeping blank_screen locked false.
    let updated_grant = initial_grant
        .reissue_with_permissions(
            &host,
            GrantPermissions {
                view_screen: true,
                mouse_keyboard: true,
                clipboard_sync: false,
                file_transfer: false,
                blank_screen: false,
            },
            now + ChronoDuration::seconds(5),
        )
        .expect("reissue succeeds");

    gatekeeper
        .update_grant(updated_grant.clone(), now + ChronoDuration::seconds(5))
        .expect("gatekeeper updates grant");
    assert!(gatekeeper
        .authorize_frame(FrameKind::MouseKeyboard, now + ChronoDuration::seconds(6))
        .is_ok());
    assert!(gatekeeper
        .authorize_frame(FrameKind::BlankScreen, now + ChronoDuration::seconds(6))
        .is_err());

    // Revoking input mid-session drops subsequent MouseKeyboard frames immediately.
    let revoked_input = updated_grant
        .reissue_with_permissions(
            &host,
            GrantPermissions::default(),
            now + ChronoDuration::seconds(10),
        )
        .expect("reissue screen-only");
    gatekeeper
        .update_grant(revoked_input, now + ChronoDuration::seconds(10))
        .expect("gatekeeper updates immediately");
    assert!(gatekeeper
        .authorize_frame(FrameKind::MouseKeyboard, now + ChronoDuration::seconds(11))
        .is_err());

    // Grant expiration fails closed.
    let after_expiry = now + ChronoDuration::hours(2);
    assert!(gatekeeper
        .authorize_frame(FrameKind::ScreenView, after_expiry)
        .is_err());
}

#[test]
fn ice_config_supports_p2p_default_cloudflare_turn_and_coturn_fallback() {
    let default_p2p = IceConfig::direct_p2p_default();
    let rtc_default = default_p2p
        .to_rtc_configuration()
        .expect("P2P default configuration builds");
    assert_eq!(rtc_default.ice_servers.len(), 1);
    assert_eq!(
        rtc_default.ice_servers[0].urls,
        vec!["stun:stun.cloudflare.com:3478"]
    );

    let cf_turn = IceConfig::p2p_with_turn_fallback(TurnFallbackConfig::cloudflare_default(
        "cf-user",
        "cf-secret",
    ));
    let rtc_cf = cf_turn
        .to_rtc_configuration()
        .expect("Cloudflare TURN fallback config builds");
    assert_eq!(rtc_cf.ice_servers.len(), 2);
    assert!(rtc_cf.ice_servers[1]
        .urls
        .iter()
        .any(|u| u.contains("turn.cloudflare.com")));

    let coturn = IceConfig::p2p_with_turn_fallback(TurnFallbackConfig::self_hosted_coturn(
        "turn.example.org",
        3478,
        Some(5349),
        "oudos",
        "coturn-pass",
    ));
    let rtc_coturn = coturn
        .to_rtc_configuration()
        .expect("coturn fallback config builds");
    assert_eq!(rtc_coturn.ice_servers[1].urls.len(), 3);

    // Invalid empty credentials fail closed.
    let invalid = IceConfig::p2p_with_turn_fallback(TurnFallbackConfig::cloudflare_default("", ""));
    assert!(matches!(
        invalid.to_rtc_configuration(),
        Err(CaptureError::InvalidIceConfig(_))
    ));
}

#[test]
fn quality_presets_and_adaptive_congestion_ladder() {
    let auto_good = QualityPreset::Auto.adapt_for_network(25.0, 0.0);
    assert_eq!(auto_good.target_width, 1920);
    assert_eq!(auto_good.target_height, 1080);
    assert_eq!(auto_good.target_fps, 30);

    let auto_congested = QualityPreset::Auto.adapt_for_network(180.0, 0.08);
    assert_eq!(auto_congested.target_width, 1280);
    assert_eq!(auto_congested.target_height, 720);
    assert_eq!(auto_congested.target_fps, 24);

    let sharp = QualityPreset::Sharp.baseline_profile();
    assert_eq!(sharp.target_fps, 60);
    assert_eq!(sharp.target_bitrate_bps, 8_000_000);

    let fast = QualityPreset::Fast.baseline_profile();
    assert_eq!(fast.target_bitrate_bps, 1_500_000);
}

#[test]
fn hardware_to_software_h264_fallback_produces_valid_annex_b_bitstream() {
    let _adapters = discover_hardware_h264_adapters();

    let mut capturer = SyntheticFrameCapturer::new(1280, 720, 30);
    let frame = capturer.capture_frame().expect("capture synthetic frame");

    for fail_with_no_hw in [true, false] {
        let software = SoftwareH264Encoder::new(QualityPreset::Auto).expect("OpenH264 init");
        let mut pipeline = AdaptiveH264Pipeline::with_primary_and_fallback(
            Some(Box::new(FailingHardwareEncoder::new(fail_with_no_hw))),
            software,
        );
        assert!(matches!(
            pipeline.active_encoder(),
            EncoderKind::HardwareH264Mft { .. }
        ));
        assert_eq!(pipeline.fallback_count(), 0);

        let pkt = pipeline
            .encode(&frame, QualityPreset::Auto, 25.0, 0.0)
            .expect("software OpenH264 fallback encodes frame");

        assert_eq!(pkt.encoder, EncoderKind::SoftwareOpenH264);
        assert_eq!(pipeline.active_encoder(), &EncoderKind::SoftwareOpenH264);
        assert_eq!(pipeline.fallback_count(), 1);
        assert!(pipeline.last_fallback_reason().is_some());
        assert!(pkt.is_keyframe);
        assert!(
            contains_annex_b_start_code(&pkt.annex_b),
            "OpenH264 must emit valid Annex B H.264 NAL units"
        );
    }
}

#[cfg(target_os = "windows")]
#[test]
fn windows_native_capture_probes_and_handles_session_state() {
    match oud_capture::create_platform_capturer() {
        Ok(mut capturer) => {
            let geom = capturer
                .probe_availability()
                .expect("Windows display geometry probed");
            assert!(geom.width > 0 && geom.height > 0);
            match capturer.capture_frame() {
                Ok(frame) => {
                    assert_eq!(frame.width, geom.width);
                    assert_eq!(frame.height, geom.height);
                    assert_ne!(frame.data, [] as [u8; 0]);
                }
                Err(CaptureError::ScreenUnavailable(_)) => {
                    // Acceptable in locked/non-interactive CI session; must return typed reason.
                }
                Err(other) => panic!("unexpected Windows capture error: {other}"),
            }
        }
        Err(CaptureError::ScreenUnavailable(_)) => {
            // Headless CI without attached display maps cleanly to ScreenUnavailableReason.
        }
        Err(other) => panic!("unexpected Windows capturer init error: {other}"),
    }
}

fn perform_authenticated_handshake(
    host_keys: &IdentityKeyPair,
    viewer_keys: &IdentityKeyPair,
    session_id: Uuid,
) -> (VerifiedPeerContext, VerifiedPeerContext) {
    let (init, offer) = InitiatorPending::initiate(session_id, "support.example.org", viewer_keys);
    let (resp, accept) = ResponderPending::respond(&offer, host_keys).expect("offer verifies");
    let (reveal, viewer_out) = init
        .complete(&accept, Some(&host_keys.public_key()))
        .expect("accept verifies");
    let host_out = resp
        .verify_reveal_and_finalize(&reveal)
        .expect("reveal verifies");
    assert_eq!(viewer_out.transcript_hash, host_out.transcript_hash);

    (
        VerifiedPeerContext {
            session_id,
            local_pubkey: host_keys.public_key(),
            remote_pubkey: viewer_keys.public_key(),
            handshake_transcript_hash: host_out.transcript_hash,
        },
        VerifiedPeerContext {
            session_id,
            local_pubkey: viewer_keys.public_key(),
            remote_pubkey: host_keys.public_key(),
            handshake_transcript_hash: viewer_out.transcript_hash,
        },
    )
}

async fn connect_loopback_peers(
    host: &HostMediaPeer,
    viewer: &ViewerMediaPeer,
    host_keys: &IdentityKeyPair,
    viewer_keys: &IdentityKeyPair,
) {
    let offer = host.create_offer(host_keys).await.expect("offer");
    let answer = viewer
        .accept_offer_and_create_answer(offer, viewer_keys)
        .await
        .expect("answer");
    host.accept_answer(answer).await.expect("accept answer");

    let connect_deadline = Instant::now() + Duration::from_secs(5);
    loop {
        for cand in host.drain_local_ice_candidates().await {
            viewer
                .add_ice_candidate(cand)
                .await
                .expect("viewer add host candidate");
        }
        for cand in viewer.drain_local_ice_candidates().await {
            host.add_ice_candidate(cand)
                .await
                .expect("host add viewer candidate");
        }
        if host.connection_state() == RTCPeerConnectionState::Connected
            && viewer.connection_state() == RTCPeerConnectionState::Connected
            && viewer.is_input_channel_ready().await
        {
            break;
        }
        assert!(
            Instant::now() < connect_deadline,
            "WebRTC P2P loopback did not connect within 5s (host={:?}, viewer={:?})",
            host.connection_state(),
            viewer.connection_state()
        );
        sleep(Duration::from_millis(15)).await;
    }
}

async fn verify_grant_gating_over_data_channel(
    host: &HostMediaPeer,
    viewer: &ViewerMediaPeer,
    host_keys: &IdentityKeyPair,
    initial_grant: &SessionGrant,
) {
    viewer
        .send_input_message(&HostInboundMessage::MouseKeyboardInput {
            seq: 1,
            action: "click".to_owned(),
        })
        .await
        .expect("send input message");
    viewer
        .send_input_message(&HostInboundMessage::BlankScreenRequest)
        .await
        .expect("send blank request");

    sleep(Duration::from_millis(60)).await;
    assert!(
        host.drain_authorized_inputs().await.is_empty(),
        "screen-only grant must drop inbound input and blank-screen frames"
    );
    assert!(
        host.gatekeeper().lock().await.dropped_frames() >= 2,
        "dropped frame counter must record blocked inbound frames"
    );

    let updated_grant = initial_grant
        .reissue_with_permissions(
            host_keys,
            GrantPermissions {
                view_screen: true,
                mouse_keyboard: true,
                clipboard_sync: false,
                file_transfer: false,
                blank_screen: false,
            },
            Utc::now(),
        )
        .expect("reissue with mouse_keyboard");
    host.gatekeeper()
        .lock()
        .await
        .update_grant(updated_grant, Utc::now())
        .expect("update grant");

    viewer
        .send_input_message(&HostInboundMessage::MouseKeyboardInput {
            seq: 2,
            action: "move".to_owned(),
        })
        .await
        .expect("send authorized input");

    let wait_input = Instant::now() + Duration::from_secs(2);
    loop {
        let items = host.drain_authorized_inputs().await;
        if !items.is_empty() {
            assert_eq!(
                items[0],
                HostInboundMessage::MouseKeyboardInput {
                    seq: 2,
                    action: "move".to_owned(),
                }
            );
            break;
        }
        assert!(
            Instant::now() < wait_input,
            "authorized input did not arrive at host"
        );
        sleep(Duration::from_millis(10)).await;
    }
}

#[tokio::test]
async fn webrtc_loopback_1080p_auto_mode_meets_slos_and_enforces_grant_gating() {
    let host_keys = IdentityKeyPair::generate();
    let viewer_keys = IdentityKeyPair::generate();
    let session_id = Uuid::now_v7();
    let now = Utc::now();

    let (host_ctx, viewer_ctx) =
        perform_authenticated_handshake(&host_keys, &viewer_keys, session_id);

    let grant = SessionGrant::issue(
        &host_keys,
        session_id,
        viewer_keys.public_key(),
        "support.example.org",
        GrantPermissions::default(),
        GrantDuration::ThisSession,
        now,
    )
    .expect("screen-only grant issued");

    let gatekeeper = HostFrameGatekeeper::from_verified_grant(
        grant.clone(),
        host_keys.public_key(),
        session_id,
        viewer_keys.public_key(),
        "support.example.org",
        now,
    )
    .expect("gatekeeper verified");

    let pipeline = AdaptiveH264Pipeline::new(QualityPreset::Auto).expect("H.264 pipeline");
    let ice = IceConfig::local_loopback();

    let host = Box::pin(HostMediaPeer::new(&ice, gatekeeper, pipeline, host_ctx))
        .await
        .expect("host peer");
    let viewer = Box::pin(ViewerMediaPeer::new(&ice, viewer_ctx))
        .await
        .expect("viewer peer");

    connect_loopback_peers(&host, &viewer, &host_keys, &viewer_keys).await;
    verify_grant_gating_over_data_channel(&host, &viewer, &host_keys, &grant).await;

    let grant_signed_at = Instant::now();
    let (active_encoder, fallback_count) = {
        let pipe = host.pipeline().lock().await;
        (pipe.active_encoder().clone(), pipe.fallback_count())
    };
    let mut telemetry = SessionTelemetry::start(
        QualityPreset::Auto,
        active_encoder.clone(),
        TransportMode::DirectP2p,
        grant_signed_at,
    );
    telemetry.record_encoder_state(active_encoder, fallback_count);

    let mut capturer = SyntheticFrameCapturer::new(1920, 1080, 30);
    for idx in 0..30 {
        let raw = capturer.capture_frame().expect("capture 1080p frame");
        let send_started = raw.captured_at;
        let encoded = host
            .send_video_frame(&raw, QualityPreset::Auto, 25.0, 0.0, Utc::now())
            .await
            .expect("encode and send H.264 sample");
        assert_eq!(encoded.width, 1920);
        assert_eq!(encoded.height, 1080);

        let delivered = timeout(Duration::from_secs(2), viewer.recv_complete_video_frame())
            .await
            .expect("RTP video frame timeout")
            .expect("RTP video frame received");
        assert!(delivered.total_bytes > 0);
        if idx == 0 {
            let allow_ms = delivered
                .delivered_at
                .saturating_duration_since(grant_signed_at)
                .as_secs_f64()
                * 1000.0;
            assert!(
                allow_ms <= 3000.0,
                "Allow-to-first-frame exceeded 3s: {allow_ms:.2} ms"
            );
        }
        telemetry.record_delivered_frame(
            delivered
                .delivered_at
                .saturating_duration_since(send_started),
            delivered.delivered_at,
        );
    }

    let dropped = host.gatekeeper().lock().await.dropped_frames();
    let report = telemetry.snapshot(dropped);
    eprintln!(
        "WebRTC 1080p Auto benchmark: first_frame={:?} ms, p50={:.2} ms, p95={:.2} ms, fps={:.1}",
        report.allow_to_first_frame_ms,
        report.p50_latency_ms,
        report.p95_latency_ms,
        report.measured_fps
    );
    assert!(
        report.meets_v01_slos(),
        "v0.1 benchmark SLO failed: {report:?}"
    );

    host.close().await.expect("close host");
    viewer.close().await.expect("close viewer");
}

fn swap_fingerprint_in_sdp(sdp: &str, forged_fp: &str) -> String {
    sdp.lines()
        .map(|line| {
            if line.trim_start().starts_with("a=fingerprint:") {
                format!("a=fingerprint:sha-256 {forged_fp}")
            } else {
                line.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\r\n")
        + "\r\n"
}

#[tokio::test]
async fn tampered_or_swapped_sdp_fingerprint_is_rejected() {
    let host_keys = IdentityKeyPair::generate();
    let viewer_keys = IdentityKeyPair::generate();
    let attacker_keys = IdentityKeyPair::generate();
    let session_id = Uuid::now_v7();
    let now = Utc::now();

    let (host_ctx, viewer_ctx) =
        perform_authenticated_handshake(&host_keys, &viewer_keys, session_id);

    let grant = SessionGrant::issue(
        &host_keys,
        session_id,
        viewer_keys.public_key(),
        "support.example.org",
        GrantPermissions::default(),
        GrantDuration::ThisSession,
        now,
    )
    .expect("grant");
    let gk = HostFrameGatekeeper::from_verified_grant(
        grant,
        host_keys.public_key(),
        session_id,
        viewer_keys.public_key(),
        "support.example.org",
        now,
    )
    .expect("gatekeeper");

    let ice = IceConfig::local_loopback();
    let host = Box::pin(HostMediaPeer::new(
        &ice,
        gk,
        AdaptiveH264Pipeline::new(QualityPreset::Auto).expect("pipeline"),
        host_ctx,
    ))
    .await
    .expect("host");
    let viewer = Box::pin(ViewerMediaPeer::new(&ice, viewer_ctx))
        .await
        .expect("viewer");

    let attacker_fp =
        "aa:bb:cc:dd:ee:ff:00:11:22:33:44:55:66:77:88:99:aa:bb:cc:dd:ee:ff:00:11:22:33:44:55:66:77:88:99";

    // 1. Malicious signaling swaps SDP offer `a=fingerprint` before viewer `set_remote_description`.
    let valid_offer = host.create_offer(&host_keys).await.expect("host offer");
    let mut swapped_offer: SignedSdpDescription = valid_offer.clone();
    swapped_offer.description.sdp =
        swap_fingerprint_in_sdp(&valid_offer.description.sdp, attacker_fp);
    let viewer_err = viewer
        .accept_offer_and_create_answer(swapped_offer, &viewer_keys)
        .await
        .expect_err("viewer must reject MITM swapped SDP offer fingerprint");
    assert!(matches!(
        viewer_err,
        CaptureError::DtlsFingerprintMismatch(_)
    ));

    // 2. Malicious signaling replaces binding with attacker's Ed25519 signature over swapped fingerprint.
    let mut rogue_signed_offer = valid_offer.clone();
    rogue_signed_offer.description.sdp =
        swap_fingerprint_in_sdp(&valid_offer.description.sdp, attacker_fp);
    rogue_signed_offer.fingerprint_binding.signature = attacker_keys.sign(b"forged");
    rogue_signed_offer.fingerprint_binding.fingerprint = attacker_fp.to_owned();
    assert!(matches!(
        viewer
            .accept_offer_and_create_answer(rogue_signed_offer, &viewer_keys)
            .await,
        Err(CaptureError::DtlsFingerprintMismatch(_))
    ));

    // 3. Duplicate / injected conflicting `a=fingerprint` line in SDP must fail closed.
    let mut multi_fp_offer = valid_offer.clone();
    multi_fp_offer
        .description
        .sdp
        .push_str("a=fingerprint:sha-256 ");
    multi_fp_offer.description.sdp.push_str(attacker_fp);
    multi_fp_offer.description.sdp.push_str("\r\n");
    assert!(matches!(
        viewer
            .accept_offer_and_create_answer(multi_fp_offer, &viewer_keys)
            .await,
        Err(CaptureError::DtlsFingerprintMismatch(_))
    ));

    // 4. Valid offer accepted by viewer; now malicious signaling swaps answer `a=fingerprint`.
    let valid_answer = viewer
        .accept_offer_and_create_answer(valid_offer, &viewer_keys)
        .await
        .expect("viewer accepts valid signed offer");
    let mut swapped_answer = valid_answer.clone();
    swapped_answer.description.sdp =
        swap_fingerprint_in_sdp(&valid_answer.description.sdp, attacker_fp);
    let host_err = host
        .accept_answer(swapped_answer)
        .await
        .expect_err("host must reject MITM swapped SDP answer fingerprint");
    assert!(matches!(host_err, CaptureError::DtlsFingerprintMismatch(_)));

    // 5. Cross-session / cross-handshake replay of signed answer binding must fail closed.
    let mut replayed_answer = valid_answer;
    replayed_answer
        .fingerprint_binding
        .handshake_transcript_hash[0] ^= 0xFF;
    assert!(matches!(
        host.accept_answer(replayed_answer).await,
        Err(CaptureError::DtlsFingerprintMismatch(_))
    ));

    host.close().await.expect("close host");
    viewer.close().await.expect("close viewer");
}

#[tokio::test]
async fn malformed_inbound_payload_increments_rejected_counters() {
    let host_keys = IdentityKeyPair::generate();
    let viewer_keys = IdentityKeyPair::generate();
    let session_id = Uuid::now_v7();
    let now = Utc::now();

    let (host_ctx, _viewer_ctx) =
        perform_authenticated_handshake(&host_keys, &viewer_keys, session_id);
    let grant = SessionGrant::issue(
        &host_keys,
        session_id,
        viewer_keys.public_key(),
        "support.example.org",
        GrantPermissions {
            view_screen: true,
            mouse_keyboard: true,
            ..GrantPermissions::default()
        },
        GrantDuration::ThisSession,
        now,
    )
    .expect("grant");
    let gk = HostFrameGatekeeper::from_verified_grant(
        grant,
        host_keys.public_key(),
        session_id,
        viewer_keys.public_key(),
        "support.example.org",
        now,
    )
    .expect("gatekeeper");
    let host = Box::pin(HostMediaPeer::new(
        &IceConfig::local_loopback(),
        gk,
        AdaptiveH264Pipeline::new(QualityPreset::Auto).expect("pipeline"),
        host_ctx,
    ))
    .await
    .expect("host");

    let err = host
        .process_raw_inbound_bytes(b"corrupt-non-json", now)
        .await
        .expect_err("malformed input rejected");
    assert!(matches!(
        err,
        CaptureError::FrameBlockedByGrant {
            frame_kind: "malformed_input"
        }
    ));

    let gk_guard = host.gatekeeper().lock().await;
    assert_eq!(gk_guard.dropped_frames(), 1);
    assert_eq!(gk_guard.rejected_malformed_payloads(), 1);
    drop(gk_guard);
    assert!(
        host.drain_authorized_inputs().await.is_empty(),
        "malformed payload must never enter authorized inputs"
    );

    host.close().await.expect("close host");
}

async fn setup_protected_loopback_peers(
    host_keys: &IdentityKeyPair,
    viewer_keys: &IdentityKeyPair,
    session_id: Uuid,
) -> (HostMediaPeer, ViewerMediaPeer) {
    let now = Utc::now();
    let (host_ctx, viewer_ctx) =
        perform_authenticated_handshake(host_keys, viewer_keys, session_id);
    let grant = SessionGrant::issue(
        host_keys,
        session_id,
        viewer_keys.public_key(),
        "support.example.org",
        GrantPermissions {
            view_screen: true,
            mouse_keyboard: true,
            clipboard_sync: false,
            file_transfer: false,
            blank_screen: false,
        },
        GrantDuration::ThisSession,
        now,
    )
    .expect("grant");
    let gk = HostFrameGatekeeper::from_verified_grant(
        grant,
        host_keys.public_key(),
        session_id,
        viewer_keys.public_key(),
        "support.example.org",
        now,
    )
    .expect("gatekeeper");
    let ice = IceConfig::local_loopback();
    let host = Box::pin(HostMediaPeer::new(
        &ice,
        gk,
        AdaptiveH264Pipeline::new(QualityPreset::Auto).expect("pipeline"),
        host_ctx,
    ))
    .await
    .expect("host");
    let viewer = Box::pin(ViewerMediaPeer::new(&ice, viewer_ctx))
        .await
        .expect("viewer");

    host.protected_surfaces().set_surface(
        ProtectedSurfaceKind::ConsentPrompt,
        Rect {
            x: 400,
            y: 120,
            width: 520,
            height: 660,
        },
        None,
    );
    host.protected_surfaces().set_surface(
        ProtectedSurfaceKind::SessionBanner,
        Rect {
            x: 500,
            y: 12,
            width: 460,
            height: 44,
        },
        None,
    );
    connect_loopback_peers(&host, &viewer, host_keys, viewer_keys).await;
    (host, viewer)
}

#[tokio::test]
async fn remote_pointer_cannot_click_consent_allow_or_dismiss_banner_over_webrtc() {
    let host_keys = IdentityKeyPair::generate();
    let viewer_keys = IdentityKeyPair::generate();
    let session_id = Uuid::now_v7();
    let (host, viewer) = setup_protected_loopback_peers(&host_keys, &viewer_keys, session_id).await;

    // 1. Remote click targeting Allow button inside 520x660 consent card -> blocked
    viewer
        .send_input_message(&HostInboundMessage::PointerAction {
            seq: 10,
            action: "click".to_owned(),
            x: 780,
            y: 730,
        })
        .await
        .expect("send consent click");

    // 2. Remote click targeting End session button inside top-most banner -> blocked
    viewer
        .send_input_message(&HostInboundMessage::PointerAction {
            seq: 11,
            action: "click".to_owned(),
            x: 910,
            y: 34,
        })
        .await
        .expect("send banner click");

    // 3. Unchecked clipboard & file transfer frames -> dropped at protocol gatekeeper
    viewer
        .send_input_message(&HostInboundMessage::ClipboardText {
            text: "secret".to_owned(),
        })
        .await
        .expect("send unchecked clipboard");
    viewer
        .send_input_message(&HostInboundMessage::FileChunk {
            transfer_id: "t-1".to_owned(),
            bytes_len: 128,
        })
        .await
        .expect("send unchecked file chunk");

    // 4. Remote click outside protected rects -> allowed through
    viewer
        .send_input_message(&HostInboundMessage::PointerAction {
            seq: 12,
            action: "click".to_owned(),
            x: 120,
            y: 120,
        })
        .await
        .expect("send safe desktop click");

    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let authorized = host.drain_authorized_inputs().await;
        if !authorized.is_empty() && host.protected_surfaces().total_blocked() >= 2 {
            assert_eq!(
                authorized,
                vec![HostInboundMessage::PointerAction {
                    seq: 12,
                    action: "click".to_owned(),
                    x: 120,
                    y: 120,
                }]
            );
            break;
        }
        assert!(
            Instant::now() < deadline,
            "expected safe pointer delivery and protected-rect rejections"
        );
        sleep(Duration::from_millis(15)).await;
    }

    assert!(host.gatekeeper().lock().await.dropped_frames() >= 4);

    let mut audit_chain = AuditChain::new();
    let logged = host
        .protected_surfaces()
        .flush_to_audit_chain(
            &mut audit_chain,
            &host_keys,
            session_id,
            viewer_keys.public_key(),
            "support.example.org",
        )
        .expect("flush audit chain");
    assert_eq!(logged, 2);
    assert!(verify_chain(audit_chain.entries(), &host_keys.public_key()).is_ok());
    assert!(audit_chain
        .entries()
        .iter()
        .all(|e| matches!(e.event.kind, AuditEventKind::SyntheticInputBlocked { .. })));

    host.close().await.expect("close host");
    viewer.close().await.expect("close viewer");
}

#[tokio::test]
async fn webrtc_clipboard_and_chunked_file_transfer_fail_closed_unless_granted() {
    let host_keys = IdentityKeyPair::generate();
    let viewer_keys = IdentityKeyPair::generate();
    let session_id = Uuid::now_v7();
    let now = Utc::now();

    let (host_ctx, viewer_ctx) =
        perform_authenticated_handshake(&host_keys, &viewer_keys, session_id);
    let grant = SessionGrant::issue(
        &host_keys,
        session_id,
        viewer_keys.public_key(),
        "support.example.org",
        GrantPermissions::default(),
        GrantDuration::OneHour,
        now,
    )
    .expect("screen-only grant");

    let gk = HostFrameGatekeeper::from_verified_grant(
        grant.clone(),
        host_keys.public_key(),
        session_id,
        viewer_keys.public_key(),
        "support.example.org",
        now,
    )
    .expect("gatekeeper");
    let pipeline = AdaptiveH264Pipeline::new(QualityPreset::Auto).expect("pipeline");
    let ice = IceConfig::local_loopback();

    let host = Box::pin(HostMediaPeer::new(&ice, gk, pipeline, host_ctx))
        .await
        .expect("host");
    let viewer = Box::pin(ViewerMediaPeer::new(&ice, viewer_ctx))
        .await
        .expect("viewer");
    connect_loopback_peers(&host, &viewer, &host_keys, &viewer_keys).await;

    // 1. Default screen-only grant drops clipboard and file transfer frames.
    viewer
        .send_input_message(&HostInboundMessage::ClipboardText {
            text: "ungranted clipboard".to_owned(),
        })
        .await
        .expect("send clipboard");
    viewer
        .send_input_message(&HostInboundMessage::FileTransferStart {
            transfer_id: "tx-blocked".to_owned(),
            filename: "notes.txt".to_owned(),
            total_bytes: 12,
        })
        .await
        .expect("send file start");

    let drop_deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if host.gatekeeper().lock().await.dropped_frames() >= 2 {
            break;
        }
        assert!(Instant::now() < drop_deadline, "expected dropped frames");
        sleep(Duration::from_millis(15)).await;
    }
    assert_eq!(
        host.drain_authorized_inputs().await,
        [] as [HostInboundMessage; 0]
    );

    verify_granted_clipboard_and_file_transfer(
        &host,
        &viewer,
        &host_keys,
        &viewer_keys,
        session_id,
        &grant,
    )
    .await;

    host.close().await.expect("close host");
    viewer.close().await.expect("close viewer");
}

async fn verify_granted_clipboard_and_file_transfer(
    host: &HostMediaPeer,
    viewer: &ViewerMediaPeer,
    host_keys: &IdentityKeyPair,
    viewer_keys: &IdentityKeyPair,
    session_id: Uuid,
    grant: &SessionGrant,
) {
    let updated_grant = grant
        .reissue_with_permissions(
            host_keys,
            GrantPermissions {
                view_screen: true,
                mouse_keyboard: false,
                clipboard_sync: true,
                file_transfer: true,
                blank_screen: false,
            },
            Utc::now(),
        )
        .expect("reissue");
    host.gatekeeper()
        .lock()
        .await
        .update_grant(updated_grant, Utc::now())
        .expect("update grant");
    host.set_clipboard_direction(ClipboardDirection::ViewerToHost)
        .await;

    viewer
        .send_input_message(&HostInboundMessage::ClipboardText {
            text: "authorized clipboard text".to_owned(),
        })
        .await
        .expect("send authorized clipboard");

    let file_bytes = b"Oudos chunked WebRTC payload verified!";
    viewer
        .send_input_message(&HostInboundMessage::FileTransferStart {
            transfer_id: "tx-ok".to_owned(),
            filename: "summary.txt".to_owned(),
            total_bytes: file_bytes.len() as u64,
        })
        .await
        .expect("send start");
    for pkt in chunk_file_bytes("tx-ok", file_bytes, 16).expect("chunks") {
        viewer
            .send_input_message(&HostInboundMessage::FileTransferData {
                transfer_id: pkt.transfer_id,
                seq: pkt.seq,
                data: pkt.data,
            })
            .await
            .expect("send chunk");
    }
    viewer
        .send_input_message(&HostInboundMessage::FileTransferComplete {
            transfer_id: "tx-ok".to_owned(),
        })
        .await
        .expect("send complete");

    let done_deadline = Instant::now() + Duration::from_secs(3);
    let completed = loop {
        let files = host.drain_completed_files().await;
        if !files.is_empty() {
            break files;
        }
        assert!(
            Instant::now() < done_deadline,
            "completed file transfer did not arrive"
        );
        sleep(Duration::from_millis(15)).await;
    };
    assert_eq!(completed.len(), 1);
    assert_eq!(completed[0].filename, "summary.txt");
    assert_eq!(completed[0].payload, file_bytes);

    let mut audit = AuditChain::new();
    audit
        .append(
            AuditEvent {
                event_id: Uuid::now_v7(),
                timestamp_utc: Utc::now(),
                session_id,
                device_pubkey: host_keys.public_key(),
                actor_pubkey: viewer_keys.public_key(),
                server_domain: "support.example.org".to_owned(),
                kind: completed[0].audit_metadata.clone(),
            },
            host_keys,
        )
        .expect("append file metadata");
    let serialized = serde_json::to_string(audit.entries()).expect("json");
    assert!(serialized.contains("summary.txt"));
    assert!(!serialized.contains("Oudos chunked WebRTC payload verified!"));
}
