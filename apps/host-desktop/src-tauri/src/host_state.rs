//! Host-side Tauri application state wiring `oud-protocol`, `oud-policy`, and `oud-capture`.
//!
//! Enforces:
//! - Handshake-derived 6-digit SAS (never server-issued).
//! - Mandatory 5-second cooling-off timer for first-time requesters.
//! - Monotonic policy ceilings & `v0.1` Asks-First bounds (`ThisSession` / `OneHour` only).
//! - Local-only `End session` grant revocation (`HostFrameGatekeeper::revoke()`).
//! - Protected consent/banner surface rect blocking + synthetic input audit logging.

use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Duration, Utc};
use oud_capture::{
    CaptureError, ClipboardAuditMetadata, ClipboardEndpoint, ClipboardSyncManager,
    CompletedFileTransfer, FileTransferLimits, FileTransferManager, FileTransferProgress,
    HostFrameGatekeeper, HostInboundMessage, ProtectedSurfaceKind, ProtectedSurfaceRegistry, Rect,
};
use oud_policy::{merge_policies, EffectivePolicy, PolicyLayer};
use oud_protocol::audit::{AuditChain, AuditEvent, AuditEventKind};
use oud_protocol::grant::{ClipboardDirection, GrantDuration, GrantPermissions, SessionGrant};
use oud_protocol::handshake::{InitiatorPending, ResponderPending};
use oud_protocol::identity::{IdentityKeyPair, IdentityPublicKey};
use oud_protocol::state_machine::{
    BlockedReason, ConsentEvent, ConsentMachine, ConsentState, DenialReason, RevocationReason,
    CONSENT_TTL_SECS, COOLING_OFF_SECS,
};
use oud_protocol::ProtocolError;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::tray::TrayVisualState;

/// Protected consent window width in logical pixels.
pub const CONSENT_CARD_WIDTH: u32 = 520;
/// Protected consent window height in logical pixels.
pub const CONSENT_CARD_HEIGHT: u32 = 660;
/// Maximum display length for a sanitized requester identity label.
pub const MAX_REQUESTER_NAME_CHARS: usize = 48;
/// Maximum ASCII length for a validated `server_domain` hostname.
pub const MAX_SERVER_DOMAIN_CHARS: usize = 253;
/// Maximum consecutive denials/SAS mismatches before automatic host cooldown lockout.
pub const HOST_MAX_DENIAL_STREAK: u8 = 3;
/// Automatic host cooldown lockout duration in seconds (15 minutes).
pub const HOST_LOCKOUT_DURATION_SECS: i64 = 900;
/// Hard maximum lifetime in seconds for one-time support links (15 minutes).
pub const SUPPORT_LINK_MAX_TTL_SECS: i64 = 900;

/// Input parameters for recording a local host abuse report.
#[derive(Debug, Clone, Copy)]
pub struct HostAbuseReportParams<'a> {
    /// Offending requester Ed25519 public key.
    pub requester_pubkey: IdentityPublicKey,
    /// Raw server domain.
    pub server_domain: &'a str,
    /// Abuse category (`"impersonation"`, `"harassment"`, `"scam_attempt"`, `"other"`).
    pub category: &'a str,
    /// Plain-text incident summary (metadata only).
    pub summary: &'a str,
    /// Whether to automatically block the requester on this host.
    pub also_block: bool,
}

/// Host-side metadata-only abuse report record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostAbuseReport {
    /// Report identifier (`UUIDv7`).
    pub report_id: String,
    /// Offending requester Ed25519 public key.
    pub requester_pubkey: String,
    /// Abuse category (`"impersonation"`, `"harassment"`, `"scam_attempt"`, `"other"`).
    pub category: String,
    /// Plain-text summary (no secrets/payloads).
    pub summary: String,
    /// Whether the requester was also blocked.
    pub blocked: bool,
    /// UTC creation timestamp.
    pub created_at_utc: String,
}

/// Window close policy evaluated when the OS requests closing `"banner"` or `"consent"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowCloseAction {
    /// Prevent closing and keep the window visible (active persistent banner).
    PreventAndKeepVisible,
    /// Prevent destroying the window and hide it back to tray.
    PreventAndHide,
    /// Prevent destroying the consent window, hide it, and fail closed (`OwnerDenied`).
    PreventHideAndDenyPending,
    /// Allow close for unrelated windows.
    AllowClose,
}

/// Decides how Tauri's `WindowEvent::CloseRequested` must respond for protected windows.
#[must_use]
pub fn decide_window_close_action(
    window_label: &str,
    has_pending_consent: bool,
    has_active_banner: bool,
) -> WindowCloseAction {
    match window_label {
        "banner" => {
            if has_active_banner {
                WindowCloseAction::PreventAndKeepVisible
            } else {
                WindowCloseAction::PreventAndHide
            }
        }
        "consent" => {
            if has_pending_consent {
                WindowCloseAction::PreventHideAndDenyPending
            } else {
                WindowCloseAction::PreventAndHide
            }
        }
        _ => WindowCloseAction::AllowClose,
    }
}

/// Origin of a user-facing action on the host consent card or session banner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InputOrigin {
    /// Physical local host user interaction.
    LocalHost,
    /// Remote peer or synthetic/injected input attempt (always rejected on consent/banner).
    RemoteOrSynthetic,
}

/// Requested or selected permission flags on the host consent card.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostPermissionSelection {
    /// Required screen view permission (`true`).
    pub view_screen: bool,
    /// Mouse & keyboard control (defaults to `false`).
    pub mouse_keyboard: bool,
    /// Clipboard sync (defaults to `false`).
    pub clipboard_sync: bool,
    /// File transfer (defaults to `false`).
    pub file_transfer: bool,
}

impl Default for HostPermissionSelection {
    fn default() -> Self {
        Self {
            view_screen: true,
            mouse_keyboard: false,
            clipboard_sync: false,
            file_transfer: false,
        }
    }
}

impl From<HostPermissionSelection> for GrantPermissions {
    fn from(value: HostPermissionSelection) -> Self {
        Self {
            view_screen: true,
            mouse_keyboard: value.mouse_keyboard,
            clipboard_sync: value.clipboard_sync,
            file_transfer: value.file_transfer,
            blank_screen: false, // Locked OFF in v0.1 Asks-First mode.
        }
    }
}

/// Monotonic policy limits exposed to the consent card UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyCeilingView {
    /// Whether policy allows granting mouse & keyboard control.
    pub allow_mouse_keyboard: bool,
    /// Whether policy allows granting clipboard sync.
    pub allow_clipboard_sync: bool,
    /// Whether policy allows granting file transfer.
    pub allow_file_transfer: bool,
    /// Human-readable idle timeout label.
    pub idle_timeout_label: String,
}

