//! Pure Rust Consent State Machine with strict fail-closed invariants.
//!
//! State graph:
//! `IDLE -> VERIFYING_IDENTITY -> (BLOCKED_ALERT | AWAITING_HOST_CONSENT)`
//! `AWAITING_HOST_CONSENT -> (COOLING_OFF_5S -> AWAITING_HOST_CONSENT | DENIED_LOGGED -> IDLE | GRANT_SIGNED_ACTIVE)`
//! `GRANT_SIGNED_ACTIVE -> (GRANT_UPDATED_IMMEDIATELY -> GRANT_SIGNED_ACTIVE | REVOKED_LOGGED -> IDLE)`

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::ProtocolError;
use crate::grant::{FrameKind, SessionGrant};
use crate::handshake::MAX_SAS_MISMATCHES;
use crate::identity::IdentityPublicKey;

/// Hard 2-minute consent prompt TTL in seconds.
pub const CONSENT_TTL_SECS: i64 = 120;

/// Mandatory 5-second cooling-off delay for first-time requesters.
pub const COOLING_OFF_SECS: i64 = 5;

/// Context gathered when verifying an incoming request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifyingContext {
    /// Unique session ID.
    pub session_id: Uuid,
    /// Authenticated requester Ed25519 key.
    pub viewer_pubkey: IdentityPublicKey,
    /// Pinned enrolled server domain.
    pub server_domain: String,
    /// Whether this requester is a first-time requester to this host.
    pub first_time_requester: bool,
    /// Timestamp request entered verification.
    pub requested_at: DateTime<Utc>,
}

/// Active consent prompt context (2-minute TTL, optional 5-second cooling-off).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsentContext {
    /// Unique session ID.
    pub session_id: Uuid,
    /// Authenticated requester Ed25519 key.
    pub viewer_pubkey: IdentityPublicKey,
    /// Pinned enrolled server domain.
    pub server_domain: String,
    /// Locally derived 6-digit SAS code.
    pub derived_sas: String,
    /// Whether both sides confirmed matching 6-digit SAS.
    pub sas_verified: bool,
    /// Whether requester is first-time (requires 5s cooling-off).
    pub first_time_requester: bool,
    /// Whether the 5s cooling-off requirement has completed.
    pub cooling_off_satisfied: bool,
    /// Timestamp when prompt started.
    pub prompt_started_at: DateTime<Utc>,
    /// Hard 2-minute TTL deadline.
    pub consent_expires_at: DateTime<Utc>,
}

/// Context for an active, host-authorized session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveSessionContext {
    /// Verified host-signed grant.
    pub grant: SessionGrant,
    /// Host Ed25519 public key that signed the grant.
    pub host_pubkey: IdentityPublicKey,
    /// Whether the persistent on-screen `End session` banner is mounted.
    pub banner_mounted: bool,
    /// Optional idle timeout duration in seconds.
    pub idle_timeout_secs: Option<i64>,
    /// Timestamp of last activity.
    pub last_activity_at: DateTime<Utc>,
}

/// Security-blocking alert reasons.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlockedReason {
    /// Pinned host key changed.
    HostKeyChanged,
    /// Server public key is not enrolled on this host.
    UnknownServer,
    /// Requester is not authenticated or failed rate limits.
    UnauthorizedOrRateLimited,
}

/// Reasons a pending request was denied and logged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DenialReason {
    /// Host owner clicked `Deny`.
    OwnerDenied,
    /// Hard 2-minute consent TTL expired.
    RequestExpired,
    /// 6-digit SAS code mismatch.
    SasMismatch,
    /// Control plane became unreachable before grant signing.
    ControlPlaneOffline,
}

/// Reasons an active session was revoked and logged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RevocationReason {
    /// Host owner clicked `End session`.
    HostEnded,
    /// Viewer disconnected cleanly.
    ViewerDisconnected,
    /// Idle timeout elapsed.
    IdleTimeout,
    /// Grant UTC expiry reached.
    GrantExpired,
    /// Host key change or security policy abort.
    SecurityAbort,
}

