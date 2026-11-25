//! Property-based (10,000+ sequences) and negative-transition tests for `oud-protocol`.

use chrono::{DateTime, Duration, Utc};
use oud_protocol::{
    BlockedReason, ConsentEvent, ConsentMachine, ConsentState, DenialReason, FrameKind,
    GrantDuration, GrantPermissions, HandshakeAccept, HandshakeOffer, IdentityKeyPair,
    InitiatorPending, ProtocolError, ResponderPending, RevocationReason, SessionGrant,
    CONSENT_TTL_SECS, COOLING_OFF_SECS,
};
use proptest::prelude::*;
use uuid::Uuid;

#[derive(Debug, Clone)]
enum FuzzAction {
    RequestIncoming {
        auth: bool,
        enrolled: bool,
        rate_ok: bool,
        pin_ok: bool,
        first_time: bool,
    },
    HandshakeVerified {
        valid_sas: bool,
    },
    SasCheck {
        matches: bool,
    },
    AdvanceTimeSecs(i64),
    HostAllow {
        use_attacker_key: bool,
        wrong_session: bool,
        wrong_viewer: bool,
        wrong_server: bool,
        blank_screen: bool,
        expired_grant: bool,
        allow_input: bool,
    },
    HostDeny,
    UpdatePermissions {
        use_attacker_key: bool,
        allow_input: bool,
        allow_clipboard: bool,
        allow_file: bool,
        blank_screen: bool,
    },
    ControlPlaneOffline,
    HostKeyMismatch,
    RevokeSession,
    Acknowledge,
}

fn fuzz_action_strategy() -> impl Strategy<Value = FuzzAction> {
    prop_oneof![
        (
            any::<bool>(),
            any::<bool>(),
            any::<bool>(),
            any::<bool>(),
            any::<bool>()
        )
            .prop_map(|(auth, enrolled, rate_ok, pin_ok, first_time)| {
                FuzzAction::RequestIncoming {
                    auth,
                    enrolled,
                    rate_ok,
                    pin_ok,
                    first_time,
                }
            }),
        any::<bool>().prop_map(|valid_sas| FuzzAction::HandshakeVerified { valid_sas }),
        any::<bool>().prop_map(|matches| FuzzAction::SasCheck { matches }),
        (0_i64..=200_i64).prop_map(FuzzAction::AdvanceTimeSecs),
        (
            any::<bool>(),
            any::<bool>(),
            any::<bool>(),
            any::<bool>(),
            any::<bool>(),
            any::<bool>(),
            any::<bool>(),
        )
            .prop_map(
                |(
                    use_attacker_key,
                    wrong_session,
                    wrong_viewer,
                    wrong_server,
                    blank_screen,
                    expired_grant,
                    allow_input,
                )| {
                    FuzzAction::HostAllow {
                        use_attacker_key,
                        wrong_session,
                        wrong_viewer,
                        wrong_server,
                        blank_screen,
                        expired_grant,
                        allow_input,
                    }
                }
            ),
        Just(FuzzAction::HostDeny),
        (
            any::<bool>(),
            any::<bool>(),
            any::<bool>(),
            any::<bool>(),
            any::<bool>(),
        )
            .prop_map(
                |(use_attacker_key, allow_input, allow_clipboard, allow_file, blank_screen)| {
                    FuzzAction::UpdatePermissions {
                        use_attacker_key,
                        allow_input,
                        allow_clipboard,
                        allow_file,
                        blank_screen,
                    }
                }
            ),
        Just(FuzzAction::ControlPlaneOffline),
        Just(FuzzAction::HostKeyMismatch),
        Just(FuzzAction::RevokeSession),
        Just(FuzzAction::Acknowledge),
    ]
}