/// View model for an active `520x660` host consent prompt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostConsentView {
    /// Session identifier (`UUIDv7`).
    pub session_id: String,
    /// Authenticated requester display name / email.
    pub requester_name: String,
    /// Requester Ed25519 public key (`ed25519:...`).
    pub requester_pubkey: String,
    /// Raw enrolled server domain (or `"Personal / Unverified"`).
    pub server_domain: String,
    /// Whether the presenting server public key is enrolled on this host.
    pub server_enrolled: bool,
    /// Locally derived 6-digit SAS formatted `XXX XXX`.
    pub sas_code: String,
    /// Whether requester is connecting for the first time (triggers 5s cooling-off).
    pub first_time_requester: bool,
    /// Remaining mandatory cooling-off seconds (`0..=5`).
    pub cooling_off_remaining_secs: i64,
    /// Remaining hard 2-minute consent countdown seconds (`0..=120`).
    pub ttl_remaining_secs: i64,
    /// Current permission toggles (defaults to screen-only).
    pub permissions: HostPermissionSelection,
    /// Effective monotonic policy ceilings.
    pub policy_ceilings: PolicyCeilingView,
    /// Selected `v0.1` bounded duration (`this_session` or `one_hour`).
    pub duration: GrantDuration,
}

/// View model for the persistent top-most `End session` banner.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActiveBannerView {
    /// Active session identifier (`UUIDv7`).
    pub session_id: String,
    /// Authenticated requester display name.
    pub requester_name: String,
    /// Raw server domain rendered beside requester identity.
    pub server_domain: String,
    /// Whether the server key is enrolled.
    pub server_enrolled: bool,
    /// Plain-text live indicator (`"LIVE"`).
    pub status_label: String,
    /// Active host-enforced permissions.
    pub permissions: GrantPermissions,
    /// Colour-independent summary (`"Screen only"`, `"Screen + Input"`, etc.).
    pub permission_summary: String,
    /// Selected `v0.1` duration (`this_session` or `one_hour`).
    pub duration: GrantDuration,
    /// Grant expiry UTC RFC 3339 timestamp.
    pub expires_at_utc: String,
}

/// Metadata-only audit entry summary for host visibility.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditSummaryView {
    /// Monotonic audit sequence number.
    pub sequence: u64,
    /// Timestamp in RFC 3339 UTC.
    pub timestamp_utc: String,
    /// Raw server domain.
    pub server_domain: String,
    /// Plain-English summary of the metadata event kind.
    pub summary: String,
}

/// Complete snapshot returned to the Tauri frontend views (`consent` & `banner`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostSnapshot {
    /// Current monochrome Facing Brackets tray state (`idle`, `in_session`, `alert`).
    pub tray_state: TrayVisualState,
    /// Underlying `ConsentState` label (`IDLE`, `COOLING_OFF_5S`, `GRANT_SIGNED_ACTIVE`, etc.).
    pub consent_state_name: String,
    /// Active consent prompt data when awaiting owner decision.
    pub pending_request: Option<HostConsentView>,
    /// Persistent banner data when `SessionGrant` is active.
    pub active_banner: Option<ActiveBannerView>,
    /// Blocking alert banner text if an unknown server, key mismatch, or lockout occurred.
    pub alert_message: Option<String>,
    /// Total entries recorded in the host Ed25519 hash-chained `AuditChain`.
    pub audit_entry_count: usize,
    /// Total synthetic/protected-rect input attempts blocked.
    pub blocked_input_count: u64,
    /// Total unauthorized/dropped frames blocked by `HostFrameGatekeeper`.
    pub dropped_frame_count: u64,
    /// Recent metadata-only audit events.
    pub recent_audit: Vec<AuditSummaryView>,
}

/// Parameters describing a verified incoming connection request presented to the host owner.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug)]
pub struct IncomingRequestParams {
    /// Unique session ID (`UUIDv7`).
    pub session_id: Uuid,
    /// Authenticated requester identity label.
    pub requester_name: String,
    /// Requester keypair (viewer peer identity) used to run the real X25519+Ed25519 handshake.
    pub viewer_keys: IdentityKeyPair,
    /// Raw domain of the control plane presenting this request.
    pub server_domain: String,
    /// Whether the server's public key is enrolled on this host.
    pub server_enrolled: bool,
    /// Whether the viewer's pinned host key matches.
    pub host_key_matches_pin: bool,
    /// Whether requester passed control-plane authentication.
    pub authenticated_requester: bool,
    /// Whether requester passed rate limits.
    pub rate_limit_passed: bool,
    /// Whether this requester has never been approved on this host before.
    pub first_time_requester: bool,
    /// Permissions requested by the viewer (host defaults stay screen-only until toggled).
    pub requested_permissions: HostPermissionSelection,
}

#[derive(Debug, Clone)]
struct PendingSessionInternal {
    session_id: Uuid,
    requester_name: String,
    viewer_pubkey: IdentityPublicKey,
    server_domain: String,
    server_enrolled: bool,
    derived_sas: String,
    first_time_requester: bool,
    prompt_started_at: DateTime<Utc>,
    consent_expires_at: DateTime<Utc>,
    permissions: HostPermissionSelection,
    duration: GrantDuration,
}

#[derive(Debug, Clone)]
struct ActiveSessionInternal {
    session_id: Uuid,
    requester_name: String,
    viewer_pubkey: IdentityPublicKey,
    server_domain: String,
    server_enrolled: bool,
    started_at: DateTime<Utc>,
    grant: SessionGrant,
}

/// Core host desktop state machine, grant gatekeeper, protected rect registry, and audit chain.
pub struct HostDesktopState {
    host_keys: IdentityKeyPair,
    effective_policy: EffectivePolicy,
    consent_machine: ConsentMachine,
    gatekeeper: Option<HostFrameGatekeeper>,
    protected_surfaces: ProtectedSurfaceRegistry,
    clipboard_manager: ClipboardSyncManager,
    file_transfer_manager: FileTransferManager,
    blocked_requesters: HashSet<IdentityPublicKey>,
    denial_streaks: HashMap<IdentityPublicKey, (u8, Option<DateTime<Utc>>)>,
    abuse_reports: Vec<HostAbuseReport>,
    audit_chain: AuditChain,
    pending: Option<PendingSessionInternal>,
    active: Option<ActiveSessionInternal>,
    tray_state: TrayVisualState,
    alert_message: Option<String>,
}

impl HostDesktopState {
    /// Initializes a new host agent state with default monotonic policies and `Idle` tray state.
    #[must_use]
    pub fn new(
        host_keys: IdentityKeyPair,
        org: &PolicyLayer,
        group: &PolicyLayer,
        owner: &PolicyLayer,
    ) -> Self {
        let effective_policy = merge_policies(org, group, owner);
        let ft_limits = FileTransferLimits {
            max_file_size_bytes: effective_policy.rules.max_file_size_bytes,
            ..FileTransferLimits::default()
        };
        Self {
            host_keys,
            effective_policy,
            consent_machine: ConsentMachine::new(),
            gatekeeper: None,
            protected_surfaces: ProtectedSurfaceRegistry::new(),
            clipboard_manager: ClipboardSyncManager::default(),
            file_transfer_manager: FileTransferManager::new(
                ft_limits,
                std::sync::Arc::new(oud_capture::NoopFileScanHook),
            ),
            blocked_requesters: HashSet::new(),
            denial_streaks: HashMap::new(),
            abuse_reports: Vec::new(),
            audit_chain: AuditChain::new(),
            pending: None,
            active: None,
            tray_state: TrayVisualState::Idle,
            alert_message: None,
        }
    }

