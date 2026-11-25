//! Real `webrtc-rs` peer connection with H.264 RTP video track, low-latency
//! input data channel, host-side `SessionGrant` gatekeeper enforcement, and
//! Ed25519-signed DTLS certificate fingerprint binding against signaling MITM.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration as StdDuration, Instant, SystemTime};

use bytes::Bytes;
use chrono::{DateTime, Utc};
use oud_protocol::grant::FrameKind;
use oud_protocol::identity::{IdentityKeyPair, IdentityPublicKey, IdentitySignature};
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, Mutex};
use uuid::Uuid;
use webrtc::api::interceptor_registry::register_default_interceptors;
use webrtc::api::media_engine::{MediaEngine, MIME_TYPE_H264};
use webrtc::api::APIBuilder;
use webrtc::data_channel::data_channel_init::RTCDataChannelInit;
use webrtc::data_channel::data_channel_message::DataChannelMessage;
use webrtc::data_channel::RTCDataChannel;
use webrtc::dtls::crypto::Certificate as DtlsCertificate;
use webrtc::ice_transport::ice_candidate::RTCIceCandidateInit;
use webrtc::interceptor::registry::Registry;
use webrtc::media::Sample;
use webrtc::peer_connection::certificate::RTCCertificate;
use webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState;
use webrtc::peer_connection::sdp::sdp_type::RTCSdpType;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::peer_connection::RTCPeerConnection;
use webrtc::rtp_transceiver::rtp_codec::RTCRtpCodecCapability;
use webrtc::track::track_local::track_local_static_sample::TrackLocalStaticSample;
use webrtc::track::track_local::TrackLocal;

use crate::capture::RawFrame;
use crate::encoder::{AdaptiveH264Pipeline, EncodedVideoFrame};
use crate::error::CaptureError;
use crate::gatekeeper::HostFrameGatekeeper;
use crate::ice_config::{IceConfig, TransportMode};
use crate::input_isolation::ProtectedSurfaceRegistry;
use crate::preset::QualityPreset;

const DTLS_BINDING_DOMAIN: &[u8] = b"oudos-dtls-fp-v1";
const SHA256_FINGERPRINT_ALGORITHM: &str = "sha-256";
const DTLS_CERT_LIFETIME_SECS: u64 = 86_400;

/// Peer authentication and handshake context binding WebRTC DTLS certificates
/// to an active `oud_protocol` authenticated key exchange.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedPeerContext {
    /// Active session UUID (`UUIDv7`).
    pub session_id: Uuid,
    /// Local peer Ed25519 identity public key.
    pub local_pubkey: IdentityPublicKey,
    /// Expected remote peer Ed25519 identity public key.
    pub remote_pubkey: IdentityPublicKey,
    /// SHA-256 `transcript_hash` produced by the completed `oud_protocol` handshake.
    pub handshake_transcript_hash: [u8; 32],
}

/// Host- or viewer-signed DTLS certificate fingerprint binding tying WebRTC SDP
/// `a=fingerprint` attributes to `oud_protocol` identity keys and handshake transcript.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DtlsFingerprintBinding {
    /// Bound session identifier (`UUIDv7`).
    pub session_id: Uuid,
    /// Ed25519 identity public key of the peer owning `fingerprint`.
    pub signer_pubkey: IdentityPublicKey,
    /// Intended remote peer's Ed25519 identity public key (`remote_pubkey`).
    pub remote_pubkey: IdentityPublicKey,
    /// Completed `oud_protocol::HandshakeOutcome::transcript_hash`.
    pub handshake_transcript_hash: [u8; 32],
    /// Digest algorithm (strictly `"sha-256"`).
    pub algorithm: String,
    /// Lowercase colon-separated SHA-256 DTLS certificate fingerprint.
    pub fingerprint: String,
    /// Ed25519 signature covering `session_id || local_fingerprint || remote_pubkey`
    /// plus domain tag, `handshake_transcript_hash`, `signer_pubkey`, and `algorithm`.
    pub signature: IdentitySignature,
}

impl DtlsFingerprintBinding {
    /// Signs a local peer's normalized SHA-256 DTLS fingerprint for a verified session peer.
    ///
    /// # Errors
    /// Returns [`CaptureError::DtlsFingerprintMismatch`] if the fingerprint format is invalid
    /// or `signer_keys` does not match `context.local_pubkey`.
    pub fn sign(
        context: &VerifiedPeerContext,
        signer_keys: &IdentityKeyPair,
        local_fingerprint: &str,
    ) -> Result<Self, CaptureError> {
        let signer_pubkey = signer_keys.public_key();
        if signer_pubkey != context.local_pubkey {
            return Err(CaptureError::DtlsFingerprintMismatch(
                "signing identity key does not match peer local_pubkey".to_owned(),
            ));
        }
        let normalized_fp = normalize_sha256_fingerprint(local_fingerprint)?;
        let payload = binding_signing_bytes(
            context.session_id,
            &normalized_fp,
            &context.remote_pubkey,
            &context.handshake_transcript_hash,
            &signer_pubkey,
            SHA256_FINGERPRINT_ALGORITHM,
        );
        let signature = signer_keys.sign(&payload);

        Ok(Self {
            session_id: context.session_id,
            signer_pubkey,
            remote_pubkey: context.remote_pubkey,
            handshake_transcript_hash: context.handshake_transcript_hash,
            algorithm: SHA256_FINGERPRINT_ALGORITHM.to_owned(),
            fingerprint: normalized_fp,
            signature,
        })
    }

