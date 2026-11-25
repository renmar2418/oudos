//! Platform screen capture trait, frame representation, and deterministic test source.

use std::time::Instant;

use serde::{Deserialize, Serialize};

use crate::error::CaptureError;
use crate::CaptureBackend;

/// Supported raw frame pixel layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    /// 32-bit `B G R A` byte order (native Windows DXGI / GDI layout).
    Bgra8,
    /// 32-bit `R G B A` byte order.
    Rgba8,
}

/// Screen or dirty display region in pixel coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rect {
    /// Left coordinate in pixels.
    pub x: i32,
    /// Top coordinate in pixels.
    pub y: i32,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
}

/// Active display geometry reported by the host capture backend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisplayGeometry {
    /// Display width in pixels.
    pub width: u32,
    /// Display height in pixels.
    pub height: u32,
    /// Nominal refresh rate in Hz.
    pub refresh_hz: u32,
    /// OS monitor identifier.
    pub monitor_name: String,
}

/// Captured host desktop frame buffer with capture timestamp for latency measurement.
#[derive(Debug, Clone)]
pub struct RawFrame {
    /// Frame width in pixels.
    pub width: u32,
    /// Frame height in pixels.
    pub height: u32,
    /// Row stride in bytes.
    pub stride: usize,
    /// Byte order of pixel buffer.
    pub pixel_format: PixelFormat,
    /// Monotonic capture timestamp.
    pub captured_at: Instant,
    /// Optional dirty rectangles emitted by DXGI Desktop Duplication.
    pub dirty_rects: Vec<Rect>,
    /// Raw pixel buffer (`stride * height` bytes).
    pub data: Vec<u8>,
}

/// Platform-agnostic screen capture interface implemented by Windows DXGI/WGC (`v0.1`).
pub trait ScreenCapturer: Send {
    /// Identifies the underlying OS capture backend.
    fn backend(&self) -> CaptureBackend;

    /// Probes active display availability or returns an actionable [`ScreenUnavailableReason`].
    ///
    /// # Errors
    /// Returns [`CaptureError::ScreenUnavailable`] if locked, asleep, missing permission, or headless.
    fn probe_availability(&self) -> Result<DisplayGeometry, CaptureError>;

    /// Acquires the next desktop frame.
    ///
    /// # Errors
    /// Returns [`CaptureError`] when display capture fails or becomes unavailable.
    fn capture_frame(&mut self) -> Result<RawFrame, CaptureError>;
}

/// Deterministic desktop-pattern frame capturer for headless CI benchmarks and tests.
///
/// Generates real 32-bit `BGRA8` frames with animated dirty-region motion so H.264
/// inter-frame compression and latency benchmarks execute identically on headless CI runners.
pub struct SyntheticFrameCapturer {
    geometry: DisplayGeometry,
    frame_index: u64,
    backing_buffer: Vec<u8>,
}

impl SyntheticFrameCapturer {
    /// Creates a new deterministic capturer at `width x height`.
    #[must_use]
    pub fn new(width: u32, height: u32, refresh_hz: u32) -> Self {
        let row_bytes = (width as usize) * 4;
        let mut backing_buffer = vec![0u8; row_bytes * (height as usize)];
        // Fill calm #F5F6F4-like desktop background in BGRA8 so H.264 encodes realistic content.
        for px in backing_buffer.as_chunks_mut::<4>().0 {
            px[0] = 0xF4; // B
            px[1] = 0xF6; // G
            px[2] = 0xF5; // R
            px[3] = 0xFF; // A
        }
        Self {
            geometry: DisplayGeometry {
                width,
                height,
                refresh_hz,
                monitor_name: "Oudos-Test-Display".to_owned(),
            },
            frame_index: 0,
            backing_buffer,
        }
    }
}

impl ScreenCapturer for SyntheticFrameCapturer {
    fn backend(&self) -> CaptureBackend {
        CaptureBackend::WindowsWgc
    }

    fn probe_availability(&self) -> Result<DisplayGeometry, CaptureError> {
        Ok(self.geometry.clone())
    }

    fn capture_frame(&mut self) -> Result<RawFrame, CaptureError> {
        let w = self.geometry.width as usize;
        let h = self.geometry.height as usize;
        let stride = w * 4;

        // Paint a small moving teal cursor/card rect to exercise real P-frame dirty regions.
        let box_size = 64usize.min(w).min(h);
        let idx = usize::try_from(self.frame_index).unwrap_or(0);
        let x0 = (idx * 17) % (w.saturating_sub(box_size).max(1));
        let y0 = (idx * 11) % (h.saturating_sub(box_size).max(1));

        for dy in 0..box_size {
            let row_off = (y0 + dy) * stride + x0 * 4;
            for dx in 0..box_size {
                let off = row_off + dx * 4;
                self.backing_buffer[off] = 0x6C; // B
                self.backing_buffer[off + 1] = 0x7B; // G
                self.backing_buffer[off + 2] = 0x0F; // R (#0F7B6C teal)
                self.backing_buffer[off + 3] = 0xFF;
            }
        }

        self.frame_index = self.frame_index.saturating_add(1);
        Ok(RawFrame {
            width: self.geometry.width,
            height: self.geometry.height,
            stride,
            pixel_format: PixelFormat::Bgra8,
            captured_at: Instant::now(),
            dirty_rects: vec![Rect {
                x: i32::try_from(x0).unwrap_or(0),
                y: i32::try_from(y0).unwrap_or(0),
                width: u32::try_from(box_size).unwrap_or(64),
                height: u32::try_from(box_size).unwrap_or(64),
            }],
            data: self.backing_buffer.clone(),
        })
    }
}

/// Attempts to create the native platform capturer for the current OS.
///
/// # Errors
/// Returns [`CaptureError::ScreenUnavailable`] or [`CaptureError::CaptureFailed`] if the OS
/// display session cannot be captured.
pub fn create_platform_capturer() -> Result<Box<dyn ScreenCapturer>, CaptureError> {
    #[cfg(target_os = "windows")]
    {
        Ok(Box::new(
            crate::windows_capture::WindowsDesktopCapturer::new()?,
        ))
    }
    #[cfg(not(target_os = "windows"))]
    {
        Err(CaptureError::CaptureFailed(
            "native Windows capture requires target_os = \"windows\" (v0.1)".to_owned(),
        ))
    }
}