    /// Initializes a host state with baseline `v0.1` default policies (`allow_file_transfer = false`,
    /// `idle_timeout = 30m`, `ask_before_connection = true`).
    #[must_use]
    pub fn with_default_policy(host_keys: IdentityKeyPair) -> Self {
        Self::new(
            host_keys,
            &PolicyLayer::default(),
            &PolicyLayer::default(),
            &PolicyLayer::default(),
        )
    }

    /// Returns the host's Ed25519 identity public key.
    #[must_use]
    pub fn host_pubkey(&self) -> IdentityPublicKey {
        self.host_keys.public_key()
    }

    /// Returns a clone of the shared [`ProtectedSurfaceRegistry`].
    #[must_use]
    pub fn protected_surfaces(&self) -> ProtectedSurfaceRegistry {
        self.protected_surfaces.clone()
    }

    /// Returns a reference to the host's tamper-evident [`AuditChain`].
    #[must_use]
    pub const fn audit_chain(&self) -> &AuditChain {
        &self.audit_chain
    }

    /// Returns a reference to the active [`HostFrameGatekeeper`], if any.
    #[must_use]
    pub const fn gatekeeper(&self) -> Option<&HostFrameGatekeeper> {
        self.gatekeeper.as_ref()
    }

    /// Presents an incoming request after running a genuine commit-before-reveal X25519 + Ed25519
    /// handshake (`oud_protocol::handshake`) to derive the 6-digit SAS locally on both peers.
    ///
    /// # Errors
    /// Fails closed on security blocks (unknown server, host key mismatch, rate limit) or
    /// invalid state transitions, updating tray state to `Alert` and appending audit records.
    pub fn receive_incoming_request(
        &mut self,
        params: &IncomingRequestParams,
        now: DateTime<Utc>,
    ) -> Result<HostSnapshot, CaptureError> {
        if matches!(
            self.consent_machine.state(),
            ConsentState::BlockedAlert(_)
                | ConsentState::DeniedLogged(_)
                | ConsentState::RevokedLogged(_)
        ) {
            let _ = self
                .consent_machine
                .apply(ConsentEvent::AcknowledgeTerminal);
        }

        let viewer_pubkey = params.viewer_keys.public_key();
        let server_domain = match validate_server_domain(&params.server_domain) {
            Ok(d) => d,
            Err(err) => {
                self.tray_state = TrayVisualState::Alert;
                self.alert_message = Some(err.to_string());
                return Err(err);
            }
        };
        let requester_name = match sanitize_requester_name(&params.requester_name) {
            Ok(name) => name,
            Err(err) => {
                self.tray_state = TrayVisualState::Alert;
                self.alert_message = Some(err.to_string());
                return Err(err);
            }
        };

        let requester_allowed =
            params.rate_limit_passed && !self.is_requester_blocked_or_locked(&viewer_pubkey, now);
        let req_res = self.consent_machine.apply(ConsentEvent::RequestIncoming {
            session_id: params.session_id,
            viewer_pubkey,
            server_domain: server_domain.clone(),
            authenticated_requester: params.authenticated_requester,
            server_enrolled: params.server_enrolled,
            rate_limit_passed: requester_allowed,
            host_key_matches_pin: params.host_key_matches_pin,
            first_time_requester: params.first_time_requester,
            now,
        });

        if let Err(err) = req_res {
            self.tray_state = TrayVisualState::Alert;
            self.alert_message = Some(err.to_string());
            if let ConsentState::BlockedAlert(reason) = *self.consent_machine.state() {
                let _ = self.append_audit(
                    params.session_id,
                    viewer_pubkey,
                    &server_domain,
                    AuditEventKind::BlockedSecurityAlert { reason },
                    now,
                );
            }
            return Err(CaptureError::GrantError(err));
        }

        let _ = self.append_audit(
            params.session_id,
            viewer_pubkey,
            &server_domain,
            AuditEventKind::SessionRequested,
            now,
        );

        // Execute genuine local commit-before-reveal X25519 + Ed25519 key agreement for SAS.
        let (init, offer) =
            InitiatorPending::initiate(params.session_id, &server_domain, &params.viewer_keys);
        let (resp, accept) = ResponderPending::respond(&offer, &self.host_keys)?;
        let (reveal, viewer_out) = init.complete(&accept, Some(&self.host_keys.public_key()))?;
        let host_out = resp.verify_reveal_and_finalize(&reveal)?;
        if viewer_out.sas_code != host_out.sas_code {
            return Err(CaptureError::GrantError(ProtocolError::SasMismatch));
        }

        self.consent_machine
            .apply(ConsentEvent::HandshakeVerified {
                derived_sas: host_out.sas_code.clone(),
                now,
            })?;
        self.consent_machine.apply(ConsentEvent::SasCheck {
            reported_sas: viewer_out.sas_code.clone(),
            now,
        })?;

        // Screen view is required; optional toggles ALWAYS start OFF by default on the host card.
        let _ = params.requested_permissions;
        self.pending = Some(PendingSessionInternal {
            session_id: params.session_id,
            requester_name,
            viewer_pubkey,
            server_domain,
            server_enrolled: params.server_enrolled,
            derived_sas: host_out.sas_code.clone(),
            first_time_requester: params.first_time_requester,
            prompt_started_at: now,
            consent_expires_at: now + Duration::seconds(CONSENT_TTL_SECS),
            permissions: HostPermissionSelection::default(),
            duration: GrantDuration::ThisSession,
        });
        self.alert_message = None;
        self.tray_state = TrayVisualState::Idle;

        Ok(self.snapshot(now))
    }