    /// Verifies signature, peer identity bindings, `session_id`, and `handshake_transcript_hash`.
    ///
    /// # Errors
    /// Fails closed with [`CaptureError::DtlsFingerprintMismatch`] on any mismatch or bad signature.
    pub fn verify(
        &self,
        expected_receiver_context: &VerifiedPeerContext,
    ) -> Result<String, CaptureError> {
        if self.session_id != expected_receiver_context.session_id {
            return Err(CaptureError::DtlsFingerprintMismatch(
                "DTLS binding session_id mismatch".to_owned(),
            ));
        }
        if self.signer_pubkey != expected_receiver_context.remote_pubkey {
            return Err(CaptureError::DtlsFingerprintMismatch(
                "DTLS binding signer_pubkey does not match expected remote peer".to_owned(),
            ));
        }
        if self.remote_pubkey != expected_receiver_context.local_pubkey {
            return Err(CaptureError::DtlsFingerprintMismatch(
                "DTLS binding target peer key does not match local identity".to_owned(),
            ));
        }
        if self.handshake_transcript_hash != expected_receiver_context.handshake_transcript_hash {
            return Err(CaptureError::DtlsFingerprintMismatch(
                "DTLS binding does not match authenticated handshake transcript".to_owned(),
            ));
        }
        if !self
            .algorithm
            .eq_ignore_ascii_case(SHA256_FINGERPRINT_ALGORITHM)
        {
            return Err(CaptureError::DtlsFingerprintMismatch(format!(
                "unsupported DTLS fingerprint algorithm `{}`",
                self.algorithm
            )));
        }

        let normalized_fp = normalize_sha256_fingerprint(&self.fingerprint)?;
        let payload = binding_signing_bytes(
            self.session_id,
            &normalized_fp,
            &self.remote_pubkey,
            &self.handshake_transcript_hash,
            &self.signer_pubkey,
            SHA256_FINGERPRINT_ALGORITHM,
        );
        expected_receiver_context
            .remote_pubkey
            .verify(&payload, &self.signature)
            .map_err(|e| CaptureError::DtlsFingerprintMismatch(e.to_string()))?;

        Ok(normalized_fp)
    }
}

/// Authenticated SDP offer or answer paired with an Ed25519 [`DtlsFingerprintBinding`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedSdpDescription {
    /// Raw WebRTC SDP session description.
    pub description: RTCSessionDescription,
    /// Peer-signed DTLS fingerprint binding verified before applying remote SDP.
    pub fingerprint_binding: DtlsFingerprintBinding,
}

/// Inbound host control/input messages sent over WebRTC data channels (`oud-input`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum HostInboundMessage {
    /// Remote pointer move/click or keyboard press.
    MouseKeyboardInput {
        /// Client monotonic sequence number.
        seq: u64,
        /// Event description (e.g. `"move"` or `"key"`).
        action: String,
    },
    /// Remote pointer event carrying screen coordinates checked against protected host surfaces.
    PointerAction {
        /// Client monotonic sequence number.
        seq: u64,
        /// Pointer event kind (`"move"`, `"click"`, `"down"`, `"up"`).
        action: String,
        /// Target X coordinate in host screen pixels.
        x: i32,
        /// Target Y coordinate in host screen pixels.
        y: i32,
    },
    /// Directional clipboard text sync payload.
    ClipboardText {
        /// UTF-8 clipboard payload.
        text: String,
    },
    /// File transfer chunk metadata + payload.
    FileChunk {
        /// Transfer UUID string.
        transfer_id: String,
        /// Byte payload.
        bytes_len: usize,
    },
    /// Screen blanking request (always rejected in `v0.1` Asks-First mode).
    BlankScreenRequest,
}

impl HostInboundMessage {
    /// Maps this message to its [`FrameKind`] for `SessionGrant` enforcement.
    #[must_use]
    pub const fn frame_kind(&self) -> FrameKind {
        match self {
            Self::MouseKeyboardInput { .. } | Self::PointerAction { .. } => {
                FrameKind::MouseKeyboard
            }
            Self::ClipboardText { .. } => FrameKind::ClipboardSync,
            Self::FileChunk { .. } => FrameKind::FileTransfer,
            Self::BlankScreenRequest => FrameKind::BlankScreen,
        }
    }
}

/// Host WebRTC peer holding the H.264 video track, pinned DTLS cert, `oud-input` data channel,
/// protected consent/banner surface registry, and `HostFrameGatekeeper`.
pub struct HostMediaPeer {
    peer: Arc<RTCPeerConnection>,
    video_track: Arc<TrackLocalStaticSample>,
    input_channel: Arc<RTCDataChannel>,
    gatekeeper: Arc<Mutex<HostFrameGatekeeper>>,
    protected_surfaces: ProtectedSurfaceRegistry,
    pipeline: Arc<Mutex<AdaptiveH264Pipeline>>,
    transport_mode: TransportMode,
    local_fingerprint: String,
    auth_context: VerifiedPeerContext,
    ice_rx: Mutex<mpsc::UnboundedReceiver<RTCIceCandidateInit>>,
    authorized_inputs: Arc<Mutex<Vec<HostInboundMessage>>>,
    unexpected_channels_rejected: Arc<AtomicU64>,
}

