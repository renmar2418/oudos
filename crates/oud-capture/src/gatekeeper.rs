//! Host-side protocol-boundary frame gatekeeper wired to `oud_protocol::SessionGrant`
//! and [`oud_protocol::ConsentMachine`].

use chrono::{DateTime, Duration, Utc};
use oud_protocol::grant::{FrameKind, SessionGrant};
use oud_protocol::identity::IdentityPublicKey;
use oud_protocol::state_machine::{ConsentEvent, ConsentMachine, ConsentState, RevocationReason};
use oud_protocol::ProtocolError;
use uuid::Uuid;

use crate::error::CaptureError;

/// Enforces an active host-signed [`SessionGrant`] at the capture/input boundary.
///
/// Every outbound screen frame and inbound input/clipboard/file frame MUST pass
/// [`HostFrameGatekeeper::authorize_frame`] before touching OS APIs or media tracks.
#[derive(Debug, Clone)]
pub struct HostFrameGatekeeper {
    active_grant: Option<SessionGrant>,
    consent_machine: Option<ConsentMachine>,
    expected_host_pubkey: IdentityPublicKey,
    expected_session_id: Uuid,
    expected_viewer_pubkey: IdentityPublicKey,
    expected_server_domain: String,
    banner_mounted: bool,
    idle_timeout_secs: Option<i64>,
    last_activity_at: Option<DateTime<Utc>>,
    revoked_permanently: bool,
    allowed_frames: u64,
    dropped_frames: u64,
    rejected_malformed_payloads: u64,
}

impl HostFrameGatekeeper {
    /// Creates a new gatekeeper initialized with a verified host-signed [`SessionGrant`].
    ///
    /// Defaults to persistent `End session` banner mounted (`true`) and no idle timeout.
    /// Chain [`HostFrameGatekeeper::with_idle_timeout`] or construct via
    /// [`HostFrameGatekeeper::from_consent_machine`] to attach session idle policies.
    ///
    /// # Errors
    /// Fails closed if the grant fails signature, binding, expiry, or `v0.1` Asks-First
    /// checks (`blank_screen` must be `false`, `view_screen` must be `true`).
    pub fn from_verified_grant(
        grant: SessionGrant,
        expected_host_pubkey: IdentityPublicKey,
        expected_session_id: Uuid,
        expected_viewer_pubkey: IdentityPublicKey,
        expected_server_domain: impl Into<String>,
        now: DateTime<Utc>,
    ) -> Result<Self, CaptureError> {
        let server_domain = expected_server_domain.into();
        grant.verify(
            &expected_host_pubkey,
            expected_session_id,
            &expected_viewer_pubkey,
            &server_domain,
            now,
        )?;
        enforce_v01_grant_caps(&grant)?;

        Ok(Self {
            active_grant: Some(grant),
            consent_machine: None,
            expected_host_pubkey,
            expected_session_id,
            expected_viewer_pubkey,
            expected_server_domain: server_domain,
            banner_mounted: true,
            idle_timeout_secs: None,
            last_activity_at: Some(now),
            revoked_permanently: false,
            allowed_frames: 0,
            dropped_frames: 0,
            rejected_malformed_payloads: 0,
        })
    }