    /// Advances consent/cooling-off/session timers at `now` and flushes any hook-blocked audit events.
    pub fn tick(&mut self, now: DateTime<Utc>) -> HostSnapshot {
        self.flush_pending_synthetic_audit();
        let prev_is_pending = self.pending.is_some();
        let prev_is_active = self.active.is_some();

        let _ = self.consent_machine.apply(ConsentEvent::Tick { now });

        match *self.consent_machine.state() {
            ConsentState::DeniedLogged(reason) => {
                if prev_is_pending {
                    if let Some(p) = self.pending.take() {
                        if matches!(
                            reason,
                            DenialReason::OwnerDenied | DenialReason::SasMismatch
                        ) {
                            self.record_requester_denial(p.viewer_pubkey, now);
                        }
                        let _ = self.append_audit(
                            p.session_id,
                            p.viewer_pubkey,
                            &p.server_domain,
                            AuditEventKind::ConsentDenied { reason },
                            now,
                        );
                    }
                }
                self.protected_surfaces
                    .clear_surface(ProtectedSurfaceKind::ConsentPrompt);
                self.tray_state = TrayVisualState::Idle;
                self.alert_message = Some(match reason {
                    DenialReason::RequestExpired => {
                        "Request expired after the 2-minute window. Nothing was shared.".to_owned()
                    }
                    DenialReason::OwnerDenied => "You declined the connection request.".to_owned(),
                    DenialReason::SasMismatch => {
                        "6-digit codes did not match; connection stopped.".to_owned()
                    }
                    DenialReason::ControlPlaneOffline => {
                        "Control plane went offline before approval; connection stopped.".to_owned()
                    }
                });
            }
            ConsentState::RevokedLogged(reason) if prev_is_active => {
                self.finalize_revocation_audit(reason, now);
            }
            _ => {}
        }

        self.snapshot(now)
    }

    /// Approves the pending request ONLY from genuine local host input after cooling-off completes,
    /// signing a `SessionGrant`, activating `HostFrameGatekeeper`, mounting the persistent banner,
    /// and switching the tray icon to `InSession`.
    ///
    /// # Errors
    /// Fails closed if `origin != InputOrigin::LocalHost`, if 5s cooling-off is still active,
    /// if the 2-minute TTL expired, or if `v0.1` policy checks fail.
    pub fn allow_session(
        &mut self,
        origin: InputOrigin,
        selection: HostPermissionSelection,
        duration: GrantDuration,
        now: DateTime<Utc>,
    ) -> Result<HostSnapshot, CaptureError> {
        self.flush_pending_synthetic_audit();
        if origin != InputOrigin::LocalHost {
            let pending_meta = self
                .pending
                .as_ref()
                .map(|p| (p.session_id, p.viewer_pubkey, p.server_domain.clone()));
            if let Some((sid, viewer, domain)) = pending_meta {
                self.protected_surfaces.record_block(
                    ProtectedSurfaceKind::ConsentPrompt,
                    "remote_allow_attempt",
                    now,
                );
                let _ = self.append_audit(
                    sid,
                    viewer,
                    &domain,
                    AuditEventKind::SyntheticInputBlocked {
                        surface: ProtectedSurfaceKind::ConsentPrompt.as_str().to_owned(),
                        reason: "remote_allow_attempt".to_owned(),
                    },
                    now,
                );
            }
            return Err(CaptureError::ProtectedSurfaceInputBlocked {
                surface: ProtectedSurfaceKind::ConsentPrompt.as_str(),
                reason: "remote_allow_attempt",
            });
        }

        let pending = self.pending.clone().ok_or_else(|| {
            CaptureError::GrantError(ProtocolError::InvalidTransition {
                state: self.consent_machine.state().name(),
                event: "HostAllow",
            })
        })?;

        if !self.effective_policy.is_duration_allowed(duration) {
            return Err(CaptureError::GrantError(ProtocolError::V01PolicyViolation(
                "requested grant duration exceeds monotonic policy limit",
            )));
        }

        let requested_perms: GrantPermissions = selection.into();
        let clamped_perms = self.effective_policy.clamp_permissions(&requested_perms);

        let grant = SessionGrant::issue(
            &self.host_keys,
            pending.session_id,
            pending.viewer_pubkey,
            &pending.server_domain,
            clamped_perms,
            duration,
            now,
        )?;

        #[allow(clippy::cast_possible_wrap)]
        let idle_secs = self
            .effective_policy
            .rules
            .idle_timeout
            .as_secs()
            .map(|s| s as i64);

        self.consent_machine.apply(ConsentEvent::HostAllow {
            grant: grant.clone(),
            host_pubkey: self.host_keys.public_key(),
            idle_timeout_secs: idle_secs,
            now,
        })?;

        let gatekeeper =
            HostFrameGatekeeper::from_consent_machine(self.consent_machine.clone(), now)?;
        self.gatekeeper = Some(gatekeeper);
        self.pending = None;
        self.protected_surfaces
            .clear_surface(ProtectedSurfaceKind::ConsentPrompt);

        self.active = Some(ActiveSessionInternal {
            session_id: pending.session_id,
            requester_name: pending.requester_name,
            viewer_pubkey: pending.viewer_pubkey,
            server_domain: pending.server_domain.clone(),
            server_enrolled: pending.server_enrolled,
            started_at: now,
            grant,
        });
        self.tray_state = TrayVisualState::InSession;
        self.alert_message = None;
        self.denial_streaks.remove(&pending.viewer_pubkey);

        let _ = self.append_audit(
            pending.session_id,
            pending.viewer_pubkey,
            &pending.server_domain,
            AuditEventKind::GrantIssued {
                permissions: clamped_perms,
            },
            now,
        );

        Ok(self.snapshot(now))
    }

    /// Declines the pending request immediately and returns to `Idle`.
    ///
    /// # Errors
    /// Returns [`CaptureError::GrantError`] if no pending consent request exists.
    pub fn deny_session(&mut self, now: DateTime<Utc>) -> Result<HostSnapshot, CaptureError> {
        self.flush_pending_synthetic_audit();
        self.consent_machine.apply(ConsentEvent::HostDeny {
            reason: DenialReason::OwnerDenied,
        })?;
        if let Some(p) = self.pending.take() {
            self.record_requester_denial(p.viewer_pubkey, now);
            let _ = self.append_audit(
                p.session_id,
                p.viewer_pubkey,
                &p.server_domain,
                AuditEventKind::ConsentDenied {
                    reason: DenialReason::OwnerDenied,
                },
                now,
            );
        }
        self.protected_surfaces
            .clear_surface(ProtectedSurfaceKind::ConsentPrompt);
        self.tray_state = TrayVisualState::Idle;
        Ok(self.snapshot(now))
    }

    /// Blocks a requester locally and optionally denies any pending request from them.
    ///
    /// # Errors
    /// Fails closed if `origin != InputOrigin::LocalHost`.
    pub fn block_requester(
        &mut self,
        origin: InputOrigin,
        requester_pubkey: IdentityPublicKey,
        server_domain: &str,
        now: DateTime<Utc>,
    ) -> Result<HostSnapshot, CaptureError> {
        if origin != InputOrigin::LocalHost {
            self.protected_surfaces.record_block(
                ProtectedSurfaceKind::ConsentPrompt,
                "remote_block_attempt",
                now,
            );
            self.flush_pending_synthetic_audit();
            return Err(CaptureError::ProtectedSurfaceInputBlocked {
                surface: ProtectedSurfaceKind::ConsentPrompt.as_str(),
                reason: "remote_block_attempt",
            });
        }
        self.blocked_requesters.insert(requester_pubkey);
        if self
            .pending
            .as_ref()
            .is_some_and(|p| p.viewer_pubkey == requester_pubkey)
        {
            let _ = self.deny_session(now);
        }
        let _ = self.append_audit(
            Uuid::nil(),
            requester_pubkey,
            server_domain,
            AuditEventKind::BlockedSecurityAlert {
                reason: BlockedReason::UnauthorizedOrRateLimited,
            },
            now,
        );
        self.alert_message = Some("Requester blocked on this device.".to_owned());
        Ok(self.snapshot(now))
    }