impl HostMediaPeer {
    /// Creates a new `webrtc-rs` host peer configured with `ice_config`, `gatekeeper`, `pipeline`,
    /// an explicit local `RTCCertificate`, and verified handshake identity context.
    ///
    /// # Errors
    /// Returns [`CaptureError::DtlsFingerprintMismatch`] if `auth_context` disagrees with
    /// `gatekeeper` bindings, or [`CaptureError`] if certificate, ICE, or WebRTC setup fails.
    pub async fn new(
        ice_config: &IceConfig,
        gatekeeper: HostFrameGatekeeper,
        pipeline: AdaptiveH264Pipeline,
        auth_context: VerifiedPeerContext,
    ) -> Result<Self, CaptureError> {
        if auth_context.session_id != gatekeeper.expected_session_id()
            || auth_context.local_pubkey != gatekeeper.expected_host_pubkey()
            || auth_context.remote_pubkey != gatekeeper.expected_viewer_pubkey()
        {
            return Err(CaptureError::DtlsFingerprintMismatch(
                "HostMediaPeer auth_context disagrees with HostFrameGatekeeper bindings".to_owned(),
            ));
        }

        let (rtc_cert, local_fingerprint) = generate_peer_certificate()?;
        let mut rtc_config = ice_config.to_rtc_configuration()?;
        rtc_config.certificates = vec![rtc_cert];

        let api = build_webrtc_api()?;
        let peer = Arc::new(
            api.new_peer_connection(rtc_config)
                .await
                .map_err(|e| CaptureError::WebRtc(e.to_string()))?,
        );

        let video_track = Arc::new(TrackLocalStaticSample::new(
            RTCRtpCodecCapability {
                mime_type: MIME_TYPE_H264.to_owned(),
                clock_rate: 90_000,
                channels: 0,
                sdp_fmtp_line:
                    "level-asymmetry-allowed=1;packetization-mode=1;profile-level-id=42001f"
                        .to_owned(),
                rtcp_feedback: vec![],
            },
            "oud-video".to_owned(),
            "oudos-host-stream".to_owned(),
        ));

        peer.add_track(Arc::clone(&video_track) as Arc<dyn TrackLocal + Send + Sync>)
            .await
            .map_err(|e| CaptureError::WebRtc(e.to_string()))?;

        let input_channel = peer
            .create_data_channel(
                "oud-input",
                Some(RTCDataChannelInit {
                    ordered: Some(false),
                    max_retransmits: Some(0),
                    ..Default::default()
                }),
            )
            .await
            .map_err(|e| CaptureError::WebRtc(e.to_string()))?;

        let gatekeeper_arc = Arc::new(Mutex::new(gatekeeper));
        let protected_surfaces = ProtectedSurfaceRegistry::new();
        let authorized_inputs = Arc::new(Mutex::new(Vec::new()));
        let unexpected_channels_rejected = Arc::new(AtomicU64::new(0));
        let ice_rx = wire_host_peer_callbacks(
            &peer,
            &input_channel,
            &gatekeeper_arc,
            &protected_surfaces,
            &authorized_inputs,
            &unexpected_channels_rejected,
        );

        let transport_mode = if ice_config.force_relay {
            TransportMode::TurnRelay
        } else {
            TransportMode::DirectP2p
        };

        Ok(Self {
            peer,
            video_track,
            input_channel,
            gatekeeper: gatekeeper_arc,
            protected_surfaces,
            pipeline: Arc::new(Mutex::new(pipeline)),
            transport_mode,
            local_fingerprint,
            auth_context,
            ice_rx: Mutex::new(ice_rx),
            authorized_inputs,
            unexpected_channels_rejected,
        })
    }

    /// Returns the host peer's pinned SHA-256 DTLS certificate fingerprint.
    #[must_use]
    pub fn local_fingerprint(&self) -> &str {
        &self.local_fingerprint
    }

    /// Creates an SDP offer paired with a host Ed25519 [`DtlsFingerprintBinding`].
    ///
    /// # Errors
    /// Returns [`CaptureError::WebRtc`] or [`CaptureError::DtlsFingerprintMismatch`] if offer
    /// generation, local SDP fingerprint self-check, or Ed25519 binding signing fails.
    pub async fn create_offer(
        &self,
        host_keys: &IdentityKeyPair,
    ) -> Result<SignedSdpDescription, CaptureError> {
        let offer = self
            .peer
            .create_offer(None)
            .await
            .map_err(|e| CaptureError::WebRtc(e.to_string()))?;
        verify_sdp_fingerprint(&offer, RTCSdpType::Offer, &self.local_fingerprint)?;
        let binding =
            DtlsFingerprintBinding::sign(&self.auth_context, host_keys, &self.local_fingerprint)?;
        self.peer
            .set_local_description(offer.clone())
            .await
            .map_err(|e| CaptureError::WebRtc(e.to_string()))?;
        Ok(SignedSdpDescription {
            description: offer,
            fingerprint_binding: binding,
        })
    }

    /// Verifies the viewer's Ed25519 [`DtlsFingerprintBinding`] and SDP `a=fingerprint`
    /// BEFORE calling `set_remote_description`.
    ///
    /// # Errors
    /// Returns [`CaptureError::DtlsFingerprintMismatch`] if the signed binding or remote
    /// SDP `a=fingerprint` is invalid, missing, duplicated, or swapped, or [`CaptureError::WebRtc`]
    /// if `set_remote_description` fails.
    pub async fn accept_answer(&self, answer: SignedSdpDescription) -> Result<(), CaptureError> {
        let verified_remote_fp = answer.fingerprint_binding.verify(&self.auth_context)?;
        verify_sdp_fingerprint(&answer.description, RTCSdpType::Answer, &verified_remote_fp)?;
        self.peer
            .set_remote_description(answer.description)
            .await
            .map_err(|e| CaptureError::WebRtc(e.to_string()))
    }

    /// Adds a remote ICE candidate received via signaling.
    ///
    /// # Errors
    /// Returns [`CaptureError::WebRtc`] if candidate addition fails.
    pub async fn add_ice_candidate(
        &self,
        candidate: RTCIceCandidateInit,
    ) -> Result<(), CaptureError> {
        self.peer
            .add_ice_candidate(candidate)
            .await
            .map_err(|e| CaptureError::WebRtc(e.to_string()))
    }

    /// Drains newly gathered local ICE candidates ready to send over signaling.
    pub async fn drain_local_ice_candidates(&self) -> Vec<RTCIceCandidateInit> {
        let mut rx = self.ice_rx.lock().await;
        let mut out = Vec::new();
        while let Ok(c) = rx.try_recv() {
            out.push(c);
        }
        out
    }

    /// Returns the current `RTCPeerConnectionState`.
    #[must_use]
    pub fn connection_state(&self) -> RTCPeerConnectionState {
        self.peer.connection_state()
    }

    /// Returns active transport mode (`DirectP2p` or `TurnRelay`).
    #[must_use]
    pub const fn transport_mode(&self) -> TransportMode {
        self.transport_mode
    }

    /// Returns shared reference to the host [`HostFrameGatekeeper`].
    #[must_use]
    pub const fn gatekeeper(&self) -> &Arc<Mutex<HostFrameGatekeeper>> {
        &self.gatekeeper
    }

