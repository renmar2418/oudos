//! Host-side protocol-boundary frame gatekeeper wired to `oud_protocol::SessionGrant`.

use chrono::{DateTime, Utc};
use oud_protocol::grant::{FrameKind, SessionGrant};
use oud_protocol::identity::IdentityPublicKey;
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
    expected_host_pubkey: IdentityPublicKey,
    expected_session_id: Uuid,
    expected_viewer_pubkey: IdentityPublicKey,
    expected_server_domain: String,
    allowed_frames: u64,
    dropped_frames: u64,
}

impl HostFrameGatekeeper {
    /// Creates a new gatekeeper initialized with a verified host-signed [`SessionGrant`].
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
            expected_host_pubkey,
            expected_session_id,
            expected_viewer_pubkey,
            expected_server_domain: server_domain,
            allowed_frames: 0,
            dropped_frames: 0,
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
            expected_host_pubkey,
            expected_session_id,
            expected_viewer_pubkey,
            expected_server_domain: expected_server_domain.into(),
            allowed_frames: 0,
            dropped_frames: 0,
        }
    }

    /// Activates or updates the in-memory [`SessionGrant`] immediately.
    ///
    /// # Errors
    /// Returns [`CaptureError::GrantError`] if the grant fails cryptographic or `v0.1` checks.
    pub fn update_grant(
        &mut self,
        grant: SessionGrant,
        now: DateTime<Utc>,
    ) -> Result<(), CaptureError> {
        grant.verify(
            &self.expected_host_pubkey,
            self.expected_session_id,
            &self.expected_viewer_pubkey,
            &self.expected_server_domain,
            now,
        )?;
        enforce_v01_grant_caps(&grant)?;
        self.active_grant = Some(grant);
        Ok(())
    }

    /// Revokes the active grant immediately so all subsequent frames fail closed.
    pub fn revoke(&mut self) {
        self.active_grant = None;
    }

    /// Checks whether `frame` is permitted under the active signed grant at `now`.
    ///
    /// # Errors
    /// Returns [`CaptureError::FrameBlockedByGrant`] if no active grant exists,
    /// the grant expired, or the permission flag is `false`.
    pub fn authorize_frame(
        &mut self,
        frame: FrameKind,
        now: DateTime<Utc>,
    ) -> Result<(), CaptureError> {
        let Some(grant) = &self.active_grant else {
            self.dropped_frames = self.dropped_frames.saturating_add(1);
            return Err(CaptureError::FrameBlockedByGrant {
                frame_kind: frame_label(frame),
            });
        };

        if matches!(frame, FrameKind::BlankScreen) || grant.permissions.blank_screen {
            self.dropped_frames = self.dropped_frames.saturating_add(1);
            return Err(CaptureError::FrameBlockedByGrant {
                frame_kind: frame_label(frame),
            });
        }

        if grant.allows_frame(frame, now) {
            self.allowed_frames = self.allowed_frames.saturating_add(1);
            Ok(())
        } else {
            self.dropped_frames = self.dropped_frames.saturating_add(1);
            Err(CaptureError::FrameBlockedByGrant {
                frame_kind: frame_label(frame),
            })
        }
    }

    /// Returns a reference to the active [`SessionGrant`], if any.
    #[must_use]
    pub const fn active_grant(&self) -> Option<&SessionGrant> {
        self.active_grant.as_ref()
    }

    /// Returns count of allowed frames passed through this gatekeeper.
    #[must_use]
    pub const fn allowed_frames(&self) -> u64 {
        self.allowed_frames
    }

    /// Returns count of unauthorized frames dropped by this gatekeeper.
    #[must_use]
    pub const fn dropped_frames(&self) -> u64 {
        self.dropped_frames
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
