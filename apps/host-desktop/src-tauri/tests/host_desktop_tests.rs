//! Integration tests for `oud-host-desktop` tray icon silhouettes, consent cooling-off,
//! persistent banner enforcement, local-only grant revocation, and protected rect isolation.

use chrono::{Duration, Utc};
use oud_capture::{
    chunk_file_bytes, CaptureError, ClipboardEndpoint, HostInboundMessage, ProtectedSurfaceKind,
    Rect,
};
use oud_host_desktop::host_state::{
    decide_window_close_action, sanitize_requester_name, validate_server_domain, HostDesktopState,
    HostPermissionSelection, IncomingRequestParams, InputOrigin, WindowCloseAction,
    MAX_REQUESTER_NAME_CHARS,
};
use oud_host_desktop::tray::{
    encode_rgba_ico_32, encode_rgba_png, render_monochrome_brackets_rgba, render_tray_rgba,
    TrayVisualState, MONOCHROME_BUNDLE_INK,
};
use oud_policy::PolicyLayer;
use oud_protocol::audit::{verify_chain, AuditEventKind};
use oud_protocol::grant::{ClipboardDirection, FrameKind, GrantDuration};
use oud_protocol::identity::IdentityKeyPair;
use oud_protocol::ProtocolError;
use uuid::Uuid;

fn pixel(rgba: &[u8], x: usize, y: usize) -> [u8; 4] {
    let idx = (y * 32 + x) * 4;
    [rgba[idx], rgba[idx + 1], rgba[idx + 2], rgba[idx + 3]]
}

fn opaque_pixels_are_monochrome(rgba: &[u8]) -> bool {
    let mut expected_rgb: Option<[u8; 3]> = None;
    for px in rgba.as_chunks::<4>().0 {
        if px[3] > 0 {
            let rgb = [px[0], px[1], px[2]];
            if let Some(first) = expected_rgb {
                if rgb != first {
                    return false;
                }
            } else {
                expected_rgb = Some(rgb);
            }
        }
    }
    expected_rgb.is_some()
}

fn sample_request(first_time: bool, enrolled: bool) -> IncomingRequestParams {
    IncomingRequestParams {
        session_id: Uuid::now_v7(),
        requester_name: "Alex Rivera".to_owned(),
        viewer_keys: IdentityKeyPair::generate(),
        server_domain: if enrolled {
            "support.example.org".to_owned()
        } else {
            "unpaired.evil.example".to_owned()
        },
        server_enrolled: enrolled,
        host_key_matches_pin: true,
        authenticated_requester: true,
        rate_limit_passed: true,
        first_time_requester: first_time,
        requested_permissions: HostPermissionSelection::default(),
    }
}

#[test]
fn tray_states_are_monochrome_and_distinct_by_shape() {
    let idle = render_tray_rgba(TrayVisualState::Idle);
    let in_session = render_tray_rgba(TrayVisualState::InSession);
    let alert = render_tray_rgba(TrayVisualState::Alert);

    for buf in [&idle, &in_session, &alert] {
        assert_eq!(buf.len(), 32 * 32 * 4);
        assert!(opaque_pixels_are_monochrome(buf));
        assert_eq!(pixel(buf, 5, 15)[3], 0xFF);
        assert_eq!(pixel(buf, 26, 15)[3], 0xFF);
    }

    assert_eq!(pixel(&idle, 16, 16)[3], 0);
    assert_eq!(pixel(&idle, 27, 5)[3], 0);

    assert_eq!(pixel(&in_session, 16, 16)[3], 0xFF);
    assert_eq!(pixel(&in_session, 27, 5)[3], 0);

    assert_eq!(pixel(&alert, 16, 13)[3], 0xFF);
    assert_eq!(pixel(&alert, 16, 18)[3], 0);
    assert_eq!(pixel(&alert, 16, 20)[3], 0xFF);
    assert_eq!(pixel(&alert, 27, 5)[3], 0xFF);
}

#[test]
fn png_and_ico_encoders_produce_valid_headers() {
    let rgba =
        render_monochrome_brackets_rgba(TrayVisualState::InSession, 32, MONOCHROME_BUNDLE_INK);
    let png = encode_rgba_png(&rgba, 32, 32);
    assert_eq!(
        &png[0..8],
        &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]
    );
    assert_eq!(&png[12..16], b"IHDR");

    let ico = encode_rgba_ico_32(&rgba);
    assert_eq!(&ico[0..6], &[0, 0, 1, 0, 1, 0]);
    assert_eq!(ico[6], 32);
    assert_eq!(ico[7], 32);
}