    /// Returns shared reference to the protected consent/banner surface registry.
    #[must_use]
    pub const fn protected_surfaces(&self) -> &ProtectedSurfaceRegistry {
        &self.protected_surfaces
    }

    /// Returns shared reference to the adaptive H.264 pipeline.
    #[must_use]
    pub const fn pipeline(&self) -> &Arc<Mutex<AdaptiveH264Pipeline>> {
        &self.pipeline
    }

    /// Returns the number of unexpected viewer-created data channels rejected and closed.
    #[must_use]
    pub fn unexpected_channels_rejected(&self) -> u64 {
        self.unexpected_channels_rejected.load(Ordering::Relaxed)
    }

    /// Encodes and writes a captured desktop frame onto the WebRTC H.264 RTP track
    /// ONLY if [`HostFrameGatekeeper`] permits `FrameKind::ScreenView` at `now`.
    ///
    /// # Errors
    /// Returns [`CaptureError::FrameBlockedByGrant`] if no valid grant permits screen capture,
    /// or [`CaptureError::EncoderFailed`] / [`CaptureError::WebRtc`] on media errors.
    pub async fn send_video_frame(
        &self,
        frame: &RawFrame,
        preset: QualityPreset,
        rtt_ms: f64,
        loss_ratio: f64,
        now: DateTime<Utc>,
    ) -> Result<EncodedVideoFrame, CaptureError> {
        {
            let mut gk = self.gatekeeper.lock().await;
            gk.authorize_frame(FrameKind::ScreenView, now)?;
        }

        let encoded = {
            let mut pipe = self.pipeline.lock().await;
            pipe.encode(frame, preset, rtt_ms, loss_ratio)?
        };

        let sample = Sample {
            data: Bytes::from(encoded.annex_b.clone()),
            duration: encoded.duration,
            ..Default::default()
        };
        self.video_track
            .write_sample(&sample)
            .await
            .map_err(|e| CaptureError::WebRtc(e.to_string()))?;

        Ok(encoded)
    }

    /// Enforces [`HostFrameGatekeeper`] and [`ProtectedSurfaceRegistry`] synchronously on a decoded
    /// inbound message before OS dispatch.
    ///
    /// # Errors
    /// Returns [`CaptureError::FrameBlockedByGrant`] if the active grant disallows this frame,
    /// or [`CaptureError::ProtectedSurfaceInputBlocked`] if remote pointer coordinates target an
    /// active consent prompt or persistent session banner.
    pub async fn enforce_inbound_message(
        &self,
        msg: HostInboundMessage,
        now: DateTime<Utc>,
    ) -> Result<HostInboundMessage, CaptureError> {
        let mut gk = self.gatekeeper.lock().await;
        gk.authorize_frame(msg.frame_kind(), now)?;
        if let HostInboundMessage::PointerAction { x, y, .. } = &msg {
            if let Err(err) = self.protected_surfaces.enforce_remote_pointer(*x, *y, now) {
                gk.record_rejected_payload();
                return Err(err);
            }
        }
        drop(gk);
        self.authorized_inputs.lock().await.push(msg.clone());
        Ok(msg)
    }

    /// Validates, parses, and enforces [`HostFrameGatekeeper`] + [`ProtectedSurfaceRegistry`] on
    /// raw `"oud-input"` bytes, incrementing rejection counters on malformed/blocked payloads.
    ///
    /// # Errors
    /// Returns [`CaptureError::FrameBlockedByGrant`] on malformed JSON or denied frame kind,
    /// or [`CaptureError::ProtectedSurfaceInputBlocked`] on protected surface pointer targets.
    pub async fn process_raw_inbound_bytes(
        &self,
        data: &[u8],
        now: DateTime<Utc>,
    ) -> Result<HostInboundMessage, CaptureError> {
        dispatch_raw_input_bytes(
            &self.gatekeeper,
            &self.protected_surfaces,
            &self.authorized_inputs,
            data,
            now,
        )
        .await
    }

    /// Drains inbound messages that passed host [`HostFrameGatekeeper`] enforcement.
    pub async fn drain_authorized_inputs(&self) -> Vec<HostInboundMessage> {
        let mut guard = self.authorized_inputs.lock().await;
        std::mem::take(&mut *guard)
    }

    /// Closes the host peer connection and data channel cleanly.
    ///
    /// # Errors
    /// Returns [`CaptureError::WebRtc`] if close fails.
    pub async fn close(&self) -> Result<(), CaptureError> {
        let _ = self.input_channel.close().await;
        self.peer
            .close()
            .await
            .map_err(|e| CaptureError::WebRtc(e.to_string()))
    }
}

/// Delivered RTP video packet notification observed by [`ViewerMediaPeer`].
#[derive(Debug, Clone)]
pub struct ReceivedRtpFrame {
    /// RTP sequence number.
    pub sequence_number: u16,
    /// RTP timestamp.
    pub rtp_timestamp: u32,
    /// RTP marker bit (`true` on the final packet of an encoded video frame).
    pub marker: bool,
    /// Payload byte length.
    pub payload_len: usize,
    /// Monotonic arrival instant for latency telemetry.
    pub received_at: Instant,
}

/// Complete reassembled H.264 video frame delivered over RTP.
#[derive(Debug, Clone)]
pub struct DeliveredVideoFrame {
    /// RTP timestamp shared by all packets of this video frame.
    pub rtp_timestamp: u32,
    /// Number of RTP packets reassembled for this frame.
    pub packet_count: usize,
    /// Total received RTP payload bytes.
    pub total_bytes: usize,
    /// Monotonic arrival instant of the frame's marker packet.
    pub delivered_at: Instant,
}

