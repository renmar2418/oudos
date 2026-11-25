//! `oud-capture`: Windows screen capture, hardware/software H.264 encoding,
//! WebRTC P2P/TURN media pipeline, and host-gated `SessionGrant` enforcement.

pub mod capture;
pub mod encoder;
pub mod error;
pub mod gatekeeper;
pub mod ice_config;
pub mod input_isolation;
pub mod preset;
pub mod telemetry;
pub mod webrtc_peer;

#[cfg(target_os = "windows")]
pub mod windows_capture;

pub use capture::{
    create_platform_capturer, DisplayGeometry, PixelFormat, RawFrame, Rect, ScreenCapturer,
    SyntheticFrameCapturer,
};
pub use encoder::{
    discover_hardware_h264_adapters, AdaptiveH264Pipeline, EncodedVideoFrame, EncoderKind,
    SoftwareH264Encoder, VideoEncoder,
};
pub use error::{CaptureError, ScreenUnavailableReason};
pub use gatekeeper::HostFrameGatekeeper;
pub use ice_config::{IceConfig, TransportMode, TurnFallbackConfig, DEFAULT_STUN_URL};
pub use input_isolation::{
    is_synthetic_keyboard_flags, is_synthetic_mouse_flags, point_in_rect, ProtectedSurfaceKind,
    ProtectedSurfaceRegistry, SyntheticBlockRecord,
};
#[cfg(target_os = "windows")]
pub use input_isolation::{query_window_rect, WindowsInputGuard};
pub use oud_protocol::PROTOCOL_VERSION;
pub use preset::{EncodeProfile, QualityPreset};
pub use telemetry::{SessionTelemetry, TelemetrySnapshot};
pub use webrtc_peer::{
    DeliveredVideoFrame, DtlsFingerprintBinding, HostInboundMessage, HostMediaPeer,
    ReceivedRtpFrame, SignedSdpDescription, VerifiedPeerContext, ViewerMediaPeer,
};

/// Supported host capture backends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureBackend {
    /// Windows Desktop Duplication / Windows Graphics Capture (`v0.1`).
    WindowsWgc,
    /// Linux `PipeWire` `ScreenCast` portal (`v0.2`).
    LinuxPipeWire,
    /// macOS `ScreenCaptureKit` (`v0.3`).
    MacOsScreenCaptureKit,
}

#[cfg(test)]
mod tests {
    use super::{CaptureBackend, PROTOCOL_VERSION};

    #[test]
    fn capture_uses_workspace_protocol_version() {
        assert_eq!(PROTOCOL_VERSION, "oudos-v0.1");
        assert_ne!(CaptureBackend::WindowsWgc, CaptureBackend::LinuxPipeWire);
    }
}