fn assert_state_invariants(
    machine: &ConsentMachine,
    true_host_keys: &IdentityKeyPair,
    now: DateTime<Utc>,
) {
    let host_pub = true_host_keys.public_key();
    match machine.state() {
        ConsentState::GrantSignedActive(ctx) | ConsentState::GrantUpdatedImmediately(ctx) => {
            assert!(ctx.banner_mounted, "persistent banner must be mounted");
            assert!(
                !ctx.grant.permissions.blank_screen,
                "blank_screen must remain false in v0.1"
            );
            assert!(
                ctx.grant
                    .verify(
                        &host_pub,
                        ctx.grant.session_id,
                        &ctx.grant.viewer_pubkey,
                        &ctx.grant.server_domain,
                        now,
                    )
                    .is_ok(),
                "active grant must always carry a valid true host signature"
            );
            assert!(!machine.can_forward_frame(FrameKind::BlankScreen, now));
            assert_eq!(
                machine.can_forward_frame(FrameKind::MouseKeyboard, now),
                ctx.grant.permissions.mouse_keyboard
            );
            assert_eq!(
                machine.can_forward_frame(FrameKind::ClipboardSync, now),
                ctx.grant.permissions.clipboard_sync
            );
            assert_eq!(
                machine.can_forward_frame(FrameKind::FileTransfer, now),
                ctx.grant.permissions.file_transfer
            );
        }
        _ => {
            for frame in [
                FrameKind::ScreenView,
                FrameKind::MouseKeyboard,
                FrameKind::ClipboardSync,
                FrameKind::FileTransfer,
                FrameKind::BlankScreen,
            ] {
                assert!(
                    !machine.can_forward_frame(frame, now),
                    "non-active state must never allow frame {frame:?}"
                );
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10_000))]

    #[test]
    fn prop_never_reaches_active_grant_without_valid_host_signature(
        actions in proptest::collection::vec(fuzz_action_strategy(), 1..25)
    ) {
        let host_keys = IdentityKeyPair::from_seed([0x11; 32]);
        let attacker_keys = IdentityKeyPair::from_seed([0x22; 32]);
        let viewer_keys = IdentityKeyPair::from_seed([0x33; 32]);
        let wrong_viewer_pub = IdentityKeyPair::from_seed([0x44; 32]).public_key();

        let mut now = DateTime::from_timestamp(1_764_000_000, 0).expect("valid timestamp");
        let mut machine = ConsentMachine::new();
        let mut current_session_id = Uuid::now_v7();
        let server_domain = "support.example.org".to_owned();

        for action in actions {
            match action {
                FuzzAction::RequestIncoming {
                    auth,
                    enrolled,
                    rate_ok,
                    pin_ok,
                    first_time,
                } => {
                    current_session_id = Uuid::now_v7();
                    let _ = machine.apply(ConsentEvent::RequestIncoming {
                        session_id: current_session_id,
                        viewer_pubkey: viewer_keys.public_key(),
                        server_domain: server_domain.clone(),
                        authenticated_requester: auth,
                        server_enrolled: enrolled,
                        rate_limit_passed: rate_ok,
                        host_key_matches_pin: pin_ok,
                        first_time_requester: first_time,
                        now,
                    });
                }
                FuzzAction::HandshakeVerified { valid_sas } => {
                    let sas = if valid_sas {
                        "739204".to_owned()
                    } else {
                        "bad".to_owned()
                    };
                    let _ = machine.apply(ConsentEvent::HandshakeVerified {
                        derived_sas: sas,
                        now,
                    });
                }
                FuzzAction::SasCheck { matches } => {
                    let reported = if matches {
                        "739204".to_owned()
                    } else {
                        "000000".to_owned()
                    };
                    let _ = machine.apply(ConsentEvent::SasCheck {
                        reported_sas: reported,
                        now,
                    });
                }
                FuzzAction::AdvanceTimeSecs(secs) => {
                    now += Duration::seconds(secs);
                    let _ = machine.apply(ConsentEvent::Tick { now });
                }
                FuzzAction::HostAllow {
                    use_attacker_key,
                    wrong_session,
                    wrong_viewer,
                    wrong_server,
                    blank_screen,
                    expired_grant,
                    allow_input,
                } => {
                    let signer = if use_attacker_key {
                        &attacker_keys
                    } else {
                        &host_keys
                    };
                    let sid = if wrong_session {
                        Uuid::now_v7()
                    } else {
                        current_session_id
                    };
                    let vpub = if wrong_viewer {
                        wrong_viewer_pub
                    } else {
                        viewer_keys.public_key()
                    };
                    let sdom = if wrong_server {
                        "evil.example.org"
                    } else {
                        server_domain.as_str()
                    };
                    let issued_at = if expired_grant {
                        now - Duration::seconds(4000)
                    } else {
                        now
                    };
                    let perms = GrantPermissions {
                        view_screen: true,
                        mouse_keyboard: allow_input,
                        clipboard_sync: false,
                        file_transfer: false,
                        blank_screen,
                    };
                    if let Ok(grant) = SessionGrant::issue(
                        signer,
                        sid,
                        vpub,
                        sdom,
                        perms,
                        GrantDuration::OneHour,
                        issued_at,
                    ) {
                        let _ = machine.apply(ConsentEvent::HostAllow {
                            grant,
                            host_pubkey: host_keys.public_key(),
                            idle_timeout_secs: Some(900),
                            now,
                        });
                    }
                }
                FuzzAction::HostDeny => {
                    let _ = machine.apply(ConsentEvent::HostDeny {
                        reason: DenialReason::OwnerDenied,
                    });
                }
                FuzzAction::UpdatePermissions {
                    use_attacker_key,
                    allow_input,
                    allow_clipboard,
                    allow_file,
                    blank_screen,
                } => {
                    let signer = if use_attacker_key {
                        &attacker_keys
                    } else {
                        &host_keys
                    };
                    let perms = GrantPermissions {
                        view_screen: true,
                        mouse_keyboard: allow_input,
                        clipboard_sync: allow_clipboard,
                        file_transfer: allow_file,
                        blank_screen,
                    };
                    if let Ok(updated_grant) = SessionGrant::issue(
                        signer,
                        current_session_id,
                        viewer_keys.public_key(),
                        &server_domain,
                        perms,
                        GrantDuration::OneHour,
                        now,
                    ) {
                        let _ = machine.apply(ConsentEvent::HostUpdatePermissions {
                            updated_grant,
                            now,
                        });
                    }
                }
                FuzzAction::ControlPlaneOffline => {
                    let _ = machine.apply(ConsentEvent::ControlPlaneOffline);
                }
                FuzzAction::HostKeyMismatch => {
                    let _ = machine.apply(ConsentEvent::HostKeyMismatch);
                }
                FuzzAction::RevokeSession => {
                    let _ = machine.apply(ConsentEvent::RevokeSession {
                        reason: RevocationReason::HostEnded,
                    });
                }
                FuzzAction::Acknowledge => {
                    let _ = machine.apply(ConsentEvent::AcknowledgeTerminal);
                }
            }

            assert_state_invariants(&machine, &host_keys, now);
        }
    }
}