    /// Creates a gatekeeper directly backed by an active [`ConsentMachine`] (`GRANT_SIGNED_ACTIVE`
    /// or `GRANT_UPDATED_IMMEDIATELY`), preserving `banner_mounted` and `idle_timeout_secs` 1-to-1.
    ///
    /// # Errors
    /// Fails closed if `machine` is not in an active grant state or the underlying grant
    /// fails live signature or `v0.1` policy verification at `now`.
    pub fn from_consent_machine(
        mut machine: ConsentMachine,
        now: DateTime<Utc>,
    ) -> Result<Self, CaptureError> {
        let _ = machine.apply(ConsentEvent::Tick { now });
        let (grant, host_pubkey, banner_mounted, idle_timeout_secs, last_activity_at) =
            match machine.state() {
                ConsentState::GrantSignedActive(ctx)
                | ConsentState::GrantUpdatedImmediately(ctx) => (
                    ctx.grant.clone(),
                    ctx.host_pubkey,
                    ctx.banner_mounted,
                    ctx.idle_timeout_secs,
                    ctx.last_activity_at,
                ),
                other => {
                    return Err(CaptureError::GrantError(ProtocolError::InvalidTransition {
                        state: other.name(),
                        event: "HostFrameGatekeeper::from_consent_machine",
                    }));
                }
            };

        grant.verify(
            &host_pubkey,
            grant.session_id,
            &grant.viewer_pubkey,
            &grant.server_domain,
            now,
        )?;
        enforce_v01_grant_caps(&grant)?;

        let expected_session_id = grant.session_id;
        let expected_viewer_pubkey = grant.viewer_pubkey;
        let expected_server_domain = grant.server_domain.clone();

        Ok(Self {
            active_grant: Some(grant),
            consent_machine: Some(machine),
            expected_host_pubkey: host_pubkey,
            expected_session_id,
            expected_viewer_pubkey,
            expected_server_domain,
            banner_mounted,
            idle_timeout_secs,
            last_activity_at: Some(last_activity_at),
            revoked_permanently: false,
            allowed_frames: 0,
            dropped_frames: 0,
            rejected_malformed_payloads: 0,
        })
    }

    /// Creates an ungranted gatekeeper that drops all frames until a valid grant activates.
    #[must_use]
    pub fn ungranted(
        expected_host_pubkey: IdentityPublicKey,
        expected_session_id: Uuid,
        expected_viewer_pubkey: IdentityPublicKey,
        expected_server_domain: impl Into<String>,
    ) -> Self {
        Self {
            active_grant: None,
            consent_machine: None,
            expected_host_pubkey,
            expected_session_id,
            expected_viewer_pubkey,
            expected_server_domain: expected_server_domain.into(),
            banner_mounted: true,
            idle_timeout_secs: None,
            last_activity_at: None,
            revoked_permanently: false,
            allowed_frames: 0,
            dropped_frames: 0,
            rejected_malformed_payloads: 0,
        }
    }

    /// Attaches an idle timeout threshold in seconds to a standalone gatekeeper.
    #[must_use]
    pub const fn with_idle_timeout(mut self, idle_timeout_secs: Option<i64>) -> Self {
        self.idle_timeout_secs = idle_timeout_secs;
        self
    }

    /// Sets whether the persistent on-screen `End session` banner is mounted.
    ///
    /// When `false`, [`HostFrameGatekeeper::authorize_frame`] fails closed for all frame kinds,
    /// matching [`ConsentMachine::can_forward_frame`].
    pub fn set_banner_mounted(&mut self, mounted: bool) {
        self.banner_mounted = mounted;
    }

    /// Records explicit session interaction at `now`, resetting the idle-timeout clock
    /// only if the session is still active and has not already reached idle timeout.
    pub fn record_activity(&mut self, now: DateTime<Utc>) {
        if self.revoked_permanently || self.active_grant.is_none() {
            return;
        }
        if let Some(machine) = &mut self.consent_machine {
            let _ = machine.apply(ConsentEvent::Tick { now });
            if !matches!(
                machine.state(),
                ConsentState::GrantSignedActive(_) | ConsentState::GrantUpdatedImmediately(_)
            ) {
                self.revoke_internal(RevocationReason::IdleTimeout);
                return;
            }
        }
        if is_idle_timed_out(self.idle_timeout_secs, self.last_activity_at, now) {
            self.revoke_internal(RevocationReason::IdleTimeout);
            return;
        }
        if self.last_activity_at.is_none_or(|prev| now > prev) {
            self.last_activity_at = Some(now);
        }
    }

    /// Records a malformed inbound payload or unexpected viewer data channel rejection.
    pub fn record_rejected_payload(&mut self) {
        self.dropped_frames = self.dropped_frames.saturating_add(1);
        self.rejected_malformed_payloads = self.rejected_malformed_payloads.saturating_add(1);
    }