    /// Records a local-only abuse report and blocks the offending requester when requested.
    ///
    /// # Errors
    /// Fails closed if `origin != InputOrigin::LocalHost` or summary is empty.
    pub fn report_abuse(
        &mut self,
        origin: InputOrigin,
        params: HostAbuseReportParams<'_>,
        now: DateTime<Utc>,
    ) -> Result<HostAbuseReport, CaptureError> {
        if origin != InputOrigin::LocalHost {
            return Err(CaptureError::ProtectedSurfaceInputBlocked {
                surface: ProtectedSurfaceKind::ConsentPrompt.as_str(),
                reason: "remote_abuse_report_attempt",
            });
        }
        let cleaned: String = params
            .summary
            .chars()
            .filter(|c| !c.is_control())
            .collect::<String>()
            .trim()
            .chars()
            .take(500)
            .collect();
        if cleaned.is_empty() {
            return Err(CaptureError::GrantError(ProtocolError::V01PolicyViolation(
                "abuse report summary must not be empty",
            )));
        }
        if params.also_block {
            self.block_requester(origin, params.requester_pubkey, params.server_domain, now)?;
        }
        let report = HostAbuseReport {
            report_id: Uuid::now_v7().to_string(),
            requester_pubkey: params.requester_pubkey.to_prefixed_hex(),
            category: params.category.trim().to_owned(),
            summary: cleaned,
            blocked: params.also_block,
            created_at_utc: now.to_rfc3339(),
        };
        self.abuse_reports.push(report.clone());
        Ok(report)
    }

    /// Returns all locally recorded abuse reports.
    #[must_use]
    pub fn abuse_reports(&self) -> &[HostAbuseReport] {
        &self.abuse_reports
    }

    /// Returns whether `requester_pubkey` is explicitly blocked or in 15-minute cooldown lockout.
    #[must_use]
    pub fn is_requester_blocked_or_locked(
        &self,
        requester_pubkey: &IdentityPublicKey,
        now: DateTime<Utc>,
    ) -> bool {
        if self.blocked_requesters.contains(requester_pubkey) {
            return true;
        }
        self.denial_streaks
            .get(requester_pubkey)
            .and_then(|(_, locked_until)| *locked_until)
            .is_some_and(|until| now < until)
    }

    fn record_requester_denial(&mut self, requester_pubkey: IdentityPublicKey, now: DateTime<Utc>) {
        let entry = self
            .denial_streaks
            .entry(requester_pubkey)
            .or_insert((0, None));
        entry.0 = entry.0.saturating_add(1);
        if entry.0 >= HOST_MAX_DENIAL_STREAK {
            entry.1 = Some(now + Duration::seconds(HOST_LOCKOUT_DURATION_SECS));
        }
    }

    /// Updates pending permission checkboxes or active session grant permissions immediately.
    ///
    /// # Errors
    /// Returns [`CaptureError`] if active grant re-signing or monotonic clamping fails.
    pub fn update_permissions(
        &mut self,
        origin: InputOrigin,
        selection: HostPermissionSelection,
        now: DateTime<Utc>,
    ) -> Result<HostSnapshot, CaptureError> {
        if origin != InputOrigin::LocalHost {
            let surface = if self.active.is_some() {
                ProtectedSurfaceKind::SessionBanner
            } else {
                ProtectedSurfaceKind::ConsentPrompt
            };
            self.protected_surfaces
                .record_block(surface, "remote_permission_escalation", now);
            self.flush_pending_synthetic_audit();
            return Err(CaptureError::ProtectedSurfaceInputBlocked {
                surface: surface.as_str(),
                reason: "remote_permission_escalation",
            });
        }

        if let Some(pending) = &mut self.pending {
            let clamped = self.effective_policy.clamp_permissions(&selection.into());
            pending.permissions = HostPermissionSelection {
                view_screen: true,
                mouse_keyboard: clamped.mouse_keyboard,
                clipboard_sync: clamped.clipboard_sync,
                file_transfer: clamped.file_transfer,
            };
            return Ok(self.snapshot(now));
        }

        let active = self.active.as_mut().ok_or_else(|| {
            CaptureError::GrantError(ProtocolError::InvalidTransition {
                state: self.consent_machine.state().name(),
                event: "HostUpdatePermissions",
            })
        })?;

        let clamped = self.effective_policy.clamp_permissions(&selection.into());
        let updated_grant = active
            .grant
            .reissue_with_permissions(&self.host_keys, clamped, now)?;

        self.consent_machine
            .apply(ConsentEvent::HostUpdatePermissions {
                updated_grant: updated_grant.clone(),
                now,
            })?;
        if let Some(gk) = &mut self.gatekeeper {
            gk.update_grant(updated_grant.clone(), now)?;
        }
        active.grant = updated_grant;

        let sid = active.session_id;
        let viewer = active.viewer_pubkey;
        let domain = active.server_domain.clone();
        let _ = self.append_audit(
            sid,
            viewer,
            &domain,
            AuditEventKind::PermissionsUpdated {
                permissions: clamped,
            },
            now,
        );

        Ok(self.snapshot(now))
    }

    /// Local-only immediate `End session` action wired directly to `HostFrameGatekeeper::revoke()`.
    ///
    /// Rejects non-local origins (`InputOrigin::RemoteOrSynthetic`), keeping the persistent banner
    /// mounted and logging `SyntheticInputBlocked`.
    ///
    /// # Errors
    /// Returns [`CaptureError::ProtectedSurfaceInputBlocked`] on remote attempts, or
    /// [`CaptureError::GrantError`] if no session is active.
    pub fn end_session_local(
        &mut self,
        origin: InputOrigin,
        now: DateTime<Utc>,
    ) -> Result<HostSnapshot, CaptureError> {
        self.flush_pending_synthetic_audit();
        if origin != InputOrigin::LocalHost {
            self.protected_surfaces.record_block(
                ProtectedSurfaceKind::SessionBanner,
                "remote_dismiss_attempt",
                now,
            );
            self.flush_pending_synthetic_audit();
            return Err(CaptureError::ProtectedSurfaceInputBlocked {
                surface: ProtectedSurfaceKind::SessionBanner.as_str(),
                reason: "remote_dismiss_attempt",
            });
        }

        if let Some(gk) = &mut self.gatekeeper {
            gk.revoke();
        }
        self.consent_machine.apply(ConsentEvent::RevokeSession {
            reason: RevocationReason::HostEnded,
        })?;
        self.finalize_revocation_audit(RevocationReason::HostEnded, now);
        Ok(self.snapshot(now))
    }