/// Explicit state machine states matching the security specification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConsentState {
    /// No pending or active session.
    Idle,
    /// Verifying requester authentication, enrolled server, rate limit, and key pin.
    VerifyingIdentity(VerifyingContext),
    /// Halted due to key mismatch, unknown server, or rate limit.
    BlockedAlert(BlockedReason),
    /// Consent card shown (`520x660`); 2-minute countdown running.
    AwaitingHostConsent(ConsentContext),
    /// First-time requester 5-second cooling-off (`Allow` disabled).
    CoolingOff5s(ConsentContext),
    /// Request denied/expired/mismatched and recorded in audit log.
    DeniedLogged(DenialReason),
    /// Active session backed by a valid host-signed [`SessionGrant`].
    GrantSignedActive(ActiveSessionContext),
    /// Owner toggled permissions mid-session; updated grant enforced immediately.
    GrantUpdatedImmediately(ActiveSessionContext),
    /// Session ended/revoked and recorded in audit log.
    RevokedLogged(RevocationReason),
}

impl ConsentState {
    /// Returns a static label for error reporting and diagnostics.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Idle => "IDLE",
            Self::VerifyingIdentity(_) => "VERIFYING_IDENTITY",
            Self::BlockedAlert(_) => "BLOCKED_ALERT",
            Self::AwaitingHostConsent(_) => "AWAITING_HOST_CONSENT",
            Self::CoolingOff5s(_) => "COOLING_OFF_5S",
            Self::DeniedLogged(_) => "DENIED_LOGGED",
            Self::GrantSignedActive(_) => "GRANT_SIGNED_ACTIVE",
            Self::GrantUpdatedImmediately(_) => "GRANT_UPDATED_IMMEDIATELY",
            Self::RevokedLogged(_) => "REVOKED_LOGGED",
        }
    }
}

/// Inputs driving the consent state machine.
#[derive(Debug, Clone)]
pub enum ConsentEvent {
    /// Incoming connection request from signaling.
    RequestIncoming {
        /// Unique session ID.
        session_id: Uuid,
        /// Requester public key.
        viewer_pubkey: IdentityPublicKey,
        /// Server domain presenting the request.
        server_domain: String,
        /// Whether requester auth succeeded.
        authenticated_requester: bool,
        /// Whether the server's public key is explicitly enrolled on this host.
        server_enrolled: bool,
        /// Whether rate-limiting allows this request.
        rate_limit_passed: bool,
        /// Whether the host key matches the viewer's pinned TOFU key.
        host_key_matches_pin: bool,
        /// Whether this requester has never connected to this host before.
        first_time_requester: bool,
        /// Current UTC time.
        now: DateTime<Utc>,
    },
    /// Key exchange succeeded and local 6-digit SAS was derived.
    HandshakeVerified {
        /// Locally derived 6-digit SAS.
        derived_sas: String,
        /// Current UTC time.
        now: DateTime<Utc>,
    },
    /// Viewer reported 6-digit SAS comparison result.
    SasCheck {
        /// 6-digit SAS entered/read by peer.
        reported_sas: String,
        /// Current UTC time.
        now: DateTime<Utc>,
    },
    /// Periodic time tick for cooling-off completion, 2-min TTL, idle timeout, or grant expiry.
    Tick {
        /// Current UTC time.
        now: DateTime<Utc>,
    },
    /// Host owner clicked `Allow` with a host-signed [`SessionGrant`].
    HostAllow {
        /// Host-signed grant.
        grant: SessionGrant,
        /// Expected host public key.
        host_pubkey: IdentityPublicKey,
        /// Optional idle timeout in seconds.
        idle_timeout_secs: Option<i64>,
        /// Current UTC time.
        now: DateTime<Utc>,
    },
    /// Host owner clicked `Deny` (or control plane dropped before grant).
    HostDeny {
        /// Denial reason.
        reason: DenialReason,
    },
    /// Host owner toggled permissions mid-session.
    HostUpdatePermissions {
        /// Re-signed updated grant.
        updated_grant: SessionGrant,
        /// Current UTC time.
        now: DateTime<Utc>,
    },
    /// Control plane went offline before a grant was signed (fail closed).
    ControlPlaneOffline,
    /// Host key mismatch detected mid-flow.
    HostKeyMismatch,
    /// End session / revoke active grant.
    RevokeSession {
        /// Revocation reason.
        reason: RevocationReason,
    },
    /// Acknowledge terminal state (`DENIED_LOGGED`, `REVOKED_LOGGED`, `BLOCKED_ALERT`) back to `IDLE`.
    AcknowledgeTerminal,
}