/// Viewer WebRTC peer receiving the H.264 RTP stream and sending input data-channel frames.
pub struct ViewerMediaPeer {
    peer: Arc<RTCPeerConnection>,
    local_fingerprint: String,
    auth_context: VerifiedPeerContext,
    ice_rx: Mutex<mpsc::UnboundedReceiver<RTCIceCandidateInit>>,
    rtp_rx: Mutex<mpsc::UnboundedReceiver<ReceivedRtpFrame>>,
    input_channel: Arc<Mutex<Option<Arc<RTCDataChannel>>>>,
}

impl ViewerMediaPeer {
    /// Creates a new viewer `RTCPeerConnection` configured with `ice_config`, an explicit
    /// local `RTCCertificate`, and verified handshake identity context.
    ///
    /// # Errors
    /// Returns [`CaptureError`] if certificate, ICE config, or `RTCPeerConnection` setup fails.
    pub async fn new(
        ice_config: &IceConfig,
        auth_context: VerifiedPeerContext,
    ) -> Result<Self, CaptureError> {
        let (rtc_cert, local_fingerprint) = generate_peer_certificate()?;
        let mut rtc_config = ice_config.to_rtc_configuration()?;
        rtc_config.certificates = vec![rtc_cert];

        let api = build_webrtc_api()?;
        let peer = Arc::new(
            api.new_peer_connection(rtc_config)
                .await
                .map_err(|e| CaptureError::WebRtc(e.to_string()))?,
        );

        let (ice_tx, ice_rx) = mpsc::unbounded_channel();
        peer.on_ice_candidate(Box::new(move |cand| {
            let tx = ice_tx.clone();
            Box::pin(async move {
                if let Some(c) = cand {
                    if let Ok(init) = c.to_json() {
                        let _ = tx.send(init);
                    }
                }
            })
        }));

        let (rtp_tx, rtp_rx) = mpsc::unbounded_channel();
        peer.on_track(Box::new(move |track, _, _| {
            let tx = rtp_tx.clone();
            Box::pin(async move {
                tokio::spawn(async move {
                    let mut buf = vec![0u8; 1600];
                    while let Ok((pkt, _)) = track.read(&mut buf).await {
                        let _ = tx.send(ReceivedRtpFrame {
                            sequence_number: pkt.header.sequence_number,
                            rtp_timestamp: pkt.header.timestamp,
                            marker: pkt.header.marker,
                            payload_len: pkt.payload.len(),
                            received_at: Instant::now(),
                        });
                    }
                });
            })
        }));

        let input_channel = Arc::new(Mutex::new(None));
        let ch_holder = Arc::clone(&input_channel);
        peer.on_data_channel(Box::new(move |dc| {
            let holder = Arc::clone(&ch_holder);
            Box::pin(async move {
                if dc.label() == "oud-input" {
                    *holder.lock().await = Some(dc);
                } else {
                    close_unexpected_data_channel(dc).await;
                }
            })
        }));

        Ok(Self {
            peer,
            local_fingerprint,
            auth_context,
            ice_rx: Mutex::new(ice_rx),
            rtp_rx: Mutex::new(rtp_rx),
            input_channel,
        })
    }

    /// Returns the viewer peer's pinned SHA-256 DTLS certificate fingerprint.
    #[must_use]
    pub fn local_fingerprint(&self) -> &str {
        &self.local_fingerprint
    }

    /// Verifies the host's signed [`DtlsFingerprintBinding`] and SDP `a=fingerprint` BEFORE
    /// calling `set_remote_description`, then generates and signs the viewer's SDP answer.
    ///
    /// # Errors
    /// Returns [`CaptureError::DtlsFingerprintMismatch`] if the signed host binding or remote
    /// SDP `a=fingerprint` is invalid, missing, duplicated, or swapped, or [`CaptureError::WebRtc`]
    /// if SDP negotiation fails.
    pub async fn accept_offer_and_create_answer(
        &self,
        offer: SignedSdpDescription,
        viewer_keys: &IdentityKeyPair,
    ) -> Result<SignedSdpDescription, CaptureError> {
        let verified_remote_fp = offer.fingerprint_binding.verify(&self.auth_context)?;
        verify_sdp_fingerprint(&offer.description, RTCSdpType::Offer, &verified_remote_fp)?;

        self.peer
            .set_remote_description(offer.description)
            .await
            .map_err(|e| CaptureError::WebRtc(e.to_string()))?;
        let answer = self
            .peer
            .create_answer(None)
            .await
            .map_err(|e| CaptureError::WebRtc(e.to_string()))?;
        verify_sdp_fingerprint(&answer, RTCSdpType::Answer, &self.local_fingerprint)?;
        let binding =
            DtlsFingerprintBinding::sign(&self.auth_context, viewer_keys, &self.local_fingerprint)?;
        self.peer
            .set_local_description(answer.clone())
            .await
            .map_err(|e| CaptureError::WebRtc(e.to_string()))?;
        Ok(SignedSdpDescription {
            description: answer,
            fingerprint_binding: binding,
        })
    }

    /// Adds a remote ICE candidate from the host peer.
    ///
    /// # Errors
    /// Returns [`CaptureError::WebRtc`] if candidate addition fails.
    pub async fn add_ice_candidate(
        &self,
        candidate: RTCIceCandidateInit,
    ) -> Result<(), CaptureError> {
        self.peer
            .add_ice_candidate(candidate)
            .await
            .map_err(|e| CaptureError::WebRtc(e.to_string()))
    }

    /// Drains gathered local ICE candidates.
    pub async fn drain_local_ice_candidates(&self) -> Vec<RTCIceCandidateInit> {
        let mut rx = self.ice_rx.lock().await;
        let mut out = Vec::new();
        while let Ok(c) = rx.try_recv() {
            out.push(c);
        }
        out
    }

    /// Waits for the next received H.264 RTP packet on the inbound video track.
    pub async fn recv_rtp_frame(&self) -> Option<ReceivedRtpFrame> {
        self.rtp_rx.lock().await.recv().await
    }