    /// Evaluates an inbound WebRTC/remote message against `HostFrameGatekeeper`,
    /// `ProtectedSurfaceRegistry`, directional clipboard, and chunked file transfer limits,
    /// logging metadata-only audit entries (`ClipboardSyncEvent` / `FileTransferMetadata` /
    /// `SyntheticInputBlocked`).
    ///
    /// # Errors
    /// Returns [`CaptureError::FrameBlockedByGrant`], [`CaptureError::ProtectedSurfaceInputBlocked`],
    /// [`CaptureError::ClipboardSyncBlocked`], or [`CaptureError::FileTransferRejected`].
    pub fn enforce_inbound_message(
        &mut self,
        msg: &HostInboundMessage,
        now: DateTime<Utc>,
    ) -> Result<(), CaptureError> {
        let gk = self
            .gatekeeper
            .as_mut()
            .ok_or(CaptureError::FrameBlockedByGrant {
                frame_kind: "ungranted",
            })?;
        gk.authorize_frame(msg.frame_kind(), now)?;

        match msg {
            HostInboundMessage::PointerAction { x, y, .. } => {
                if let Err(err) = self.protected_surfaces.enforce_remote_pointer(*x, *y, now) {
                    gk.record_rejected_payload();
                    self.flush_pending_synthetic_audit();
                    return Err(err);
                }
                Ok(())
            }
            HostInboundMessage::ClipboardText { text } => {
                if let Err(err) = self.process_clipboard_sync(ClipboardEndpoint::Viewer, text, now)
                {
                    if let Some(gk) = &mut self.gatekeeper {
                        gk.record_rejected_payload();
                    }
                    return Err(err);
                }
                Ok(())
            }
            HostInboundMessage::FileTransferStart { .. }
            | HostInboundMessage::FileTransferData { .. }
            | HostInboundMessage::FileTransferComplete { .. }
            | HostInboundMessage::FileTransferCancel { .. } => {
                self.enforce_inbound_file_transfer(msg, now)
            }
            HostInboundMessage::MouseKeyboardInput { .. }
            | HostInboundMessage::FileChunk { .. }
            | HostInboundMessage::BlankScreenRequest => Ok(()),
        }
    }

    fn enforce_inbound_file_transfer(
        &mut self,
        msg: &HostInboundMessage,
        now: DateTime<Utc>,
    ) -> Result<(), CaptureError> {
        let res = match msg {
            HostInboundMessage::FileTransferStart {
                transfer_id,
                filename,
                total_bytes,
            } => self
                .start_file_transfer(transfer_id, filename, *total_bytes, now)
                .map(|_| ()),
            HostInboundMessage::FileTransferData {
                transfer_id,
                seq,
                data,
            } => self
                .push_file_chunk(transfer_id, *seq, data, now)
                .map(|_| ()),
            HostInboundMessage::FileTransferComplete { transfer_id } => {
                self.complete_file_transfer(transfer_id, now).map(|_| ())
            }
            HostInboundMessage::FileTransferCancel {
                transfer_id,
                reason,
            } => {
                let _ = self.cancel_file_transfer(transfer_id, reason);
                Ok(())
            }
            _ => Ok(()),
        };
        if let Err(err) = res {
            if let Some(gk) = &mut self.gatekeeper {
                gk.record_rejected_payload();
            }
            return Err(err);
        }
        Ok(())
    }

    /// Sets clipboard sync direction from genuine local host input only.
    ///
    /// # Errors
    /// Fails closed if `origin != InputOrigin::LocalHost`.
    pub fn set_clipboard_direction(
        &mut self,
        origin: InputOrigin,
        direction: ClipboardDirection,
        now: DateTime<Utc>,
    ) -> Result<HostSnapshot, CaptureError> {
        if origin != InputOrigin::LocalHost {
            self.protected_surfaces.record_block(
                ProtectedSurfaceKind::SessionBanner,
                "remote_clipboard_direction_change",
                now,
            );
            self.flush_pending_synthetic_audit();
            return Err(CaptureError::ProtectedSurfaceInputBlocked {
                surface: ProtectedSurfaceKind::SessionBanner.as_str(),
                reason: "remote_clipboard_direction_change",
            });
        }
        self.clipboard_manager.set_direction(direction);
        Ok(self.snapshot(now))
    }

    /// Authorizes a clipboard text sync operation and records metadata-only audit entry.
    ///
    /// # Errors
    /// Fails closed if `clipboard_sync` is unchecked in `SessionGrant` or direction blocks `sender`.
    pub fn process_clipboard_sync(
        &mut self,
        sender: ClipboardEndpoint,
        text: &str,
        now: DateTime<Utc>,
    ) -> Result<ClipboardAuditMetadata, CaptureError> {
        let gk = self
            .gatekeeper
            .as_mut()
            .ok_or(CaptureError::FrameBlockedByGrant {
                frame_kind: "clipboard_sync",
            })?;
        let meta = self
            .clipboard_manager
            .authorize_text_sync(gk, sender, text, now)?;
        if let Some(active) = &self.active {
            let (sid, viewer, domain) = (
                active.session_id,
                active.viewer_pubkey,
                active.server_domain.clone(),
            );
            let _ = self.append_audit(sid, viewer, &domain, meta.to_audit_kind(), now);
        }
        Ok(meta)
    }

    /// Starts a chunked file transfer through `HostFrameGatekeeper` and monotonic size/type limits.
    ///
    /// # Errors
    /// Fails closed if `file_transfer` is unchecked in `SessionGrant` or file violates limits.
    pub fn start_file_transfer(
        &mut self,
        transfer_id: impl Into<String>,
        filename: &str,
        total_bytes: u64,
        now: DateTime<Utc>,
    ) -> Result<FileTransferProgress, CaptureError> {
        let gk = self
            .gatekeeper
            .as_mut()
            .ok_or(CaptureError::FrameBlockedByGrant {
                frame_kind: "file_transfer",
            })?;
        self.file_transfer_manager
            .start_transfer(gk, transfer_id, filename, total_bytes, now)
    }

    /// Appends a file chunk through `HostFrameGatekeeper`.
    ///
    /// # Errors
    /// Fails closed if `file_transfer` is unchecked/revoked or chunk checks fail.
    pub fn push_file_chunk(
        &mut self,
        transfer_id: &str,
        seq: u32,
        data: &[u8],
        now: DateTime<Utc>,
    ) -> Result<FileTransferProgress, CaptureError> {
        let gk = self
            .gatekeeper
            .as_mut()
            .ok_or(CaptureError::FrameBlockedByGrant {
                frame_kind: "file_transfer",
            })?;
        self.file_transfer_manager
            .push_chunk(gk, transfer_id, seq, data, now)
    }

