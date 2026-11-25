//! `oud-capture`: Platform-agnostic traits for screen capture and
//! grant-gated input injection.

pub use oud_protocol::PROTOCOL_VERSION;

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