    /// Activates or updates the in-memory [`SessionGrant`] immediately.
    ///
    /// # Errors
    /// Returns [`CaptureError::GrantError`] if the session is already revoked or the grant
    /// fails cryptographic or `v0.1` checks.
    pub fn update_grant(
        &mut self,
        grant: SessionGrant,
        now: DateTime<Utc>,
    ) -> Result<(), CaptureError> {
        if self.revoked_permanently {
            return Err(CaptureError::GrantError(ProtocolError::InvalidTransition {
                state: "REVOKED_LOGGED",
                event: "HostUpdatePermissions",
            }));
        }

        if let Some(machine) = &mut self.consent_machine {
            let _ = machine.apply(ConsentEvent::Tick { now });
            machine.apply(ConsentEvent::HostUpdatePermissions {
                updated_grant: grant.clone(),
                now,
            })?;
            enforce_v01_grant_caps(&grant)?;
            self.active_grant = Some(grant);
            self.last_activity_at = Some(now);
            return Ok(());
        }

        if self.active_grant.is_some()
            && is_idle_timed_out(self.idle_timeout_secs, self.last_activity_at, now)
        {
            self.revoke_internal(RevocationReason::IdleTimeout);
            return Err(CaptureError::GrantError(ProtocolError::InvalidTransition {
                state: "REVOKED_LOGGED",
                event: "HostUpdatePermissions",
            }));
        }

        grant.verify(
            &self.expected_host_pubkey,
            self.expected_session_id,
            &self.expected_viewer_pubkey,
            &self.expected_server_domain,
            now,
        )?;
        enforce_v01_grant_caps(&grant)?;
        self.active_grant = Some(grant);
        self.last_activity_at = Some(now);
        Ok(())
    }

    /// Revokes the active grant immediately so all subsequent frames fail closed.
    pub fn revoke(&mut self) {
        self.revoke_internal(RevocationReason::HostEnded);
    }

    fn revoke_internal(&mut self, reason: RevocationReason) {
        self.active_grant = None;
        self.revoked_permanently = true;
        if let Some(machine) = &mut self.consent_machine {
            let _ = machine.apply(ConsentEvent::RevokeSession { reason });
        }
    }

    /// Checks whether `frame` is permitted under the active signed grant, banner-mount
    /// invariant, idle-timeout deadline, and live cryptographic verification at `now`.
    ///
    /// Matches [`ConsentMachine::can_forward_frame`] and `Tick` transitions 1-to-1.
    ///
    /// # Errors
    /// Returns [`CaptureError::FrameBlockedByGrant`] if no active grant exists, the grant
    /// signature or binding fails verification, the persistent banner is unmounted,
    /// idle timeout elapsed, the grant expired, or the permission flag is `false`.
    pub fn authorize_frame(
        &mut self,
        frame: FrameKind,
        now: DateTime<Utc>,
    ) -> Result<(), CaptureError> {
        if self.revoked_permanently {
            return self.record_drop(frame);
        }

        if let Some(machine) = &mut self.consent_machine {
            let _ = machine.apply(ConsentEvent::Tick { now });
            match machine.state() {
                ConsentState::GrantSignedActive(ctx)
                | ConsentState::GrantUpdatedImmediately(ctx) => {
                    if !self.banner_mounted
                        || !ctx.banner_mounted
                        || !machine.can_forward_frame(frame, now)
                    {
                        return self.record_drop(frame);
                    }
                    self.active_grant = Some(ctx.grant.clone());
                    self.idle_timeout_secs = ctx.idle_timeout_secs;
                }
                _ => {
                    self.active_grant = None;
                    self.revoked_permanently = true;
                    return self.record_drop(frame);
                }
            }
        }

        let Some(grant) = self.active_grant.clone() else {
            return self.record_drop(frame);
        };

        if !self.banner_mounted {
            return self.record_drop(frame);
        }

        if is_idle_timed_out(self.idle_timeout_secs, self.last_activity_at, now) {
            self.revoke_internal(RevocationReason::IdleTimeout);
            return self.record_drop(frame);
        }

        if let Err(err) = grant.verify(
            &self.expected_host_pubkey,
            self.expected_session_id,
            &self.expected_viewer_pubkey,
            &self.expected_server_domain,
            now,
        ) {
            if matches!(err, ProtocolError::GrantExpired) {
                self.revoke_internal(RevocationReason::GrantExpired);
            }
            return self.record_drop(frame);
        }

        if enforce_v01_grant_caps(&grant).is_err() {
            return self.record_drop(frame);
        }

        if grant.allows_frame(frame, now) {
            self.allowed_frames = self.allowed_frames.saturating_add(1);
            Ok(())
        } else {
            self.record_drop(frame)
        }
    }