    /// Finalizes a file transfer and records metadata-only `FileTransferMetadata` in `AuditChain`.
    ///
    /// # Errors
    /// Fails closed if incomplete, rejected by scan hook, or `file_transfer` was disabled.
    pub fn complete_file_transfer(
        &mut self,
        transfer_id: &str,
        now: DateTime<Utc>,
    ) -> Result<CompletedFileTransfer, CaptureError> {
        let gk = self
            .gatekeeper
            .as_mut()
            .ok_or(CaptureError::FrameBlockedByGrant {
                frame_kind: "file_transfer",
            })?;
        let completed = self
            .file_transfer_manager
            .finish_transfer(gk, transfer_id, now)?;
        if let Some(active) = &self.active {
            let (sid, viewer, domain) = (
                active.session_id,
                active.viewer_pubkey,
                active.server_domain.clone(),
            );
            let _ = self.append_audit(sid, viewer, &domain, completed.audit_metadata.clone(), now);
        }
        Ok(completed)
    }

    /// Cancels an active file transfer and drops partial buffered data.
    pub fn cancel_file_transfer(
        &mut self,
        transfer_id: &str,
        reason: impl Into<String>,
    ) -> Option<FileTransferProgress> {
        self.file_transfer_manager
            .cancel_transfer(transfer_id, reason)
    }

    /// Registers or updates the live bounding [`Rect`] (and optional Win32 `HWND`) for a
    /// protected consent or session banner window.
    pub fn set_protected_surface_rect(
        &self,
        surface: ProtectedSurfaceKind,
        rect: Rect,
        hwnd_raw: Option<isize>,
    ) {
        if let Some(hwnd) = hwnd_raw {
            self.protected_surfaces
                .set_surface(surface, rect, Some(hwnd));
        } else {
            self.protected_surfaces.update_surface_rect(surface, rect);
        }
    }

    /// Returns whether a pending consent prompt is currently active.
    #[must_use]
    pub const fn has_pending_request(&self) -> bool {
        self.pending.is_some()
    }

    /// Returns the pending requester's public key and bound server domain, if any.
    #[must_use]
    pub fn pending_requester_identity(&self) -> Option<(IdentityPublicKey, String)> {
        self.pending
            .as_ref()
            .map(|p| (p.viewer_pubkey, p.server_domain.clone()))
    }

    /// Returns whether an active remote session banner is currently mounted.
    #[must_use]
    pub const fn has_active_banner(&self) -> bool {
        self.active.is_some()
    }

    /// Triggers an explicit alert tray state (e.g. control plane error or TOFU alert).
    pub fn raise_alert(&mut self, message: impl Into<String>) {
        self.tray_state = TrayVisualState::Alert;
        self.alert_message = Some(message.into());
    }

    /// Builds a serializable [`HostSnapshot`] for the frontend windows.
    #[must_use]
    pub fn snapshot(&self, now: DateTime<Utc>) -> HostSnapshot {
        let pending_request = self.pending.as_ref().map(|p| {
            let cooling_end = p.prompt_started_at + Duration::seconds(COOLING_OFF_SECS);
            let cooling_off_remaining_secs = if p.first_time_requester && now < cooling_end {
                (cooling_end - now).num_seconds().clamp(1, COOLING_OFF_SECS)
            } else {
                0
            };
            let ttl_remaining_secs = (p.consent_expires_at - now)
                .num_seconds()
                .clamp(0, CONSENT_TTL_SECS);

            HostConsentView {
                session_id: p.session_id.to_string(),
                requester_name: p.requester_name.clone(),
                requester_pubkey: p.viewer_pubkey.to_prefixed_hex(),
                server_domain: p.server_domain.clone(),
                server_enrolled: p.server_enrolled,
                sas_code: format_sas_spaced(&p.derived_sas),
                first_time_requester: p.first_time_requester,
                cooling_off_remaining_secs,
                ttl_remaining_secs,
                permissions: p.permissions,
                policy_ceilings: PolicyCeilingView {
                    allow_mouse_keyboard: self.effective_policy.rules.allow_mouse_keyboard,
                    allow_clipboard_sync: self.effective_policy.rules.allow_clipboard_sync,
                    allow_file_transfer: self.effective_policy.rules.allow_file_transfer,
                    idle_timeout_label: match self.effective_policy.rules.idle_timeout.as_secs() {
                        Some(900) => "15 minutes".to_owned(),
                        Some(1800) => "30 minutes".to_owned(),
                        Some(3600) => "1 hour".to_owned(),
                        _ => "Never".to_owned(),
                    },
                },
                duration: p.duration,
            }
        });

        let active_banner = self.active.as_ref().map(|a| ActiveBannerView {
            session_id: a.session_id.to_string(),
            requester_name: a.requester_name.clone(),
            server_domain: a.server_domain.clone(),
            server_enrolled: a.server_enrolled,
            status_label: "LIVE".to_owned(),
            permissions: a.grant.permissions,
            permission_summary: summarize_permissions(&a.grant.permissions),
            duration: a.grant.duration,
            expires_at_utc: a.grant.expires_at_utc.to_rfc3339(),
        });

        let recent_audit = self
            .audit_chain
            .entries()
            .iter()
            .rev()
            .take(12)
            .map(|entry| AuditSummaryView {
                sequence: entry.sequence,
                timestamp_utc: entry.event.timestamp_utc.to_rfc3339(),
                server_domain: entry.event.server_domain.clone(),
                summary: summarize_audit_kind(&entry.event.kind),
            })
            .collect();

        HostSnapshot {
            tray_state: self.tray_state,
            consent_state_name: self.consent_machine.state().name().to_owned(),
            pending_request,
            active_banner,
            alert_message: self.alert_message.clone(),
            audit_entry_count: self.audit_chain.entries().len(),
            blocked_input_count: self.protected_surfaces.total_blocked(),
            dropped_frame_count: self
                .gatekeeper
                .as_ref()
                .map_or(0, HostFrameGatekeeper::dropped_frames),
            recent_audit,
        }
    }

    fn flush_pending_synthetic_audit(&mut self) {
        let (sid, viewer, domain) = if let Some(active) = &self.active {
            (
                active.session_id,
                active.viewer_pubkey,
                active.server_domain.clone(),
            )
        } else if let Some(pending) = &self.pending {
            (
                pending.session_id,
                pending.viewer_pubkey,
                pending.server_domain.clone(),
            )
        } else {
            (
                Uuid::nil(),
                self.host_keys.public_key(),
                "local.host".to_owned(),
            )
        };
        let _ = self.protected_surfaces.flush_to_audit_chain(
            &mut self.audit_chain,
            &self.host_keys,
            sid,
            viewer,
            &domain,
        );
    }

    fn finalize_revocation_audit(&mut self, reason: RevocationReason, now: DateTime<Utc>) {
        if let Some(active) = self.active.take() {
            let duration_secs =
                u64::try_from((now - active.started_at).num_seconds().max(0)).unwrap_or(0);
            let _ = self.append_audit(
                active.session_id,
                active.viewer_pubkey,
                &active.server_domain,
                AuditEventKind::SessionRevoked {
                    reason,
                    duration_secs,
                },
                now,
            );
        }
        self.protected_surfaces
            .clear_surface(ProtectedSurfaceKind::SessionBanner);
        self.tray_state = TrayVisualState::Idle;
    }