    /// Reads RTP packets until the frame marker bit indicates a complete H.264 video frame.
    pub async fn recv_complete_video_frame(&self) -> Option<DeliveredVideoFrame> {
        let mut rx = self.rtp_rx.lock().await;
        let mut packet_count = 0usize;
        let mut total_bytes = 0usize;
        while let Some(pkt) = rx.recv().await {
            packet_count += 1;
            total_bytes += pkt.payload_len;
            if pkt.marker {
                return Some(DeliveredVideoFrame {
                    rtp_timestamp: pkt.rtp_timestamp,
                    packet_count,
                    total_bytes,
                    delivered_at: pkt.received_at,
                });
            }
        }
        None
    }

    /// Sends an inbound message over the `oud-input` data channel to the host peer.
    ///
    /// # Errors
    /// Returns [`CaptureError::WebRtc`] if the data channel is not open yet or send fails.
    pub async fn send_input_message(&self, msg: &HostInboundMessage) -> Result<(), CaptureError> {
        let guard = self.input_channel.lock().await;
        let dc = guard.as_ref().ok_or_else(|| {
            CaptureError::WebRtc("oud-input data channel not open yet".to_owned())
        })?;
        let bytes = serde_json::to_vec(msg).map_err(|e| CaptureError::WebRtc(e.to_string()))?;
        dc.send(&Bytes::from(bytes))
            .await
            .map_err(|e| CaptureError::WebRtc(e.to_string()))?;
        Ok(())
    }

    /// Returns `true` once the `oud-input` data channel is attached and open.
    pub async fn is_input_channel_ready(&self) -> bool {
        let guard = self.input_channel.lock().await;
        guard.as_ref().is_some_and(|dc| {
            dc.ready_state() == webrtc::data_channel::data_channel_state::RTCDataChannelState::Open
        })
    }

    /// Returns the current `RTCPeerConnectionState`.
    #[must_use]
    pub fn connection_state(&self) -> RTCPeerConnectionState {
        self.peer.connection_state()
    }

    /// Closes the viewer peer connection cleanly.
    ///
    /// # Errors
    /// Returns [`CaptureError::WebRtc`] if close fails.
    pub async fn close(&self) -> Result<(), CaptureError> {
        self.peer
            .close()
            .await
            .map_err(|e| CaptureError::WebRtc(e.to_string()))
    }
}

async fn close_unexpected_data_channel(dc: Arc<RTCDataChannel>) {
    dc.on_message(Box::new(|_| Box::pin(async {})));
    let dc_for_open = Arc::clone(&dc);
    // `webrtc-rs` invokes `on_data_channel` before `RTCDataChannel::handle_open` binds the SCTP
    // stream, so defer `close()` until `on_open` when the underlying channel is attached.
    dc.on_open(Box::new(move || {
        let dc_inner = Arc::clone(&dc_for_open);
        Box::pin(async move {
            let _ = dc_inner.close().await;
        })
    }));
    if dc.ready_state() == webrtc::data_channel::data_channel_state::RTCDataChannelState::Open {
        let _ = dc.close().await;
    }
}

fn wire_host_peer_callbacks(
    peer: &RTCPeerConnection,
    input_channel: &RTCDataChannel,
    gatekeeper: &Arc<Mutex<HostFrameGatekeeper>>,
    protected_surfaces: &ProtectedSurfaceRegistry,
    authorized_inputs: &Arc<Mutex<Vec<HostInboundMessage>>>,
    unexpected_channels_rejected: &Arc<AtomicU64>,
) -> mpsc::UnboundedReceiver<RTCIceCandidateInit> {
    let unexpected_dc_count = Arc::clone(unexpected_channels_rejected);
    let unexpected_dc_gk = Arc::clone(gatekeeper);
    peer.on_data_channel(Box::new(move |dc: Arc<RTCDataChannel>| {
        let counter = Arc::clone(&unexpected_dc_count);
        let gk = Arc::clone(&unexpected_dc_gk);
        Box::pin(async move {
            counter.fetch_add(1, Ordering::Relaxed);
            gk.lock().await.record_rejected_payload();
            close_unexpected_data_channel(dc).await;
        })
    }));

    let gk_cb = Arc::clone(gatekeeper);
    let ps_cb = protected_surfaces.clone();
    let auth_cb = Arc::clone(authorized_inputs);
    input_channel.on_message(Box::new(move |msg: DataChannelMessage| {
        let gk = Arc::clone(&gk_cb);
        let ps = ps_cb.clone();
        let authorized = Arc::clone(&auth_cb);
        Box::pin(async move {
            let _ = dispatch_raw_input_bytes(&gk, &ps, &authorized, &msg.data, Utc::now()).await;
        })
    }));

    let (ice_tx, ice_rx) = mpsc::unbounded_channel();
    peer.on_ice_candidate(Box::new(move |cand| {
        let tx = ice_tx.clone();
        Box::pin(async move {
            if let Some(c) = cand {
                if let Ok(init) = c.to_json() {
                    let _ = tx.send(init);
                }
            }
        })
    }));
    ice_rx
}

async fn dispatch_raw_input_bytes(
    gatekeeper: &Mutex<HostFrameGatekeeper>,
    protected_surfaces: &ProtectedSurfaceRegistry,
    authorized_inputs: &Mutex<Vec<HostInboundMessage>>,
    data: &[u8],
    now: DateTime<Utc>,
) -> Result<HostInboundMessage, CaptureError> {
    let Ok(parsed) = serde_json::from_slice::<HostInboundMessage>(data) else {
        gatekeeper.lock().await.record_rejected_payload();
        return Err(CaptureError::FrameBlockedByGrant {
            frame_kind: "malformed_input",
        });
    };

    let mut gk = gatekeeper.lock().await;
    gk.authorize_frame(parsed.frame_kind(), now)?;
    if let HostInboundMessage::PointerAction { x, y, .. } = &parsed {
        if let Err(err) = protected_surfaces.enforce_remote_pointer(*x, *y, now) {
            gk.record_rejected_payload();
            return Err(err);
        }
    }
    drop(gk);

    authorized_inputs.lock().await.push(parsed.clone());
    Ok(parsed)
}