#[test]
fn first_time_requester_enforces_5s_cooling_off_and_rejects_remote_allow() {
    let host_keys = IdentityKeyPair::generate();
    let host_pubkey = host_keys.public_key();
    let mut state = HostDesktopState::with_default_policy(host_keys);
    let t0 = Utc::now();

    let snap = state
        .receive_incoming_request(&sample_request(true, true), t0)
        .expect("incoming request accepted");
    assert_eq!(snap.tray_state, TrayVisualState::Idle);
    assert_eq!(snap.consent_state_name, "COOLING_OFF_5S");
    let req = snap.pending_request.expect("pending consent card");
    assert_eq!(req.server_domain, "support.example.org");
    assert!(req.server_enrolled);
    assert_eq!(req.cooling_off_remaining_secs, 5);
    assert!(!req.permissions.mouse_keyboard);
    assert!(!req.permissions.clipboard_sync);
    assert!(!req.permissions.file_transfer);

    let early = state.allow_session(
        InputOrigin::LocalHost,
        HostPermissionSelection::default(),
        GrantDuration::ThisSession,
        t0 + Duration::seconds(4),
    );
    assert!(matches!(
        early,
        Err(CaptureError::GrantError(ProtocolError::CoolingOffActive))
    ));

    let remote_allow = state.allow_session(
        InputOrigin::RemoteOrSynthetic,
        HostPermissionSelection::default(),
        GrantDuration::ThisSession,
        t0 + Duration::seconds(6),
    );
    assert!(matches!(
        remote_allow,
        Err(CaptureError::ProtectedSurfaceInputBlocked {
            surface: "consent_prompt",
            ..
        })
    ));
    assert!(verify_chain(state.audit_chain().entries(), &host_pubkey).is_ok());
}

fn activate_sample_session(
    now: chrono::DateTime<Utc>,
) -> (HostDesktopState, oud_protocol::identity::IdentityPublicKey) {
    let host_keys = IdentityKeyPair::generate();
    let host_pubkey = host_keys.public_key();
    let mut state = HostDesktopState::with_default_policy(host_keys);
    state
        .receive_incoming_request(&sample_request(true, true), now)
        .expect("incoming request accepted");
    (state, host_pubkey)
}

#[test]
fn local_allow_mounts_banner_gates_frames_and_revokes_on_local_end() {
    let t0 = Utc::now();
    let (mut state, host_pubkey) = activate_sample_session(t0);

    let t_allow = t0 + Duration::seconds(6);
    let snap_active = state
        .allow_session(
            InputOrigin::LocalHost,
            HostPermissionSelection::default(),
            GrantDuration::OneHour,
            t_allow,
        )
        .expect("local allow succeeds after cooling off");
    assert_eq!(snap_active.tray_state, TrayVisualState::InSession);
    assert_eq!(snap_active.consent_state_name, "GRANT_SIGNED_ACTIVE");
    let banner = snap_active.active_banner.expect("banner mounted");
    assert_eq!(banner.status_label, "LIVE");
    assert_eq!(banner.permission_summary, "Screen only");
    assert_eq!(banner.server_domain, "support.example.org");

    assert!(state
        .enforce_inbound_message(
            &HostInboundMessage::MouseKeyboardInput {
                seq: 1,
                action: "click".to_owned(),
            },
            t_allow,
        )
        .is_err());
    assert!(state
        .enforce_inbound_message(
            &HostInboundMessage::FileChunk {
                transfer_id: "f-1".to_owned(),
                bytes_len: 10,
            },
            t_allow,
        )
        .is_err());
    assert_eq!(
        state
            .gatekeeper()
            .expect("gatekeeper active")
            .dropped_frames(),
        2
    );

    let t_end = t_allow + Duration::seconds(5);
    let snap_ended = state
        .end_session_local(InputOrigin::LocalHost, t_end)
        .expect("local end session succeeds");
    assert_eq!(snap_ended.tray_state, TrayVisualState::Idle);
    assert!(snap_ended.active_banner.is_none());
    assert!(state
        .gatekeeper()
        .expect("revoked gatekeeper")
        .clone()
        .authorize_frame(FrameKind::ScreenView, t_end)
        .is_err());
    assert!(verify_chain(state.audit_chain().entries(), &host_pubkey).is_ok());
}