/// Pure consent state machine enforcing fail-closed transitions and host grant gating.
#[derive(Debug, Clone)]
pub struct ConsentMachine {
    state: ConsentState,
    sas_mismatch_count: u32,
    locked_out: bool,
}

impl Default for ConsentMachine {
    fn default() -> Self {
        Self::new()
    }
}

impl ConsentMachine {
    /// Creates a new state machine in `IDLE`.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            state: ConsentState::Idle,
            sas_mismatch_count: 0,
            locked_out: false,
        }
    }

    /// Returns the current [`ConsentState`].
    #[must_use]
    pub const fn state(&self) -> &ConsentState {
        &self.state
    }

    /// Returns whether the requester is locked out after repeated SAS mismatches.
    #[must_use]
    pub const fn is_locked_out(&self) -> bool {
        self.locked_out
    }

    /// Returns consecutive SAS mismatch count.
    #[must_use]
    pub const fn sas_mismatch_count(&self) -> u32 {
        self.sas_mismatch_count
    }

    /// Resets mismatch lockout (only via explicit admin/owner action).
    pub fn reset_lockout(&mut self) {
        self.sas_mismatch_count = 0;
        self.locked_out = false;
    }

    /// Applies an event to the state machine, failing closed on any invalid transition
    /// or security invariant violation.
    ///
    /// # Errors
    /// Returns [`ProtocolError`] if the event is disallowed or security verification fails.
    pub fn apply(&mut self, event: ConsentEvent) -> Result<&ConsentState, ProtocolError> {
        match event {
            ConsentEvent::RequestIncoming {
                session_id,
                viewer_pubkey,
                server_domain,
                authenticated_requester,
                server_enrolled,
                rate_limit_passed,
                host_key_matches_pin,
                first_time_requester,
                now,
            } => self.handle_request_incoming(
                session_id,
                viewer_pubkey,
                server_domain,
                authenticated_requester,
                server_enrolled,
                rate_limit_passed,
                host_key_matches_pin,
                first_time_requester,
                now,
            ),
            ConsentEvent::HandshakeVerified { derived_sas, now } => {
                self.handle_handshake_verified(derived_sas, now)
            }
            ConsentEvent::SasCheck { reported_sas, now } => {
                self.handle_sas_check(&reported_sas, now)
            }
            ConsentEvent::Tick { now } => Ok(self.handle_tick(now)),
            ConsentEvent::HostAllow {
                grant,
                host_pubkey,
                idle_timeout_secs,
                now,
            } => self.handle_host_allow(grant, host_pubkey, idle_timeout_secs, now),
            ConsentEvent::HostDeny { reason } => self.handle_host_deny(reason),
            ConsentEvent::HostUpdatePermissions { updated_grant, now } => {
                self.handle_update_permissions(updated_grant, now)
            }
            ConsentEvent::ControlPlaneOffline => self.handle_control_plane_offline(),
            ConsentEvent::HostKeyMismatch => {
                self.state = match &self.state {
                    ConsentState::GrantSignedActive(_)
                    | ConsentState::GrantUpdatedImmediately(_) => {
                        ConsentState::RevokedLogged(RevocationReason::SecurityAbort)
                    }
                    _ => ConsentState::BlockedAlert(BlockedReason::HostKeyChanged),
                };
                Ok(&self.state)
            }
            ConsentEvent::RevokeSession { reason } => self.handle_revoke(reason),
            ConsentEvent::AcknowledgeTerminal => self.handle_acknowledge(),
        }
    }

    /// Host protocol-boundary gatekeeper: returns `true` ONLY when in an active grant state
    /// AND the signed [`SessionGrant`] explicitly permits `frame` at `now`.
    #[must_use]
    pub fn can_forward_frame(&self, frame: FrameKind, now: DateTime<Utc>) -> bool {
        match &self.state {
            ConsentState::GrantSignedActive(ctx) | ConsentState::GrantUpdatedImmediately(ctx) => {
                ctx.banner_mounted
                    && ctx
                        .grant
                        .verify(
                            &ctx.host_pubkey,
                            ctx.grant.session_id,
                            &ctx.grant.viewer_pubkey,
                            &ctx.grant.server_domain,
                            now,
                        )
                        .is_ok()
                    && ctx.grant.allows_frame(frame, now)
            }
            _ => false,
        }
    }

    #[allow(clippy::too_many_arguments, clippy::fn_params_excessive_bools)]
    fn handle_request_incoming(
        &mut self,
        session_id: Uuid,
        viewer_pubkey: IdentityPublicKey,
        server_domain: String,
        authenticated_requester: bool,
        server_enrolled: bool,
        rate_limit_passed: bool,
        host_key_matches_pin: bool,
        first_time_requester: bool,
        now: DateTime<Utc>,
    ) -> Result<&ConsentState, ProtocolError> {
        if !matches!(self.state, ConsentState::Idle) {
            return Err(ProtocolError::InvalidTransition {
                state: self.state.name(),
                event: "RequestIncoming",
            });
        }
        if !host_key_matches_pin {
            self.state = ConsentState::BlockedAlert(BlockedReason::HostKeyChanged);
            return Err(ProtocolError::HostKeyChanged);
        }
        if !server_enrolled {
            self.state = ConsentState::BlockedAlert(BlockedReason::UnknownServer);
            return Err(ProtocolError::ServerNotEnrolled);
        }
        if !authenticated_requester {
            self.state = ConsentState::BlockedAlert(BlockedReason::UnauthorizedOrRateLimited);
            return Err(ProtocolError::UnauthenticatedRequester);
        }
        if self.locked_out || !rate_limit_passed {
            self.state = ConsentState::BlockedAlert(BlockedReason::UnauthorizedOrRateLimited);
            return Err(ProtocolError::RateLimitLockedOut);
        }

        self.state = ConsentState::VerifyingIdentity(VerifyingContext {
            session_id,
            viewer_pubkey,
            server_domain,
            first_time_requester,
            requested_at: now,
        });
        Ok(&self.state)
    }

    fn handle_handshake_verified(
        &mut self,
        derived_sas: String,
        now: DateTime<Utc>,
    ) -> Result<&ConsentState, ProtocolError> {
        let ctx = match &self.state {
            ConsentState::VerifyingIdentity(v) => v.clone(),
            _ => {
                return Err(ProtocolError::InvalidTransition {
                    state: self.state.name(),
                    event: "HandshakeVerified",
                });
            }
        };

        if derived_sas.len() != 6 || !derived_sas.bytes().all(|b| b.is_ascii_digit()) {
            self.state = ConsentState::DeniedLogged(DenialReason::SasMismatch);
            return Err(ProtocolError::SasMismatch);
        }

        let consent_ctx = ConsentContext {
            session_id: ctx.session_id,
            viewer_pubkey: ctx.viewer_pubkey,
            server_domain: ctx.server_domain,
            derived_sas,
            sas_verified: false,
            first_time_requester: ctx.first_time_requester,
            cooling_off_satisfied: !ctx.first_time_requester,
            prompt_started_at: now,
            consent_expires_at: now + Duration::seconds(CONSENT_TTL_SECS),
        };

        self.state = if ctx.first_time_requester {
            ConsentState::CoolingOff5s(consent_ctx)
        } else {
            ConsentState::AwaitingHostConsent(consent_ctx)
        };
        Ok(&self.state)
    }

    fn handle_sas_check(
        &mut self,
        reported_sas: &str,
        now: DateTime<Utc>,
    ) -> Result<&ConsentState, ProtocolError> {
        let mut ctx = match &self.state {
            ConsentState::AwaitingHostConsent(c) | ConsentState::CoolingOff5s(c) => c.clone(),
            _ => {
                return Err(ProtocolError::InvalidTransition {
                    state: self.state.name(),
                    event: "SasCheck",
                });
            }
        };

        if now >= ctx.consent_expires_at {
            self.state = ConsentState::DeniedLogged(DenialReason::RequestExpired);
            return Err(ProtocolError::ConsentTtlExpired);
        }

        if reported_sas != ctx.derived_sas {
            self.sas_mismatch_count = self.sas_mismatch_count.saturating_add(1);
            if self.sas_mismatch_count >= MAX_SAS_MISMATCHES {
                self.locked_out = true;
            }
            self.state = ConsentState::DeniedLogged(DenialReason::SasMismatch);
            return Err(ProtocolError::SasMismatch);
        }

        ctx.sas_verified = true;
        if let ConsentState::CoolingOff5s(_) = self.state {
            if now >= ctx.prompt_started_at + Duration::seconds(COOLING_OFF_SECS) {
                ctx.cooling_off_satisfied = true;
                self.state = ConsentState::AwaitingHostConsent(ctx);
            } else {
                self.state = ConsentState::CoolingOff5s(ctx);
            }
        } else {
            self.state = ConsentState::AwaitingHostConsent(ctx);
        }
        Ok(&self.state)
    }

    fn handle_tick(&mut self, now: DateTime<Utc>) -> &ConsentState {
        match &self.state {
            ConsentState::CoolingOff5s(c) => {
                if now >= c.consent_expires_at {
                    self.state = ConsentState::DeniedLogged(DenialReason::RequestExpired);
                } else if now >= c.prompt_started_at + Duration::seconds(COOLING_OFF_SECS) {
                    let mut next = c.clone();
                    next.cooling_off_satisfied = true;
                    self.state = ConsentState::AwaitingHostConsent(next);
                }
                &self.state
            }
            ConsentState::AwaitingHostConsent(c) => {
                if now >= c.consent_expires_at {
                    self.state = ConsentState::DeniedLogged(DenialReason::RequestExpired);
                }
                &self.state
            }
            ConsentState::GrantSignedActive(a) | ConsentState::GrantUpdatedImmediately(a) => {
                if now >= a.grant.expires_at_utc {
                    self.state = ConsentState::RevokedLogged(RevocationReason::GrantExpired);
                } else if let Some(idle_secs) = a.idle_timeout_secs {
                    if now >= a.last_activity_at + Duration::seconds(idle_secs) {
                        self.state = ConsentState::RevokedLogged(RevocationReason::IdleTimeout);
                    }
                } else if matches!(self.state, ConsentState::GrantUpdatedImmediately(_)) {
                    self.state = ConsentState::GrantSignedActive(a.clone());
                }
                &self.state
            }
            _ => &self.state,
        }
    }

    fn handle_host_allow(
        &mut self,
        grant: SessionGrant,
        host_pubkey: IdentityPublicKey,
        idle_timeout_secs: Option<i64>,
        now: DateTime<Utc>,
    ) -> Result<&ConsentState, ProtocolError> {
        let mut ctx = match &self.state {
            ConsentState::AwaitingHostConsent(c) => c.clone(),
            ConsentState::CoolingOff5s(c) => {
                if now < c.prompt_started_at + Duration::seconds(COOLING_OFF_SECS) {
                    return Err(ProtocolError::CoolingOffActive);
                }
                let mut ready = c.clone();
                ready.cooling_off_satisfied = true;
                ready
            }
            _ => {
                return Err(ProtocolError::InvalidTransition {
                    state: self.state.name(),
                    event: "HostAllow",
                });
            }
        };

        if now >= ctx.consent_expires_at {
            self.state = ConsentState::DeniedLogged(DenialReason::RequestExpired);
            return Err(ProtocolError::ConsentTtlExpired);
        }
        if !ctx.cooling_off_satisfied {
            return Err(ProtocolError::CoolingOffActive);
        }
        if !ctx.sas_verified {
            return Err(ProtocolError::SasNotVerified);
        }

        // Verify cryptographic host signature & bindings before transitioning to active.
        grant.verify(
            &host_pubkey,
            ctx.session_id,
            &ctx.viewer_pubkey,
            &ctx.server_domain,
            now,
        )?;

        ctx.cooling_off_satisfied = true;
        self.sas_mismatch_count = 0;
        self.state = ConsentState::GrantSignedActive(ActiveSessionContext {
            grant,
            host_pubkey,
            banner_mounted: true,
            idle_timeout_secs,
            last_activity_at: now,
        });
        Ok(&self.state)
    }

    fn handle_host_deny(&mut self, reason: DenialReason) -> Result<&ConsentState, ProtocolError> {
        match &self.state {
            ConsentState::VerifyingIdentity(_)
            | ConsentState::AwaitingHostConsent(_)
            | ConsentState::CoolingOff5s(_) => {
                self.state = ConsentState::DeniedLogged(reason);
                Ok(&self.state)
            }
            _ => Err(ProtocolError::InvalidTransition {
                state: self.state.name(),
                event: "HostDeny",
            }),
        }
    }

    fn handle_update_permissions(
        &mut self,
        updated_grant: SessionGrant,
        now: DateTime<Utc>,
    ) -> Result<&ConsentState, ProtocolError> {
        let mut active = match &self.state {
            ConsentState::GrantSignedActive(a) | ConsentState::GrantUpdatedImmediately(a) => {
                a.clone()
            }
            _ => {
                return Err(ProtocolError::InvalidTransition {
                    state: self.state.name(),
                    event: "HostUpdatePermissions",
                });
            }
        };

        updated_grant.verify(
            &active.host_pubkey,
            active.grant.session_id,
            &active.grant.viewer_pubkey,
            &active.grant.server_domain,
            now,
        )?;

        active.grant = updated_grant;
        active.last_activity_at = now;
        self.state = ConsentState::GrantUpdatedImmediately(active);
        Ok(&self.state)
    }

    fn handle_control_plane_offline(&mut self) -> Result<&ConsentState, ProtocolError> {
        match &self.state {
            // Fail-closed: no new sessions may start if control plane drops before grant signing.
            ConsentState::VerifyingIdentity(_)
            | ConsentState::AwaitingHostConsent(_)
            | ConsentState::CoolingOff5s(_) => {
                self.state = ConsentState::DeniedLogged(DenialReason::ControlPlaneOffline);
                Ok(&self.state)
            }
            // Active P2P sessions survive control-plane outages until revoked or expired.
            ConsentState::GrantSignedActive(_) | ConsentState::GrantUpdatedImmediately(_) => {
                Ok(&self.state)
            }
            _ => Err(ProtocolError::InvalidTransition {
                state: self.state.name(),
                event: "ControlPlaneOffline",
            }),
        }
    }

    fn handle_revoke(&mut self, reason: RevocationReason) -> Result<&ConsentState, ProtocolError> {
        match &self.state {
            ConsentState::GrantSignedActive(_) | ConsentState::GrantUpdatedImmediately(_) => {
                self.state = ConsentState::RevokedLogged(reason);
                Ok(&self.state)
            }
            _ => Err(ProtocolError::InvalidTransition {
                state: self.state.name(),
                event: "RevokeSession",
            }),
        }
    }

    fn handle_acknowledge(&mut self) -> Result<&ConsentState, ProtocolError> {
        match &self.state {
            ConsentState::BlockedAlert(_)
            | ConsentState::DeniedLogged(_)
            | ConsentState::RevokedLogged(_) => {
                self.state = ConsentState::Idle;
                Ok(&self.state)
            }
            _ => Err(ProtocolError::InvalidTransition {
                state: self.state.name(),
                event: "AcknowledgeTerminal",
            }),
        }
    }
}