fn setup_consented_context(
    machine: &mut ConsentMachine,
    host_keys: &IdentityKeyPair,
    viewer_keys: &IdentityKeyPair,
    session_id: Uuid,
    first_time: bool,
    now: DateTime<Utc>,
) -> String {
    let (init, offer): (InitiatorPending, HandshakeOffer) =
        InitiatorPending::initiate(session_id, "support.example.org", viewer_keys);
    let (resp, accept): (ResponderPending, HandshakeAccept) =
        ResponderPending::respond(&offer, host_keys).expect("offer verifies");
    let (reveal, viewer_out) = init
        .complete(&accept, Some(&host_keys.public_key()))
        .expect("accept verifies");
    let host_out = resp
        .verify_reveal_and_finalize(&reveal)
        .expect("reveal verifies");
    assert_eq!(viewer_out.sas_code, host_out.sas_code);

    machine
        .apply(ConsentEvent::RequestIncoming {
            session_id,
            viewer_pubkey: viewer_keys.public_key(),
            server_domain: "support.example.org".to_owned(),
            authenticated_requester: true,
            server_enrolled: true,
            rate_limit_passed: true,
            host_key_matches_pin: true,
            first_time_requester: first_time,
            now,
        })
        .expect("verifying identity");
    machine
        .apply(ConsentEvent::HandshakeVerified {
            derived_sas: host_out.sas_code.clone(),
            now,
        })
        .expect("awaiting consent or cooling off");

    host_out.sas_code.clone()
}