    fn record_drop(&mut self, frame: FrameKind) -> Result<(), CaptureError> {
        self.dropped_frames = self.dropped_frames.saturating_add(1);
        Err(CaptureError::FrameBlockedByGrant {
            frame_kind: frame_label(frame),
        })
    }

    /// Returns a reference to the active [`SessionGrant`], if any.
    #[must_use]
    pub const fn active_grant(&self) -> Option<&SessionGrant> {
        self.active_grant.as_ref()
    }

    /// Returns the expected host identity public key bound to this gatekeeper.
    #[must_use]
    pub const fn expected_host_pubkey(&self) -> IdentityPublicKey {
        self.expected_host_pubkey
    }

    /// Returns the expected session UUID bound to this gatekeeper.
    #[must_use]
    pub const fn expected_session_id(&self) -> Uuid {
        self.expected_session_id
    }

    /// Returns the expected viewer identity public key bound to this gatekeeper.
    #[must_use]
    pub const fn expected_viewer_pubkey(&self) -> IdentityPublicKey {
        self.expected_viewer_pubkey
    }

    /// Returns the expected enrolled server domain bound to this gatekeeper.
    #[must_use]
    pub fn expected_server_domain(&self) -> &str {
        &self.expected_server_domain
    }

    /// Returns whether the persistent `End session` banner is currently mounted.
    #[must_use]
    pub const fn banner_mounted(&self) -> bool {
        self.banner_mounted
    }

    /// Returns the configured idle timeout duration in seconds, if any.
    #[must_use]
    pub const fn idle_timeout_secs(&self) -> Option<i64> {
        self.idle_timeout_secs
    }

    /// Returns the wrapped [`ConsentMachine`], if this gatekeeper was created from one.
    #[must_use]
    pub const fn consent_machine(&self) -> Option<&ConsentMachine> {
        self.consent_machine.as_ref()
    }

    /// Returns count of allowed frames passed through this gatekeeper.
    #[must_use]
    pub const fn allowed_frames(&self) -> u64 {
        self.allowed_frames
    }

    /// Returns count of unauthorized, expired, malformed, or unmounted frames dropped.
    #[must_use]
    pub const fn dropped_frames(&self) -> u64 {
        self.dropped_frames
    }

    /// Returns count of malformed `"oud-input"` payloads or unexpected remote data channels rejected.
    #[must_use]
    pub const fn rejected_malformed_payloads(&self) -> u64 {
        self.rejected_malformed_payloads
    }
}

fn is_idle_timed_out(
    idle_timeout_secs: Option<i64>,
    last_activity_at: Option<DateTime<Utc>>,
    now: DateTime<Utc>,
) -> bool {
    match (idle_timeout_secs, last_activity_at) {
        (Some(idle_secs), Some(last_active)) => Duration::try_seconds(idle_secs)
            .and_then(|d| last_active.checked_add_signed(d))
            .is_none_or(|deadline| now >= deadline),
        _ => false,
    }
}

fn enforce_v01_grant_caps(grant: &SessionGrant) -> Result<(), ProtocolError> {
    if !grant.permissions.view_screen {
        return Err(ProtocolError::V01PolicyViolation(
            "view_screen must be true for active session",
        ));
    }
    if grant.permissions.blank_screen {
        return Err(ProtocolError::V01PolicyViolation(
            "blank_screen is prohibited in v0.1 Asks-First mode",
        ));
    }
    Ok(())
}