    fn append_audit(
        &mut self,
        session_id: Uuid,
        actor_pubkey: IdentityPublicKey,
        server_domain: &str,
        kind: AuditEventKind,
        timestamp_utc: DateTime<Utc>,
    ) -> Result<(), ProtocolError> {
        self.audit_chain.append(
            AuditEvent {
                event_id: Uuid::now_v7(),
                timestamp_utc,
                session_id,
                device_pubkey: self.host_keys.public_key(),
                actor_pubkey,
                server_domain: server_domain.to_owned(),
                kind,
            },
            &self.host_keys,
        )?;
        Ok(())
    }
}

/// Sanitizes requester display identity: strips control chars, bidi overrides, zero-width chars,
/// and trust-badge delimiters, caps length at [`MAX_REQUESTER_NAME_CHARS`], and rejects any
/// verification-spoof claim (`"verified"`).
///
/// # Errors
/// Returns [`CaptureError::GrantError`] if empty or containing `"verified"` spoofing markers.
pub fn sanitize_requester_name(raw: &str) -> Result<String, CaptureError> {
    let cleaned_chars: String = raw
        .chars()
        .filter(|&c| !is_forbidden_identity_char(c))
        .map(|c| match c {
            '(' | ')' | '[' | ']' | '{' | '}' | '·' => ' ',
            other => other,
        })
        .collect();

    let collapsed = cleaned_chars
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if collapsed.is_empty() {
        return Err(CaptureError::GrantError(ProtocolError::V01PolicyViolation(
            "requester_name must not be empty",
        )));
    }
    if collapsed.to_ascii_lowercase().contains("verified") {
        return Err(CaptureError::GrantError(ProtocolError::V01PolicyViolation(
            "requester_name cannot contain verification claim",
        )));
    }

    let capped: String = collapsed.chars().take(MAX_REQUESTER_NAME_CHARS).collect();
    let trimmed = capped.trim_end().to_owned();
    if trimmed.is_empty() {
        return Err(CaptureError::GrantError(ProtocolError::V01PolicyViolation(
            "requester_name must not be empty",
        )));
    }
    Ok(trimmed)
}

/// Validates a strict lowercase ASCII DNS `server_domain` (`<= 253` chars, no whitespace,
/// userinfo, paths, or spoofed trust labels).
///
/// # Errors
/// Returns [`CaptureError::GrantError`] on malformed or non-ASCII domain values.
pub fn validate_server_domain(raw: &str) -> Result<String, CaptureError> {
    let trimmed = raw.trim();
    if trimmed.is_empty()
        || trimmed.len() > MAX_SERVER_DOMAIN_CHARS
        || !trimmed.is_ascii()
        || trimmed.chars().any(is_forbidden_identity_char)
    {
        return Err(CaptureError::GrantError(ProtocolError::V01PolicyViolation(
            "invalid server_domain hostname",
        )));
    }

    let normalized = trimmed.to_ascii_lowercase();
    if !normalized.contains('.') {
        return Err(CaptureError::GrantError(ProtocolError::V01PolicyViolation(
            "server_domain must be a valid DNS hostname",
        )));
    }
    for label in normalized.split('.') {
        if label.is_empty()
            || label.len() > 63
            || label.starts_with('-')
            || label.ends_with('-')
            || !label
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        {
            return Err(CaptureError::GrantError(ProtocolError::V01PolicyViolation(
                "invalid server_domain hostname label",
            )));
        }
    }
    Ok(normalized)
}

fn is_forbidden_identity_char(c: char) -> bool {
    c.is_control()
        || matches!(
            c,
            '\u{200B}'..='\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' | '\u{FEFF}'
        )
}

/// Validates that a 15-minute support link window has not expired and matches the enrolled server domain.
///
/// # Errors
/// Returns [`CaptureError::GrantError`] if the server domain mismatches, TTL exceeds 15 minutes,
/// or the link is already expired at `now`.
pub fn validate_support_link_window(
    created_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
    bound_server_domain: &str,
    enrolled_server_domain: &str,
    now: DateTime<Utc>,
) -> Result<(), CaptureError> {
    let bound = validate_server_domain(bound_server_domain)?;
    let enrolled = validate_server_domain(enrolled_server_domain)?;
    if bound != enrolled {
        return Err(CaptureError::GrantError(ProtocolError::ServerNotEnrolled));
    }
    if expires_at <= created_at
        || (expires_at - created_at).num_seconds() > SUPPORT_LINK_MAX_TTL_SECS
        || now >= expires_at
    {
        return Err(CaptureError::GrantError(ProtocolError::ConsentTtlExpired));
    }
    Ok(())
}

/// Formats a 6-digit SAS string (`"739204"`) with a center space (`"739 204"`).
#[must_use]
pub fn format_sas_spaced(raw: &str) -> String {
    if raw.len() == 6 && raw.bytes().all(|b| b.is_ascii_digit()) {
        format!("{} {}", &raw[0..3], &raw[3..6])
    } else {
        raw.to_owned()
    }
}

/// Returns a colour-independent plain-English summary of granted permissions.
#[must_use]
pub fn summarize_permissions(perms: &GrantPermissions) -> String {
    let mut parts = vec!["Screen"];
    if perms.mouse_keyboard {
        parts.push("Input");
    }
    if perms.clipboard_sync {
        parts.push("Clipboard");
    }
    if perms.file_transfer {
        parts.push("Files");
    }
    if parts.len() == 1 {
        "Screen only".to_owned()
    } else {
        parts.join(" + ")
    }
}

fn summarize_audit_kind(kind: &AuditEventKind) -> String {
    match kind {
        AuditEventKind::SessionRequested => "Session requested".to_owned(),
        AuditEventKind::BlockedSecurityAlert { reason } => format!("Blocked alert ({reason:?})"),
        AuditEventKind::ConsentDenied { reason } => format!("Consent denied ({reason:?})"),
        AuditEventKind::GrantIssued { permissions } => {
            format!("Grant issued ({})", summarize_permissions(permissions))
        }
        AuditEventKind::PermissionsUpdated { permissions } => {
            format!(
                "Permissions updated ({})",
                summarize_permissions(permissions)
            )
        }
        AuditEventKind::FileTransferMetadata {
            filename,
            size_bytes,
        } => format!("File transfer ({filename}, {size_bytes} B)"),
        AuditEventKind::ClipboardSyncEvent => "Clipboard synced".to_owned(),
        AuditEventKind::SyntheticInputBlocked { surface, reason } => {
            format!("Blocked synthetic input on {surface} ({reason})")
        }
        AuditEventKind::SessionRevoked {
            reason,
            duration_secs,
        } => format!("Session ended ({reason:?}, {duration_secs}s)"),
    }
}