#[test]
fn negative_deny_fails_closed() {
    let host = IdentityKeyPair::generate();
    let viewer = IdentityKeyPair::generate();
    let now = Utc::now();
    let mut machine = ConsentMachine::new();
    let sid = Uuid::now_v7();

    setup_consented_context(&mut machine, &host, &viewer, sid, false, now);
    machine
        .apply(ConsentEvent::HostDeny {
            reason: DenialReason::OwnerDenied,
        })
        .expect("deny succeeds");
    assert!(matches!(
        machine.state(),
        ConsentState::DeniedLogged(DenialReason::OwnerDenied)
    ));
    assert!(!machine.can_forward_frame(FrameKind::ScreenView, now));
}

#[test]
fn negative_2m_timeout_fails_closed() {
    let host = IdentityKeyPair::generate();
    let viewer = IdentityKeyPair::generate();
    let now = Utc::now();
    let mut machine = ConsentMachine::new();
    let sid = Uuid::now_v7();

    let sas = setup_consented_context(&mut machine, &host, &viewer, sid, false, now);
    machine
        .apply(ConsentEvent::SasCheck {
            reported_sas: sas,
            now,
        })
        .expect("sas verified");

    let expired_at = now + Duration::seconds(CONSENT_TTL_SECS);
    machine
        .apply(ConsentEvent::Tick { now: expired_at })
        .expect("tick transitions to expired");
    assert!(matches!(
        machine.state(),
        ConsentState::DeniedLogged(DenialReason::RequestExpired)
    ));

    let grant = SessionGrant::issue(
        &host,
        sid,
        viewer.public_key(),
        "support.example.org",
        GrantPermissions::default(),
        GrantDuration::ThisSession,
        expired_at,
    )
    .expect("grant created");
    assert!(matches!(
        machine.apply(ConsentEvent::HostAllow {
            grant,
            host_pubkey: host.public_key(),
            idle_timeout_secs: None,
            now: expired_at,
        }),
        Err(ProtocolError::InvalidTransition { .. })
    ));
}

#[test]
fn negative_first_time_cooling_off_blocks_early_allow() {
    let host = IdentityKeyPair::generate();
    let viewer = IdentityKeyPair::generate();
    let now = Utc::now();
    let mut machine = ConsentMachine::new();
    let sid = Uuid::now_v7();

    let sas = setup_consented_context(&mut machine, &host, &viewer, sid, true, now);
    assert!(matches!(machine.state(), ConsentState::CoolingOff5s(_)));

    let early = now + Duration::seconds(COOLING_OFF_SECS - 1);
    machine
        .apply(ConsentEvent::SasCheck {
            reported_sas: sas,
            now: early,
        })
        .expect("sas matched while still cooling off");
    assert!(matches!(machine.state(), ConsentState::CoolingOff5s(_)));

    let early_grant = SessionGrant::issue(
        &host,
        sid,
        viewer.public_key(),
        "support.example.org",
        GrantPermissions::default(),
        GrantDuration::ThisSession,
        early,
    )
    .expect("grant created");
    assert_eq!(
        machine.apply(ConsentEvent::HostAllow {
            grant: early_grant,
            host_pubkey: host.public_key(),
            idle_timeout_secs: None,
            now: early,
        }),
        Err(ProtocolError::CoolingOffActive)
    );

    let ready = now + Duration::seconds(COOLING_OFF_SECS);
    machine
        .apply(ConsentEvent::Tick { now: ready })
        .expect("cooling off finishes");
    assert!(matches!(
        machine.state(),
        ConsentState::AwaitingHostConsent(_)
    ));

    let ready_grant = SessionGrant::issue(
        &host,
        sid,
        viewer.public_key(),
        "support.example.org",
        GrantPermissions::default(),
        GrantDuration::ThisSession,
        ready,
    )
    .expect("grant created");
    machine
        .apply(ConsentEvent::HostAllow {
            grant: ready_grant,
            host_pubkey: host.public_key(),
            idle_timeout_secs: None,
            now: ready,
        })
        .expect("allow succeeds after cooling off");
    assert!(matches!(
        machine.state(),
        ConsentState::GrantSignedActive(_)
    ));
}