fn generate_peer_certificate() -> Result<(RTCCertificate, String), CaptureError> {
    let dtls_cert = DtlsCertificate::generate_self_signed(vec!["oudos-webrtc".to_owned()])
        .map_err(|e| CaptureError::WebRtc(format!("DTLS certificate generation failed: {e}")))?;
    let expires = SystemTime::now() + StdDuration::from_secs(DTLS_CERT_LIFETIME_SECS);
    let rtc_cert = RTCCertificate::from_existing(dtls_cert, expires);

    let mut sha256_fps = rtc_cert
        .get_fingerprints()
        .into_iter()
        .filter(|fp| {
            fp.algorithm
                .eq_ignore_ascii_case(SHA256_FINGERPRINT_ALGORITHM)
        })
        .map(|fp| normalize_sha256_fingerprint(&fp.value))
        .collect::<Result<Vec<_>, _>>()?;

    if sha256_fps.len() != 1 {
        return Err(CaptureError::DtlsFingerprintMismatch(
            "expected exactly one SHA-256 fingerprint from generated RTCCertificate".to_owned(),
        ));
    }

    Ok((rtc_cert, sha256_fps.swap_remove(0)))
}

fn verify_sdp_fingerprint(
    desc: &RTCSessionDescription,
    expected_type: RTCSdpType,
    expected_fp: &str,
) -> Result<(), CaptureError> {
    if desc.sdp_type != expected_type {
        return Err(CaptureError::DtlsFingerprintMismatch(format!(
            "unexpected SDP type: expected {expected_type}, got {}",
            desc.sdp_type
        )));
    }

    let expected_norm = normalize_sha256_fingerprint(expected_fp)?;
    let parsed = desc
        .unmarshal()
        .map_err(|e| CaptureError::DtlsFingerprintMismatch(format!("failed to parse SDP: {e}")))?;

    let mut parsed_attrs = Vec::new();
    for attr in &parsed.attributes {
        if attr.key == "fingerprint" {
            let val = attr.value.as_deref().ok_or_else(|| {
                CaptureError::DtlsFingerprintMismatch(
                    "SDP session-level fingerprint attribute missing value".to_owned(),
                )
            })?;
            parsed_attrs.push(val);
        }
    }
    for media in &parsed.media_descriptions {
        for attr in &media.attributes {
            if attr.key == "fingerprint" {
                let val = attr.value.as_deref().ok_or_else(|| {
                    CaptureError::DtlsFingerprintMismatch(
                        "SDP media-level fingerprint attribute missing value".to_owned(),
                    )
                })?;
                parsed_attrs.push(val);
            }
        }
    }

    let mut raw_attrs = Vec::new();
    for line in desc.sdp.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed
            .strip_prefix("a=fingerprint:")
            .or_else(|| trimmed.strip_prefix("A=FINGERPRINT:"))
        {
            raw_attrs.push(rest);
        } else if trimmed.len() >= 14 && trimmed[..14].eq_ignore_ascii_case("a=fingerprint:") {
            raw_attrs.push(&trimmed[14..]);
        }
    }

    if parsed_attrs.is_empty() || raw_attrs.is_empty() {
        return Err(CaptureError::DtlsFingerprintMismatch(
            "SDP contains no a=fingerprint attribute".to_owned(),
        ));
    }

    for attr in parsed_attrs.into_iter().chain(raw_attrs) {
        let actual = parse_sdp_fingerprint_attr(attr)?;
        if actual != expected_norm {
            return Err(CaptureError::DtlsFingerprintMismatch(format!(
                "SDP a=fingerprint `{actual}` does not match signed peer binding `{expected_norm}`"
            )));
        }
    }

    Ok(())
}

fn parse_sdp_fingerprint_attr(attr_value: &str) -> Result<String, CaptureError> {
    let mut parts = attr_value.split_whitespace();
    let alg = parts.next().ok_or_else(|| {
        CaptureError::DtlsFingerprintMismatch("empty SDP fingerprint attribute".to_owned())
    })?;
    let value = parts.next().ok_or_else(|| {
        CaptureError::DtlsFingerprintMismatch("missing SDP fingerprint digest value".to_owned())
    })?;
    if parts.next().is_some() {
        return Err(CaptureError::DtlsFingerprintMismatch(
            "unexpected trailing tokens in SDP fingerprint attribute".to_owned(),
        ));
    }
    if !alg.eq_ignore_ascii_case(SHA256_FINGERPRINT_ALGORITHM) {
        return Err(CaptureError::DtlsFingerprintMismatch(format!(
            "unsupported SDP DTLS fingerprint algorithm `{alg}`"
        )));
    }
    normalize_sha256_fingerprint(value)
}

fn normalize_sha256_fingerprint(raw: &str) -> Result<String, CaptureError> {
    let octets: Vec<&str> = raw.trim().split(':').collect();
    if octets.len() != 32
        || octets
            .iter()
            .any(|part| part.len() != 2 || !part.bytes().all(|b| b.is_ascii_hexdigit()))
    {
        return Err(CaptureError::DtlsFingerprintMismatch(format!(
            "malformed SHA-256 DTLS fingerprint `{}`",
            raw.trim()
        )));
    }
    Ok(octets
        .iter()
        .map(|part| part.to_ascii_lowercase())
        .collect::<Vec<_>>()
        .join(":"))
}

fn binding_signing_bytes(
    session_id: Uuid,
    local_fingerprint: &str,
    remote_pubkey: &IdentityPublicKey,
    handshake_transcript_hash: &[u8; 32],
    signer_pubkey: &IdentityPublicKey,
    algorithm: &str,
) -> Vec<u8> {
    let mut out = Vec::with_capacity(224);
    out.extend_from_slice(DTLS_BINDING_DOMAIN);
    out.extend_from_slice(session_id.as_bytes());
    let fp_len = u32::try_from(local_fingerprint.len()).unwrap_or(u32::MAX);
    out.extend_from_slice(&fp_len.to_be_bytes());
    out.extend_from_slice(local_fingerprint.as_bytes());
    out.extend_from_slice(remote_pubkey.as_bytes());
    out.extend_from_slice(handshake_transcript_hash);
    out.extend_from_slice(signer_pubkey.as_bytes());
    let alg_len = u32::try_from(algorithm.len()).unwrap_or(u32::MAX);
    out.extend_from_slice(&alg_len.to_be_bytes());
    out.extend_from_slice(algorithm.as_bytes());
    out
}

