//! Host-signed `SessionGrant` capability tokens and `v0.1` Asks-First bounds.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::ProtocolError;
use crate::identity::{IdentityKeyPair, IdentityPublicKey, IdentitySignature};

/// Maximum duration in seconds for a `OneHour` grant in `v0.1`.
pub const ONE_HOUR_SECS: i64 = 3600;

/// Maximum duration in seconds for a `ThisSession` grant in `v0.1` (bounded session cap).
pub const THIS_SESSION_MAX_SECS: i64 = 14_400;

/// Discrete grant durations allowed in `v0.1` Asks-First mode.
///
/// `"Until I turn it off"` is intentionally absent in `v0.1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GrantDuration {
    /// Active for the current session only (up to `THIS_SESSION_MAX_SECS`).
    ThisSession,
    /// Strictly bounded 1-hour grant.
    OneHour,
}

impl GrantDuration {
    /// Returns the maximum allowed lifetime for this duration option.
    #[must_use]
    pub const fn max_duration(self) -> Duration {
        match self {
            Self::ThisSession => Duration::seconds(THIS_SESSION_MAX_SECS),
            Self::OneHour => Duration::seconds(ONE_HOUR_SECS),
        }
    }
}

/// Host-enforced permission flags for an active session.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct GrantPermissions {
    /// Required screen view permission (`true` in active grants).
    pub view_screen: bool,
    /// Remote mouse & keyboard input injection (defaults to `false`).
    pub mouse_keyboard: bool,
    /// Directional clipboard sync (defaults to `false`).
    pub clipboard_sync: bool,
    /// Chunked file transfer (defaults to `false`).
    pub file_transfer: bool,
    /// Remote screen blanking (always `false` in `v0.1` Asks-First mode).
    pub blank_screen: bool,
}

impl Default for GrantPermissions {
    fn default() -> Self {
        Self {
            view_screen: true,
            mouse_keyboard: false,
            clipboard_sync: false,
            file_transfer: false,
            blank_screen: false,
        }
    }
}

/// Protocol-boundary frame classes checked against an active [`SessionGrant`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameKind {
    /// Outbound/inbound screen video frame.
    ScreenView,
    /// Remote pointer or keyboard input frame.
    MouseKeyboard,
    /// Clipboard text synchronization frame.
    ClipboardSync,
    /// File transfer chunk or control frame.
    FileTransfer,
    /// Request to blank the host display.
    BlankScreen,
}

/// Signed capability token issued exclusively by the host agent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionGrant {
    /// Unique grant identifier (`UUIDv7`).
    pub grant_id: Uuid,
    /// Bound session identifier (`UUIDv7`).
    pub session_id: Uuid,
    /// Host Ed25519 identity public key.
    pub device_pubkey: IdentityPublicKey,
    /// Viewer Ed25519 identity public key.
    pub viewer_pubkey: IdentityPublicKey,
    /// Raw pinned server domain.
    pub server_domain: String,
    /// Granted capabilities.
    pub permissions: GrantPermissions,
    /// Bounded `v0.1` duration policy.
    pub duration: GrantDuration,
    /// UTC issuance timestamp.
    pub issued_at_utc: DateTime<Utc>,
    /// UTC expiration timestamp.
    pub expires_at_utc: DateTime<Utc>,
    /// Host Ed25519 signature over the canonical grant payload.
    pub host_signature: IdentitySignature,
}

#[derive(Serialize)]
struct CanonicalGrantPayload<'a> {
    grant_id: Uuid,
    session_id: Uuid,
    device_pubkey: &'a IdentityPublicKey,
    viewer_pubkey: &'a IdentityPublicKey,
    server_domain: &'a str,
    permissions: GrantPermissions,
    duration: GrantDuration,
    issued_at_utc: i64,
    expires_at_utc: i64,
}

impl SessionGrant {
    /// Issues and signs a new [`SessionGrant`] on the host, enforcing `v0.1` invariants.
    ///
    /// # Errors
    /// Returns [`ProtocolError::V01PolicyViolation`] if `blank_screen` is `true`,
    /// `view_screen` is `false`, or `server_domain` is empty.
    pub fn issue(
        host_keys: &IdentityKeyPair,
        session_id: Uuid,
        viewer_pubkey: IdentityPublicKey,
        server_domain: &str,
        permissions: GrantPermissions,
        duration: GrantDuration,
        issued_at_utc: DateTime<Utc>,
    ) -> Result<Self, ProtocolError> {
        validate_v01_permissions(permissions)?;
        if server_domain.trim().is_empty() {
            return Err(ProtocolError::V01PolicyViolation(
                "server_domain must not be empty",
            ));
        }

        let grant_id = Uuid::now_v7();
        let issued_at_utc = truncate_to_seconds(issued_at_utc);
        let expires_at_utc = issued_at_utc + duration.max_duration();
        let device_pubkey = host_keys.public_key();
        let payload = canonical_grant_bytes(
            grant_id,
            session_id,
            &device_pubkey,
            &viewer_pubkey,
            server_domain,
            permissions,
            duration,
            issued_at_utc,
            expires_at_utc,
        )?;
        let host_signature = host_keys.sign(&payload);

        Ok(Self {
            grant_id,
            session_id,
            device_pubkey,
            viewer_pubkey,
            server_domain: server_domain.to_owned(),
            permissions,
            duration,
            issued_at_utc,
            expires_at_utc,
            host_signature,
        })
    }