#[test]
fn mid_session_input_enable_still_blocks_banner_rect_and_remote_dismiss() {
    let t0 = Utc::now();
    let (mut state, host_pubkey) = activate_sample_session(t0);
    let t_allow = t0 + Duration::seconds(6);
    state
        .allow_session(
            InputOrigin::LocalHost,
            HostPermissionSelection::default(),
            GrantDuration::OneHour,
            t_allow,
        )
        .expect("allow session");

    state.set_protected_surface_rect(
        ProtectedSurfaceKind::SessionBanner,
        Rect {
            x: 500,
            y: 12,
            width: 640,
            height: 56,
        },
        None,
    );
    state
        .update_permissions(
            InputOrigin::LocalHost,
            HostPermissionSelection {
                view_screen: true,
                mouse_keyboard: true,
                clipboard_sync: false,
                file_transfer: false,
            },
            t_allow + Duration::seconds(1),
        )
        .expect("mid-session permission update");
    assert!(matches!(
        state.enforce_inbound_message(
            &HostInboundMessage::PointerAction {
                seq: 2,
                action: "click".to_owned(),
                x: 800,
                y: 30,
            },
            t_allow + Duration::seconds(1),
        ),
        Err(CaptureError::ProtectedSurfaceInputBlocked {
            surface: "session_banner",
            ..
        })
    ));
    assert!(state
        .end_session_local(
            InputOrigin::RemoteOrSynthetic,
            t_allow + Duration::seconds(2)
        )
        .is_err());
    assert_eq!(
        state.snapshot(t_allow).tray_state,
        TrayVisualState::InSession
    );
    assert!(verify_chain(state.audit_chain().entries(), &host_pubkey).is_ok());
}

#[test]
fn unenrolled_server_triggers_alert_tray_and_blocks_consent() {
    let host_keys = IdentityKeyPair::generate();
    let host_pubkey = host_keys.public_key();
    let mut state = HostDesktopState::with_default_policy(host_keys);
    let res = state.receive_incoming_request(&sample_request(false, false), Utc::now());
    assert!(matches!(
        res,
        Err(CaptureError::GrantError(ProtocolError::ServerNotEnrolled))
    ));
    let snap = state.snapshot(Utc::now());
    assert_eq!(snap.tray_state, TrayVisualState::Alert);
    assert!(snap.pending_request.is_none());
    assert!(verify_chain(state.audit_chain().entries(), &host_pubkey).is_ok());
}

#[test]
fn requester_spoofing_newlines_bidi_and_oversize_inputs_are_sanitized_or_rejected() {
    // Trust-badge spoofing including zero-width/bidi evasion must fail closed.
    for spoof in [
        "Your Bank (Verified)",
        "Support [VERIFIED]",
        "Bank \u{200B}(Ver\u{202E}ified)",
        "Your Bank\r\n(Ver\nified)",
    ] {
        assert!(
            sanitize_requester_name(spoof).is_err(),
            "spoof requester `{spoof}` must be rejected"
        );
    }

    // Newlines, carriage returns, bidi overrides, and parentheses are stripped and capped <= 48 chars.
    let noisy = format!("Alex\r\n\u{202E}Rivera (Support) {}", "X".repeat(80));
    let cleaned = sanitize_requester_name(&noisy).expect("benign name sanitized");
    assert!(cleaned.chars().count() <= MAX_REQUESTER_NAME_CHARS);
    assert!(!cleaned.contains(['\r', '\n', '\u{202E}', '(', ')']));
    assert!(cleaned.starts_with("AlexRivera Support"));

    // Invalid / spoofed server_domain values fail closed.
    assert!(validate_server_domain("support.example.org (Verified)").is_err());
    assert!(validate_server_domain("bank.com@evil.example").is_err());
    assert!(validate_server_domain(&format!("{}.example.org", "a".repeat(250))).is_err());
    assert_eq!(
        validate_server_domain(" Support.Example.Org ").expect("valid hostname"),
        "support.example.org"
    );

    // End-to-end receive_incoming_request rejects "Your Bank (Verified)" and sets Alert tray state.
    let host_keys = IdentityKeyPair::generate();
    let mut state = HostDesktopState::with_default_policy(host_keys);
    let mut bad_req = sample_request(true, true);
    bad_req.requester_name = "Your Bank (Verified)".to_owned();
    assert!(state
        .receive_incoming_request(&bad_req, Utc::now())
        .is_err());
    let snap = state.snapshot(Utc::now());
    assert_eq!(snap.tray_state, TrayVisualState::Alert);
    assert!(snap.pending_request.is_none());
}

#[test]
fn close_requested_guard_prevents_banner_dismissal_and_denies_pending_consent() {
    assert_eq!(
        decide_window_close_action("banner", false, true),
        WindowCloseAction::PreventAndKeepVisible
    );
    assert_eq!(
        decide_window_close_action("banner", false, false),
        WindowCloseAction::PreventAndHide
    );
    assert_eq!(
        decide_window_close_action("consent", true, false),
        WindowCloseAction::PreventHideAndDenyPending
    );
    assert_eq!(
        decide_window_close_action("consent", false, false),
        WindowCloseAction::PreventAndHide
    );
}

