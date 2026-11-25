//! Error types for screen capture, encoding fallback, WebRTC transport, and grant gating.

use oud_protocol::ProtocolError;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Plain-language reasons a host screen cannot currently be captured.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScreenUnavailableReason {
    /// Windows lock screen or secure desktop is active (`v0.1` user-session host cannot capture).
    Locked,
    /// Host computer or monitor is asleep.
    Asleep,
    /// Desktop Duplication / Windows Graphics Capture permission is unavailable.
    PermissionMissing,
    /// No active display output is connected.
    NoDisplay,
}

impl std::fmt::Display for ScreenUnavailableReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Locked => write!(f, "host screen is locked"),
            Self::Asleep => write!(f, "host display is asleep"),
            Self::PermissionMissing => write!(f, "screen capture permission missing"),
            Self::NoDisplay => write!(f, "no active monitor detected"),
        }
    }
}

/// Errors emitted by `oud-capture` capture, encoding, gating, or WebRTC components.
#[derive(Debug, Error)]
pub enum CaptureError {
    /// Host display is temporarily unavailable with an actionable reason.
    #[error("screen unavailable: {0}")]
    ScreenUnavailable(ScreenUnavailableReason),

    /// OS screen capture initialization or frame acquisition failed.
    #[error("screen capture failed: {0}")]
    CaptureFailed(String),

    /// Hardware H.264 encoder is unavailable on this host.
    #[error("no hardware H.264 encoder available: {0}")]
    NoHardwareEncoder(String),

    /// Video encoder failed to initialize or encode a frame.
    #[error("video encoder error: {0}")]
    EncoderFailed(String),

    /// Underlying `oud-protocol` grant or state-machine verification failed.
    #[error("protocol grant error: {0}")]
    GrantError(#[from] ProtocolError),

    /// Host gatekeeper dropped a frame not authorized by the active `SessionGrant`.
    #[error("frame `{frame_kind}` dropped: not permitted by active SessionGrant")]
    FrameBlockedByGrant {
        /// Human-readable frame class that was blocked.
        frame_kind: &'static str,
    },

    /// Remote pointer or synthetic input targeted a protected consent/banner surface.
    #[error("input blocked on protected surface `{surface}` ({reason})")]
    ProtectedSurfaceInputBlocked {
        /// Protected UI surface (`"consent_prompt"` or `"session_banner"`).
        surface: &'static str,
        /// Rejection mechanism (`"protected_rect"`, `"llmhf_injected"`, `"llkhf_injected"`).
        reason: &'static str,
    },

    /// Invalid STUN/TURN ICE server configuration.
    #[error("invalid ICE/TURN configuration: {0}")]
    InvalidIceConfig(String),

    /// WebRTC peer connection, SDP, or data-channel failure.
    #[error("WebRTC transport error: {0}")]
    WebRtc(String),

    /// Remote WebRTC SDP `a=fingerprint` failed signed handshake binding verification.
    #[error("DTLS fingerprint verification failed: {0}")]
    DtlsFingerprintMismatch(String),
}