    /// Re-issues a signed grant immediately when the host owner updates permissions mid-session.
    ///
    /// # Errors
    /// Returns [`ProtocolError::V01PolicyViolation`] if `new_permissions` violates `v0.1` bounds.
    pub fn reissue_with_permissions(
        &self,
        host_keys: &IdentityKeyPair,
        new_permissions: GrantPermissions,
        now: DateTime<Utc>,
    ) -> Result<Self, ProtocolError> {
        validate_v01_permissions(new_permissions)?;
        if host_keys.public_key() != self.device_pubkey {
            return Err(ProtocolError::HostKeyChanged);
        }
        if now >= self.expires_at_utc {
            return Err(ProtocolError::GrantExpired);
        }

        let issued_at_utc = truncate_to_seconds(now);
        let grant_id = Uuid::now_v7();
        let payload = canonical_grant_bytes(
            grant_id,
            self.session_id,
            &self.device_pubkey,
            &self.viewer_pubkey,
            &self.server_domain,
            new_permissions,
            self.duration,
            issued_at_utc,
            self.expires_at_utc,
        )?;
        let host_signature = host_keys.sign(&payload);

        Ok(Self {
            grant_id,
            session_id: self.session_id,
            device_pubkey: self.device_pubkey,
            viewer_pubkey: self.viewer_pubkey,
            server_domain: self.server_domain.clone(),
            permissions: new_permissions,
            duration: self.duration,
            issued_at_utc,
            expires_at_utc: self.expires_at_utc,
            host_signature,
        })
    }

    /// Verifies signature, bindings, `v0.1` policy caps, and expiration.
    ///
    /// # Errors
    /// Fails closed on any signature, binding, cap, or expiration violation.
    pub fn verify(
        &self,
        expected_host: &IdentityPublicKey,
        expected_session_id: Uuid,
        expected_viewer: &IdentityPublicKey,
        expected_server_domain: &str,
        now: DateTime<Utc>,
    ) -> Result<(), ProtocolError> {
        if &self.device_pubkey != expected_host {
            return Err(ProtocolError::HostKeyChanged);
        }
        if self.session_id != expected_session_id
            || &self.viewer_pubkey != expected_viewer
            || self.server_domain != expected_server_domain
        {
            return Err(ProtocolError::GrantBindingMismatch);
        }
        validate_v01_permissions(self.permissions)?;
        if self.issued_at_utc.timestamp_subsec_nanos() != 0
            || self.expires_at_utc.timestamp_subsec_nanos() != 0
        {
            return Err(ProtocolError::GrantBindingMismatch);
        }
        if self.expires_at_utc <= self.issued_at_utc {
            return Err(ProtocolError::GrantExpired);
        }
        if self.expires_at_utc - self.issued_at_utc > self.duration.max_duration() {
            return Err(ProtocolError::V01PolicyViolation(
                "grant exceeds v0.1 duration cap",
            ));
        }
        if now >= self.expires_at_utc {
            return Err(ProtocolError::GrantExpired);
        }

        let payload = canonical_grant_bytes(
            self.grant_id,
            self.session_id,
            &self.device_pubkey,
            &self.viewer_pubkey,
            &self.server_domain,
            self.permissions,
            self.duration,
            self.issued_at_utc,
            self.expires_at_utc,
        )?;
        expected_host.verify(&payload, &self.host_signature)
    }

    /// Host-side gatekeeper check before passing frames to OS capture/input APIs.
    #[must_use]
    pub fn allows_frame(&self, frame: FrameKind, now: DateTime<Utc>) -> bool {
        if now >= self.expires_at_utc {
            return false;
        }
        match frame {
            FrameKind::ScreenView => self.permissions.view_screen,
            FrameKind::MouseKeyboard => self.permissions.mouse_keyboard,
            FrameKind::ClipboardSync => self.permissions.clipboard_sync,
            FrameKind::FileTransfer => self.permissions.file_transfer,
            FrameKind::BlankScreen => false,
        }
    }
}

fn truncate_to_seconds(dt: DateTime<Utc>) -> DateTime<Utc> {
    DateTime::from_timestamp(dt.timestamp(), 0).unwrap_or(dt)
}