const fn frame_label(frame: FrameKind) -> &'static str {
    match frame {
        FrameKind::ScreenView => "screen_view",
        FrameKind::MouseKeyboard => "mouse_keyboard",
        FrameKind::ClipboardSync => "clipboard_sync",
        FrameKind::FileTransfer => "file_transfer",
        FrameKind::BlankScreen => "blank_screen",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oud_protocol::grant::{GrantDuration, GrantPermissions};
    use oud_protocol::handshake::{InitiatorPending, ResponderPending};
    use oud_protocol::identity::{IdentityKeyPair, IdentitySignature};

    fn active_consent_machine(
        host: &IdentityKeyPair,
        viewer: &IdentityKeyPair,
        session_id: Uuid,
        idle_timeout_secs: Option<i64>,
        now: DateTime<Utc>,
    ) -> (ConsentMachine, SessionGrant) {
        let (init, offer) = InitiatorPending::initiate(session_id, "support.example.org", viewer);
        let (resp, accept) = ResponderPending::respond(&offer, host).expect("offer verifies");
        let (reveal, viewer_out) = init
            .complete(&accept, Some(&host.public_key()))
            .expect("accept verifies");
        let host_out = resp
            .verify_reveal_and_finalize(&reveal)
            .expect("reveal verifies");
        assert_eq!(viewer_out.sas_code, host_out.sas_code);

        let mut machine = ConsentMachine::new();
        machine
            .apply(ConsentEvent::RequestIncoming {
                session_id,
                viewer_pubkey: viewer.public_key(),
                server_domain: "support.example.org".to_owned(),
                authenticated_requester: true,
                server_enrolled: true,
                rate_limit_passed: true,
                host_key_matches_pin: true,
                first_time_requester: false,
                now,
            })
            .expect("verifying");
        machine
            .apply(ConsentEvent::HandshakeVerified {
                derived_sas: host_out.sas_code.clone(),
                now,
            })
            .expect("awaiting consent");
        machine
            .apply(ConsentEvent::SasCheck {
                reported_sas: host_out.sas_code.clone(),
                now,
            })
            .expect("sas verified");

        let grant = SessionGrant::issue(
            host,
            session_id,
            viewer.public_key(),
            "support.example.org",
            GrantPermissions {
                view_screen: true,
                mouse_keyboard: true,
                clipboard_sync: false,
                file_transfer: false,
                blank_screen: false,
            },
            GrantDuration::OneHour,
            now,
        )
        .expect("grant issued");

        machine
            .apply(ConsentEvent::HostAllow {
                grant: grant.clone(),
                host_pubkey: host.public_key(),
                idle_timeout_secs,
                now,
            })
            .expect("active");

        (machine, grant)
    }

    #[test]
    fn authorize_frame_performs_live_signature_verification() {
        let host = IdentityKeyPair::generate();
        let viewer = IdentityKeyPair::generate().public_key();
        let sid = Uuid::now_v7();
        let now = Utc::now();

        let grant = SessionGrant::issue(
            &host,
            sid,
            viewer,
            "support.example.org",
            GrantPermissions::default(),
            GrantDuration::OneHour,
            now,
        )
        .expect("valid grant");

        let mut gk = HostFrameGatekeeper::from_verified_grant(
            grant,
            host.public_key(),
            sid,
            viewer,
            "support.example.org",
            now,
        )
        .expect("gatekeeper initializes");

        assert!(gk.authorize_frame(FrameKind::ScreenView, now).is_ok());

        // Corrupt cached grant signature in-memory to prove live verification happens per frame.
        if let Some(active) = gk.active_grant.as_mut() {
            let mut sig_bytes = *active.host_signature.as_bytes();
            sig_bytes[0] ^= 0xFF;
            active.host_signature = IdentitySignature::from_bytes(sig_bytes);
        }

        assert!(matches!(
            gk.authorize_frame(FrameKind::ScreenView, now),
            Err(CaptureError::FrameBlockedByGrant {
                frame_kind: "screen_view"
            })
        ));
        assert_eq!(gk.dropped_frames(), 1);
    }

    #[test]
    fn unmounted_banner_blocks_all_frames_fail_closed() {
        let host = IdentityKeyPair::generate();
        let viewer = IdentityKeyPair::generate().public_key();
        let sid = Uuid::now_v7();
        let now = Utc::now();

        let grant = SessionGrant::issue(
            &host,
            sid,
            viewer,
            "support.example.org",
            GrantPermissions {
                view_screen: true,
                mouse_keyboard: true,
                clipboard_sync: true,
                file_transfer: true,
                blank_screen: false,
            },
            GrantDuration::OneHour,
            now,
        )
        .expect("grant issued");

        let mut gk = HostFrameGatekeeper::from_verified_grant(
            grant,
            host.public_key(),
            sid,
            viewer,
            "support.example.org",
            now,
        )
        .expect("gatekeeper");

        assert!(gk.authorize_frame(FrameKind::ScreenView, now).is_ok());
        gk.set_banner_mounted(false);

        for kind in [
            FrameKind::ScreenView,
            FrameKind::MouseKeyboard,
            FrameKind::ClipboardSync,
            FrameKind::FileTransfer,
        ] {
            assert!(
                gk.authorize_frame(kind, now).is_err(),
                "unmounted persistent banner must block {kind:?}"
            );
        }
        assert_eq!(gk.dropped_frames(), 4);

        gk.set_banner_mounted(true);
        assert!(gk.authorize_frame(FrameKind::ScreenView, now).is_ok());
    }

    #[test]
    fn idle_timeout_revokes_permanently_and_matches_consent_machine_parity() {
        let host = IdentityKeyPair::generate();
        let viewer = IdentityKeyPair::generate();
        let sid = Uuid::now_v7();
        let now = Utc::now();

        let (machine, grant) = active_consent_machine(&host, &viewer, sid, Some(900), now);
        let mut gk =
            HostFrameGatekeeper::from_consent_machine(machine, now).expect("wrapped machine");

        let before_idle = now + Duration::seconds(899);
        assert!(gk
            .authorize_frame(FrameKind::ScreenView, before_idle)
            .is_ok());
        assert!(gk
            .authorize_frame(FrameKind::MouseKeyboard, before_idle)
            .is_ok());

        // Passive ScreenView at >= 900s without activity triggers idle timeout + permanent revoke.
        let at_idle = now + Duration::seconds(900);
        assert!(gk.authorize_frame(FrameKind::ScreenView, at_idle).is_err());
        assert!(gk.active_grant().is_none());
        assert!(matches!(
            gk.consent_machine().map(ConsentMachine::state),
            Some(ConsentState::RevokedLogged(RevocationReason::IdleTimeout))
        ));

        // Subsequent attempts and activity calls cannot resurrect a revoked session.
        gk.record_activity(at_idle);
        assert!(gk.authorize_frame(FrameKind::ScreenView, at_idle).is_err());
        assert!(gk.update_grant(grant, at_idle).is_err());
    }

    #[test]
    fn standalone_gatekeeper_idle_timeout_and_record_activity_work_consistently() {
        let host = IdentityKeyPair::generate();
        let viewer = IdentityKeyPair::generate().public_key();
        let sid = Uuid::now_v7();
        let now = Utc::now();

        let grant = SessionGrant::issue(
            &host,
            sid,
            viewer,
            "support.example.org",
            GrantPermissions::default(),
            GrantDuration::OneHour,
            now,
        )
        .expect("grant issued");

        let mut gk = HostFrameGatekeeper::from_verified_grant(
            grant,
            host.public_key(),
            sid,
            viewer,
            "support.example.org",
            now,
        )
        .expect("gatekeeper")
        .with_idle_timeout(Some(60));

        // Recording genuine interaction at +50s pushes deadline to +110s.
        let active_at = now + Duration::seconds(50);
        gk.record_activity(active_at);
        assert!(gk
            .authorize_frame(FrameKind::ScreenView, now + Duration::seconds(109))
            .is_ok());

        // At +110s, session times out and stays revoked.
        let deadline = now + Duration::seconds(110);
        assert!(gk.authorize_frame(FrameKind::ScreenView, deadline).is_err());
        assert!(gk.active_grant().is_none());
    }

    #[test]
    fn non_active_consent_machine_is_rejected() {
        let machine = ConsentMachine::new();
        assert!(HostFrameGatekeeper::from_consent_machine(machine, Utc::now()).is_err());
    }
}