fn build_webrtc_api() -> Result<webrtc::api::API, CaptureError> {
    let mut media_engine = MediaEngine::default();
    media_engine
        .register_default_codecs()
        .map_err(|e| CaptureError::WebRtc(e.to_string()))?;
    let mut registry = Registry::new();
    registry = register_default_interceptors(registry, &mut media_engine)
        .map_err(|e| CaptureError::WebRtc(e.to_string()))?;
    Ok(APIBuilder::new()
        .with_media_engine(media_engine)
        .with_interceptor_registry(registry)
        .build())
}

#[cfg(test)]
mod tests {
    use super::*;
    use oud_protocol::grant::{GrantDuration, GrantPermissions, SessionGrant};
    use oud_protocol::handshake::{InitiatorPending, ResponderPending};
    use std::time::Duration;
    use tokio::time::sleep;
    use webrtc::data_channel::data_channel_state::RTCDataChannelState;

    fn handshake_contexts(
        host_keys: &IdentityKeyPair,
        viewer_keys: &IdentityKeyPair,
        session_id: Uuid,
    ) -> (VerifiedPeerContext, VerifiedPeerContext) {
        let (init, offer) =
            InitiatorPending::initiate(session_id, "support.example.org", viewer_keys);
        let (resp, accept) = ResponderPending::respond(&offer, host_keys).expect("offer verifies");
        let (reveal, viewer_out) = init
            .complete(&accept, Some(&host_keys.public_key()))
            .expect("accept verifies");
        let host_out = resp
            .verify_reveal_and_finalize(&reveal)
            .expect("reveal verifies");
        assert_eq!(viewer_out.transcript_hash, host_out.transcript_hash);

        let host_ctx = VerifiedPeerContext {
            session_id,
            local_pubkey: host_keys.public_key(),
            remote_pubkey: viewer_keys.public_key(),
            handshake_transcript_hash: host_out.transcript_hash,
        };
        let viewer_ctx = VerifiedPeerContext {
            session_id,
            local_pubkey: viewer_keys.public_key(),
            remote_pubkey: host_keys.public_key(),
            handshake_transcript_hash: viewer_out.transcript_hash,
        };
        (host_ctx, viewer_ctx)
    }

    #[tokio::test]
    async fn host_rejects_unexpected_viewer_data_channels_and_malformed_input() {
        let host_keys = IdentityKeyPair::generate();
        let viewer_keys = IdentityKeyPair::generate();
        let session_id = Uuid::now_v7();
        let now = Utc::now();

        let (host_ctx, viewer_ctx) = handshake_contexts(&host_keys, &viewer_keys, session_id);
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
        let pipeline = AdaptiveH264Pipeline::new(QualityPreset::Auto).expect("pipeline");
        let ice = IceConfig::local_loopback();

        let host = Box::pin(HostMediaPeer::new(&ice, gk, pipeline, host_ctx))
            .await
            .expect("host");
        let viewer = Box::pin(ViewerMediaPeer::new(&ice, viewer_ctx))
            .await
            .expect("viewer");

        let offer = host.create_offer(&host_keys).await.expect("offer");
        let answer = viewer
            .accept_offer_and_create_answer(offer, &viewer_keys)
            .await
            .expect("answer");
        host.accept_answer(answer).await.expect("accept answer");

        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            for cand in host.drain_local_ice_candidates().await {
                viewer.add_ice_candidate(cand).await.expect("viewer ice");
            }
            for cand in viewer.drain_local_ice_candidates().await {
                host.add_ice_candidate(cand).await.expect("host ice");
            }
            if host.connection_state() == RTCPeerConnectionState::Connected
                && viewer.connection_state() == RTCPeerConnectionState::Connected
                && viewer.is_input_channel_ready().await
            {
                break;
            }
            assert!(Instant::now() < deadline, "peers failed to connect");
            sleep(Duration::from_millis(15)).await;
        }

        // Send malformed non-JSON bytes over the authorized `oud-input` channel.
        {
            let guard = viewer.input_channel.lock().await;
            let dc = guard.as_ref().expect("oud-input attached");
            dc.send(&Bytes::from_static(b"not-json-payload"))
                .await
                .expect("send malformed bytes");
        }

        // Viewer opens an unauthorized secondary data channel directly on RTCPeerConnection.
        let rogue_dc = viewer
            .peer
            .create_data_channel("rogue-control", None)
            .await
            .expect("viewer creates rogue channel");

        let check_deadline = Instant::now() + Duration::from_secs(3);
        loop {
            if host.unexpected_channels_rejected() >= 1
                && rogue_dc.ready_state() == RTCDataChannelState::Closed
                && host.gatekeeper().lock().await.rejected_malformed_payloads() >= 2
            {
                break;
            }
            assert!(
                Instant::now() < check_deadline,
                "expected rogue channel closure and malformed payload rejection (unexpected={}, closed={:?}, malformed={})",
                host.unexpected_channels_rejected(),
                rogue_dc.ready_state(),
                host.gatekeeper().try_lock().map_or(0, |g| g.rejected_malformed_payloads())
            );
            sleep(Duration::from_millis(20)).await;
        }

        assert!(
            host.drain_authorized_inputs().await.is_empty(),
            "authorized inputs must remain empty after malformed payload and unexpected channel"
        );
        assert!(host.gatekeeper().lock().await.dropped_frames() >= 2);

        host.close().await.expect("close host");
        viewer.close().await.expect("close viewer");
    }
}