fn validate_v01_permissions(permissions: GrantPermissions) -> Result<(), ProtocolError> {
    if !permissions.view_screen {
        return Err(ProtocolError::V01PolicyViolation(
            "view_screen is required for active sessions",
        ));
    }
    if permissions.blank_screen {
        return Err(ProtocolError::V01PolicyViolation(
            "blank_screen is prohibited in v0.1 Asks-First mode",
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn canonical_grant_bytes(
    grant_id: Uuid,
    session_id: Uuid,
    device_pubkey: &IdentityPublicKey,
    viewer_pubkey: &IdentityPublicKey,
    server_domain: &str,
    permissions: GrantPermissions,
    duration: GrantDuration,
    issued_at_utc: DateTime<Utc>,
    expires_at_utc: DateTime<Utc>,
) -> Result<Vec<u8>, ProtocolError> {
    let payload = CanonicalGrantPayload {
        grant_id,
        session_id,
        device_pubkey,
        viewer_pubkey,
        server_domain,
        permissions,
        duration,
        issued_at_utc: issued_at_utc.timestamp(),
        expires_at_utc: expires_at_utc.timestamp(),
    };
    let mut bytes = b"oudos-grant-v1:".to_vec();
    let json = serde_json::to_vec(&payload)
        .map_err(|e| ProtocolError::SerializationError(e.to_string()))?;
    bytes.extend_from_slice(&json);
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_grant_is_screen_only() {
        let perms = GrantPermissions::default();
        assert!(perms.view_screen);
        assert!(!perms.mouse_keyboard);
        assert!(!perms.clipboard_sync);
        assert!(!perms.file_transfer);
        assert!(!perms.blank_screen);
    }

    #[test]
    fn grant_rejects_blank_screen_in_v01() {
        let host = IdentityKeyPair::generate();
        let viewer = IdentityKeyPair::generate().public_key();
        let perms = GrantPermissions {
            blank_screen: true,
            ..GrantPermissions::default()
        };
        let res = SessionGrant::issue(
            &host,
            Uuid::now_v7(),
            viewer,
            "support.example.org",
            perms,
            GrantDuration::ThisSession,
            Utc::now(),
        );
        assert!(matches!(res, Err(ProtocolError::V01PolicyViolation(_))));
    }

    #[test]
    fn forged_signature_fails_verification() {
        let host = IdentityKeyPair::generate();
        let attacker = IdentityKeyPair::generate();
        let viewer = IdentityKeyPair::generate().public_key();
        let session_id = Uuid::now_v7();
        let now = Utc::now();

        let forged = SessionGrant::issue(
            &attacker,
            session_id,
            viewer,
            "support.example.org",
            GrantPermissions::default(),
            GrantDuration::OneHour,
            now,
        )
        .expect("forged by attacker");

        assert_eq!(
            forged.verify(
                &host.public_key(),
                session_id,
                &viewer,
                "support.example.org",
                now
            ),
            Err(ProtocolError::HostKeyChanged)
        );
    }

    #[test]
    fn tampered_grant_fields_fail_signature_verification() {
        let host = IdentityKeyPair::generate();
        let viewer = IdentityKeyPair::generate().public_key();
        let attacker = IdentityKeyPair::generate().public_key();
        let session_id = Uuid::now_v7();
        let now = Utc::now();

        let valid = SessionGrant::issue(
            &host,
            session_id,
            viewer,
            "support.example.org",
            GrantPermissions::default(),
            GrantDuration::OneHour,
            now,
        )
        .expect("grant issued");

        assert_eq!(valid.issued_at_utc.timestamp_subsec_nanos(), 0);
        assert_eq!(valid.expires_at_utc.timestamp_subsec_nanos(), 0);
        assert!(valid
            .verify(
                &host.public_key(),
                session_id,
                &viewer,
                "support.example.org",
                now
            )
            .is_ok());

        // 1. Tamper permissions (escalate mouse_keyboard)
        let mut tampered_perms = valid.clone();
        tampered_perms.permissions.mouse_keyboard = true;
        assert_eq!(
            tampered_perms.verify(
                &host.public_key(),
                session_id,
                &viewer,
                "support.example.org",
                now
            ),
            Err(ProtocolError::SignatureVerificationFailed)
        );

        // 2. Tamper expires_at_utc within duration cap
        let mut tampered_expiry = valid.clone();
        tampered_expiry.expires_at_utc -= Duration::seconds(10);
        assert_eq!(
            tampered_expiry.verify(
                &host.public_key(),
                session_id,
                &viewer,
                "support.example.org",
                now
            ),
            Err(ProtocolError::SignatureVerificationFailed)
        );

        // 3. Tamper viewer_pubkey (binding check and signature check)
        let mut tampered_viewer = valid;
        tampered_viewer.viewer_pubkey = attacker;
        assert_eq!(
            tampered_viewer.verify(
                &host.public_key(),
                session_id,
                &viewer,
                "support.example.org",
                now
            ),
            Err(ProtocolError::GrantBindingMismatch)
        );
        assert_eq!(
            tampered_viewer.verify(
                &host.public_key(),
                session_id,
                &attacker,
                "support.example.org",
                now
            ),
            Err(ProtocolError::SignatureVerificationFailed)
        );
    }
}