#[test]
fn negative_sas_mismatch_fails_closed_and_locks_out_after_three() {
    let host = IdentityKeyPair::generate();
    let viewer = IdentityKeyPair::generate();
    let now = Utc::now();
    let mut machine = ConsentMachine::new();

    for attempt in 1..=3 {
        let sid = Uuid::now_v7();
        let sas = setup_consented_context(&mut machine, &host, &viewer, sid, false, now);
        let wrong_sas = if sas == "000000" { "111111" } else { "000000" };
        let res = machine.apply(ConsentEvent::SasCheck {
            reported_sas: wrong_sas.to_owned(),
            now,
        });
        assert_eq!(res, Err(ProtocolError::SasMismatch));
        assert_eq!(machine.sas_mismatch_count(), attempt);
        assert!(matches!(
            machine.state(),
            ConsentState::DeniedLogged(DenialReason::SasMismatch)
        ));
        machine
            .apply(ConsentEvent::AcknowledgeTerminal)
            .expect("ack back to idle");
    }

    assert!(machine.is_locked_out());
    let locked = machine.apply(ConsentEvent::RequestIncoming {
        session_id: Uuid::now_v7(),
        viewer_pubkey: viewer.public_key(),
        server_domain: "support.example.org".to_owned(),
        authenticated_requester: true,
        server_enrolled: true,
        rate_limit_passed: true,
        host_key_matches_pin: true,
        first_time_requester: false,
        now,
    });
    assert_eq!(locked, Err(ProtocolError::RateLimitLockedOut));
    assert!(matches!(
        machine.state(),
        ConsentState::BlockedAlert(BlockedReason::UnauthorizedOrRateLimited)
    ));
}

#[test]
fn negative_key_change_and_unknown_server_block_immediately() {
    let viewer = IdentityKeyPair::generate();
    let now = Utc::now();

    let mut machine_key = ConsentMachine::new();
    let err_key = machine_key.apply(ConsentEvent::RequestIncoming {
        session_id: Uuid::now_v7(),
        viewer_pubkey: viewer.public_key(),
        server_domain: "support.example.org".to_owned(),
        authenticated_requester: true,
        server_enrolled: true,
        rate_limit_passed: true,
        host_key_matches_pin: false,
        first_time_requester: false,
        now,
    });
    assert_eq!(err_key, Err(ProtocolError::HostKeyChanged));
    assert!(matches!(
        machine_key.state(),
        ConsentState::BlockedAlert(BlockedReason::HostKeyChanged)
    ));

    let mut machine_srv = ConsentMachine::new();
    let err_srv = machine_srv.apply(ConsentEvent::RequestIncoming {
        session_id: Uuid::now_v7(),
        viewer_pubkey: viewer.public_key(),
        server_domain: "rogue.example.org".to_owned(),
        authenticated_requester: true,
        server_enrolled: false,
        rate_limit_passed: true,
        host_key_matches_pin: true,
        first_time_requester: false,
        now,
    });
    assert_eq!(err_srv, Err(ProtocolError::ServerNotEnrolled));
    assert!(matches!(
        machine_srv.state(),
        ConsentState::BlockedAlert(BlockedReason::UnknownServer)
    ));
}