#[test]
fn host_clipboard_direction_fails_closed_and_logs_metadata_only() {
    let t0 = Utc::now();
    let (mut state, host_pubkey) = activate_sample_session(t0);
    let t_allow = t0 + Duration::seconds(6);
    state
        .allow_session(
            InputOrigin::LocalHost,
            HostPermissionSelection::default(),
            GrantDuration::OneHour,
            t_allow,
        )
        .expect("allow screen-only");
    assert!(state
        .process_clipboard_sync(ClipboardEndpoint::Host, "secret-clip", t_allow)
        .is_err());

    state
        .update_permissions(
            InputOrigin::LocalHost,
            HostPermissionSelection {
                view_screen: true,
                mouse_keyboard: false,
                clipboard_sync: true,
                file_transfer: false,
            },
            t_allow + Duration::seconds(1),
        )
        .expect("enable clipboard");
    assert!(state
        .set_clipboard_direction(
            InputOrigin::RemoteOrSynthetic,
            ClipboardDirection::Bidirectional,
            t_allow + Duration::seconds(2),
        )
        .is_err());
    state
        .set_clipboard_direction(
            InputOrigin::LocalHost,
            ClipboardDirection::HostToViewer,
            t_allow + Duration::seconds(2),
        )
        .expect("local host sets HostToViewer");

    assert!(state
        .process_clipboard_sync(
            ClipboardEndpoint::Viewer,
            "blocked viewer->host",
            t_allow + Duration::seconds(2)
        )
        .is_err());
    assert!(state
        .process_clipboard_sync(
            ClipboardEndpoint::Host,
            "super-secret-host-clipboard",
            t_allow + Duration::seconds(2)
        )
        .is_ok());

    assert!(verify_chain(state.audit_chain().entries(), &host_pubkey).is_ok());
    let audit_json = serde_json::to_string(state.audit_chain().entries()).expect("json");
    assert!(!audit_json.contains("super-secret-host-clipboard"));
    assert!(state
        .audit_chain()
        .entries()
        .iter()
        .any(|e| matches!(e.event.kind, AuditEventKind::ClipboardSyncEvent)));
}

#[test]
fn host_opt_in_file_transfer_enforces_limits_and_logs_metadata_only() {
    let t0 = Utc::now();
    let host_keys = IdentityKeyPair::generate();
    let host_pubkey = host_keys.public_key();
    let policy_allowing_ft = PolicyLayer {
        allow_file_transfer: Some(true),
        max_file_size_bytes: Some(1024),
        ..PolicyLayer::default()
    };
    let mut state = HostDesktopState::new(
        host_keys,
        &policy_allowing_ft,
        &PolicyLayer::default(),
        &PolicyLayer::default(),
    );
    state
        .receive_incoming_request(&sample_request(false, true), t0)
        .expect("incoming request");

    let t_allow = t0 + Duration::seconds(1);
    state
        .allow_session(
            InputOrigin::LocalHost,
            HostPermissionSelection::default(),
            GrantDuration::OneHour,
            t_allow,
        )
        .expect("allow screen-only");
    assert!(state
        .start_file_transfer("tx-blocked", "notes.txt", 16, t_allow)
        .is_err());

    state
        .update_permissions(
            InputOrigin::LocalHost,
            HostPermissionSelection {
                view_screen: true,
                mouse_keyboard: false,
                clipboard_sync: false,
                file_transfer: true,
            },
            t_allow + Duration::seconds(1),
        )
        .expect("enable file transfer");

    assert!(state
        .start_file_transfer("tx-big", "large.txt", 2048, t_allow + Duration::seconds(3))
        .is_err());
    assert!(state
        .start_file_transfer("tx-exe", "evil.exe", 64, t_allow + Duration::seconds(3))
        .is_err());

    let payload = b"confidential-file-payload-never-logged";
    state
        .start_file_transfer(
            "tx-good",
            "invoice.pdf",
            payload.len() as u64,
            t_allow + Duration::seconds(4),
        )
        .expect("start tx-good");
    for pkt in chunk_file_bytes("tx-good", payload, 16).expect("chunks") {
        state
            .push_file_chunk(
                "tx-good",
                pkt.seq,
                &pkt.data,
                t_allow + Duration::seconds(4),
            )
            .expect("push chunk");
    }
    let completed = state
        .complete_file_transfer("tx-good", t_allow + Duration::seconds(5))
        .expect("complete");
    assert_eq!(completed.filename, "invoice.pdf");

    assert!(verify_chain(state.audit_chain().entries(), &host_pubkey).is_ok());
    let audit_json = serde_json::to_string(state.audit_chain().entries()).expect("json");
    assert!(!audit_json.contains("confidential-file-payload-never-logged"));
    assert!(audit_json.contains("invoice.pdf"));
    assert!(state.audit_chain().entries().iter().any(|e| matches!(
        e.event.kind,
        AuditEventKind::FileTransferMetadata { ref filename, size_bytes }
        if filename == "invoice.pdf" && size_bytes == payload.len() as u64
    )));
}