#[test]
fn negative_revoke_and_mid_session_toggle_and_control_plane_offline() {
    let host = IdentityKeyPair::generate();
    let viewer = IdentityKeyPair::generate();
    let now = Utc::now();
    let mut machine = ConsentMachine::new();
    let sid = Uuid::now_v7();

    // Control plane offline before grant fails closed
    let sas = setup_consented_context(&mut machine, &host, &viewer, sid, false, now);
    machine
        .apply(ConsentEvent::ControlPlaneOffline)
        .expect("fails closed before grant");
    assert!(matches!(
        machine.state(),
        ConsentState::DeniedLogged(DenialReason::ControlPlaneOffline)
    ));
    machine
        .apply(ConsentEvent::AcknowledgeTerminal)
        .expect("back to idle");

    // Start valid active session with mouse_keyboard enabled
    let sid2 = Uuid::now_v7();
    let sas2 = setup_consented_context(&mut machine, &host, &viewer, sid2, false, now);
    assert_ne!(sas, "");
    machine
        .apply(ConsentEvent::SasCheck {
            reported_sas: sas2,
            now,
        })
        .expect("sas verified");

    let grant_with_input = SessionGrant::issue(
        &host,
        sid2,
        viewer.public_key(),
        "support.example.org",
        GrantPermissions {
            view_screen: true,
            mouse_keyboard: true,
            ..GrantPermissions::default()
        },
        GrantDuration::OneHour,
        now,
    )
    .expect("grant issued");
    machine
        .apply(ConsentEvent::HostAllow {
            grant: grant_with_input.clone(),
            host_pubkey: host.public_key(),
            idle_timeout_secs: Some(900),
            now,
        })
        .expect("active");

    assert!(machine.can_forward_frame(FrameKind::MouseKeyboard, now));
    assert!(!machine.can_forward_frame(FrameKind::FileTransfer, now));
    assert!(!machine.can_forward_frame(FrameKind::BlankScreen, now));

    // Active P2P session survives control-plane outage
    machine
        .apply(ConsentEvent::ControlPlaneOffline)
        .expect("active session continues");
    assert!(machine.can_forward_frame(FrameKind::ScreenView, now));

    // Owner revokes mouse_keyboard mid-session -> immediately enforced
    let tightened = grant_with_input
        .reissue_with_permissions(&host, GrantPermissions::default(), now)
        .expect("reissued");
    machine
        .apply(ConsentEvent::HostUpdatePermissions {
            updated_grant: tightened,
            now,
        })
        .expect("updated");
    assert!(matches!(
        machine.state(),
        ConsentState::GrantUpdatedImmediately(_)
    ));
    assert!(!machine.can_forward_frame(FrameKind::MouseKeyboard, now));
    assert!(machine.can_forward_frame(FrameKind::ScreenView, now));

    // End session revokes immediately
    machine
        .apply(ConsentEvent::RevokeSession {
            reason: RevocationReason::HostEnded,
        })
        .expect("revoked");
    assert!(matches!(
        machine.state(),
        ConsentState::RevokedLogged(RevocationReason::HostEnded)
    ));
    assert!(!machine.can_forward_frame(FrameKind::ScreenView, now));
}

#[test]
fn grant_updated_immediately_settles_with_idle_timeout_and_revokes_on_idle() {
    let host = IdentityKeyPair::generate();
    let viewer = IdentityKeyPair::generate();
    let now = Utc::now();
    let mut machine = ConsentMachine::new();
    let sid = Uuid::now_v7();

    let sas = setup_consented_context(&mut machine, &host, &viewer, sid, false, now);
    machine
        .apply(ConsentEvent::SasCheck {
            reported_sas: sas,
            now,
        })
        .expect("sas verified");

    let grant = SessionGrant::issue(
        &host,
        sid,
        viewer.public_key(),
        "support.example.org",
        GrantPermissions {
            view_screen: true,
            mouse_keyboard: true,
            ..GrantPermissions::default()
        },
        GrantDuration::OneHour,
        now,
    )
    .expect("grant issued");
    machine
        .apply(ConsentEvent::HostAllow {
            grant: grant.clone(),
            host_pubkey: host.public_key(),
            idle_timeout_secs: Some(900),
            now,
        })
        .expect("active");

    let tightened = grant
        .reissue_with_permissions(&host, GrantPermissions::default(), now)
        .expect("reissued");
    machine
        .apply(ConsentEvent::HostUpdatePermissions {
            updated_grant: tightened,
            now,
        })
        .expect("updated");
    assert!(matches!(
        machine.state(),
        ConsentState::GrantUpdatedImmediately(_)
    ));

    let before_idle = now + Duration::seconds(10);
    machine
        .apply(ConsentEvent::Tick { now: before_idle })
        .expect("settles while active");
    assert!(matches!(
        machine.state(),
        ConsentState::GrantSignedActive(_)
    ));
    assert!(machine.can_forward_frame(FrameKind::ScreenView, before_idle));

    let at_idle = now + Duration::seconds(900);
    machine
        .apply(ConsentEvent::Tick { now: at_idle })
        .expect("idle timeout revokes");
    assert!(matches!(
        machine.state(),
        ConsentState::RevokedLogged(RevocationReason::IdleTimeout)
    ));
    assert!(!machine.can_forward_frame(FrameKind::ScreenView, at_idle));
}
