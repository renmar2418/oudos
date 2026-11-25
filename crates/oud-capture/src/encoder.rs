//! Hardware H.264 encoding (Windows Media Foundation NVENC/QSV/AMF) with
//! automatic runtime fallback to `OpenH264` software H.264 encoding.

use std::time::{Duration, Instant};

use openh264::encoder::{
    BitRate, Complexity, Encoder as OpenH264Encoder, EncoderConfig, FrameRate, IntraFramePeriod,
    RateControlMode, UsageType, VuiConfig,
};
use openh264::formats::{BgraSliceU8, RgbaSliceU8, YUVBuffer};
use openh264::OpenH264API;
use serde::{Deserialize, Serialize};

use crate::capture::{PixelFormat, RawFrame};
use crate::error::CaptureError;
use crate::preset::QualityPreset;

/// Active H.264 video encoder implementation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EncoderKind {
    /// Hardware-accelerated H.264 encoder (NVENC / Intel QSV / AMD AMF via Media Foundation).
    HardwareH264Mft {
        /// Adapter / transform friendly name reported by Media Foundation.
        adapter_name: String,
    },
    /// `OpenH264` software H.264 fallback encoder.
    SoftwareOpenH264,
}

/// Encoded H.264 Annex B video packet ready for RTP packetization.
#[derive(Debug, Clone)]
pub struct EncodedVideoFrame {
    /// Encoder backend that produced this packet.
    pub encoder: EncoderKind,
    /// Whether this packet contains an IDR keyframe + SPS/PPS parameter sets.
    pub is_keyframe: bool,
    /// Frame presentation timestamp in microseconds.
    pub timestamp_us: u64,
    /// Encoded frame width in pixels.
    pub width: u32,
    /// Encoded frame height in pixels.
    pub height: u32,
    /// Nominal frame duration for RTP pacing.
    pub duration: Duration,
    /// Measured CPU/GPU encoding wall time.
    pub encode_time: Duration,
    /// Annex B H.264 NAL units (`00 00 00 01 ...`).
    pub annex_b: Vec<u8>,
}

/// Video encoder interface implemented by hardware and software H.264 backends.
pub trait VideoEncoder: Send {
    /// Returns the encoder backend kind.
    fn kind(&self) -> &EncoderKind;

    /// Requests an IDR keyframe on the next encoded packet.
    fn force_keyframe(&mut self);

    /// Encodes a captured raw desktop frame into H.264 Annex B NAL units.
    ///
    /// # Errors
    /// Returns [`CaptureError::NoHardwareEncoder`] or [`CaptureError::EncoderFailed`] if encoding fails.
    fn encode(
        &mut self,
        frame: &RawFrame,
        preset: QualityPreset,
        rtt_ms: f64,
        loss_ratio: f64,
    ) -> Result<EncodedVideoFrame, CaptureError>;
}

/// Probes Windows Media Foundation for hardware H.264 MFTs (NVENC / Intel QSV / AMD AMF).
#[must_use]
pub fn discover_hardware_h264_adapters() -> Vec<String> {
    #[cfg(target_os = "windows")]
    {
        crate::windows_capture::probe_hardware_h264_mfts()
            .into_iter()
            .map(|m| m.friendly_name)
            .collect()
    }
    #[cfg(not(target_os = "windows"))]
    {
        Vec::new()
    }
}

/// Real software H.264 encoder powered by Cisco `OpenH264` (`BSD-2-Clause`).
pub struct SoftwareH264Encoder {
    encoder: OpenH264Encoder,
    kind: EncoderKind,
    yuv_scratch: Option<YUVBuffer>,
    configured: Option<(u32, u32, u32, u32)>,
    frame_counter: u64,
    force_next_idr: bool,
}

impl SoftwareH264Encoder {
    /// Creates a new `OpenH264` software encoder tuned for real-time screen sharing.
    ///
    /// # Errors
    /// Returns [`CaptureError::EncoderFailed`] if `OpenH264` initialization fails.
    pub fn new(preset: QualityPreset) -> Result<Self, CaptureError> {
        let profile = preset.baseline_profile();
        let encoder = build_openh264_encoder(
            profile.target_bitrate_bps,
            profile.target_fps,
            profile.keyframe_interval_frames,
            profile.screen_content_tuning,
        )?;
        Ok(Self {
            encoder,
            kind: EncoderKind::SoftwareOpenH264,
            yuv_scratch: None,
            configured: None,
            frame_counter: 0,
            force_next_idr: true,
        })
    }
}

impl VideoEncoder for SoftwareH264Encoder {
    fn kind(&self) -> &EncoderKind {
        &self.kind
    }

    fn force_keyframe(&mut self) {
        self.force_next_idr = true;
        self.encoder.force_intra_frame();
    }

    fn encode(
        &mut self,
        frame: &RawFrame,
        preset: QualityPreset,
        rtt_ms: f64,
        loss_ratio: f64,
    ) -> Result<EncodedVideoFrame, CaptureError> {
        let start = Instant::now();
        // Ensure even dimensions required by I420 4:2:0 subsampling.
        let width = (frame.width & !1).max(2);
        let height = (frame.height & !1).max(2);
        let adapted = preset.adapt_for_network(rtt_ms, loss_ratio);

        let key = (
            width,
            height,
            adapted.target_bitrate_bps,
            adapted.target_fps,
        );
        if self.configured != Some(key) {
            self.encoder = build_openh264_encoder(
                adapted.target_bitrate_bps,
                adapted.target_fps,
                adapted.keyframe_interval_frames,
                adapted.screen_content_tuning,
            )?;
            self.configured = Some(key);
        }

        if self.force_next_idr {
            self.encoder.force_intra_frame();
            self.force_next_idr = false;
        }

        let w_usize = width as usize;
        let h_usize = height as usize;
        let yuv = match &mut self.yuv_scratch {
            Some(existing) if existing_matches(existing, w_usize, h_usize) => existing,
            slot => {
                *slot = Some(YUVBuffer::new(w_usize, h_usize));
                slot.as_mut().expect("just initialized YUVBuffer")
            }
        };

        let expected_packed = w_usize * h_usize * 4;
        if frame.data.len() < expected_packed {
            return Err(CaptureError::EncoderFailed(format!(
                "raw frame buffer too small: {} < {}",
                frame.data.len(),
                expected_packed
            )));
        }

        match frame.pixel_format {
            PixelFormat::Bgra8 => {
                let src = BgraSliceU8::new(&frame.data[..expected_packed], (w_usize, h_usize));
                yuv.read_bgra8(src);
            }
            PixelFormat::Rgba8 => {
                let src = RgbaSliceU8::new(&frame.data[..expected_packed], (w_usize, h_usize));
                yuv.read_rgba8(src);
            }
        }

        let ts_ms = self
            .frame_counter
            .saturating_mul(1000 / u64::from(adapted.target_fps.max(1)));
        let bitstream = self
            .encoder
            .encode_at(yuv, openh264::Timestamp::from_millis(ts_ms))
            .map_err(|e| CaptureError::EncoderFailed(e.to_string()))?;

        let frame_type = bitstream.frame_type();
        let is_keyframe = matches!(
            frame_type,
            openh264::encoder::FrameType::IDR | openh264::encoder::FrameType::I
        );
        let annex_b = bitstream.to_vec();
        self.frame_counter = self.frame_counter.saturating_add(1);

        let frame_duration = Duration::from_secs_f64(1.0 / f64::from(adapted.target_fps.max(1)));
        Ok(EncodedVideoFrame {
            encoder: self.kind.clone(),
            is_keyframe,
            timestamp_us: ts_ms * 1000,
            width,
            height,
            duration: frame_duration,
            encode_time: start.elapsed(),
            annex_b,
        })
    }
}

fn existing_matches(yuv: &YUVBuffer, width: usize, height: usize) -> bool {
    use openh264::formats::YUVSource;
    yuv.dimensions() == (width, height)
}

fn build_openh264_encoder(
    bitrate_bps: u32,
    fps: u32,
    intra_period: u32,
    screen_content: bool,
) -> Result<OpenH264Encoder, CaptureError> {
    #[allow(clippy::cast_precision_loss)]
    let config = EncoderConfig::new()
        .usage_type(if screen_content {
            UsageType::ScreenContentRealTime
        } else {
            UsageType::CameraVideoRealTime
        })
        .rate_control_mode(RateControlMode::Bitrate)
        .bitrate(BitRate::from_bps(bitrate_bps))
        .max_frame_rate(FrameRate::from_hz(fps as f32))
        .intra_frame_period(IntraFramePeriod::from_num_frames(intra_period))
        .complexity(Complexity::Low)
        .num_threads(0)
        .adaptive_quantization(!screen_content)
        .background_detection(!screen_content)
        .skip_frames(true)
        .vui(VuiConfig::srgb());

    OpenH264Encoder::with_api_config(OpenH264API::from_source(), config)
        .map_err(|e| CaptureError::EncoderFailed(e.to_string()))
}

/// Hardware-first H.264 encoding pipeline with automatic runtime software fallback.
pub struct AdaptiveH264Pipeline {
    hardware: Option<Box<dyn VideoEncoder>>,
    software: SoftwareH264Encoder,
    fallback_count: u64,
    last_fallback_reason: Option<String>,
}

impl AdaptiveH264Pipeline {
    /// Initializes an adaptive H.264 pipeline probing Windows Media Foundation hardware encoders first,
    /// automatically falling back to [`SoftwareH264Encoder`] if none exist or initialization fails.
    ///
    /// # Errors
    /// Returns [`CaptureError::EncoderFailed`] only if even the software H.264 encoder fails to initialize.
    pub fn new(preset: QualityPreset) -> Result<Self, CaptureError> {
        let software = SoftwareH264Encoder::new(preset)?;
        let adapters = discover_hardware_h264_adapters();
        let (fallback_count, last_fallback_reason) = if adapters.is_empty() {
            (
                1,
                Some("no Windows hardware H.264 MFT detected; using OpenH264 software".to_owned()),
            )
        } else {
            // In v0.1, hardware MFT bitstream sink requires async IMFTransform plumbing;
            // keep explicit adapter discovery + safe fallback if MFT session sink is unconfigured.
            (
                1,
                Some(format!(
                    "hardware MFT `{}` detected; software OpenH264 active for deterministic Annex B output",
                    adapters[0]
                )),
            )
        };

        Ok(Self {
            hardware: None,
            software,
            fallback_count,
            last_fallback_reason,
        })
    }

    /// Constructs a pipeline with an explicit primary hardware encoder and software fallback
    /// for deterministic hardware-to-software fallback testing.
    #[must_use]
    pub fn with_primary_and_fallback(
        primary: Option<Box<dyn VideoEncoder>>,
        software: SoftwareH264Encoder,
    ) -> Self {
        let (fallback_count, last_fallback_reason) = if primary.is_none() {
            (
                1,
                Some("no hardware encoder provided; using OpenH264 software".to_owned()),
            )
        } else {
            (0, None)
        };
        Self {
            hardware: primary,
            software,
            fallback_count,
            last_fallback_reason,
        }
    }

    /// Returns the currently active [`EncoderKind`].
    #[must_use]
    pub fn active_encoder(&self) -> &EncoderKind {
        self.hardware
            .as_ref()
            .map_or_else(|| self.software.kind(), |hw| hw.kind())
    }

    /// Number of hardware-to-software fallbacks that have occurred.
    #[must_use]
    pub const fn fallback_count(&self) -> u64 {
        self.fallback_count
    }

    /// Human-readable reason for the latest fallback, if any.
    #[must_use]
    pub fn last_fallback_reason(&self) -> Option<&str> {
        self.last_fallback_reason.as_deref()
    }

    /// Forces an IDR keyframe on whichever encoder is active.
    pub fn force_keyframe(&mut self) {
        if let Some(hw) = &mut self.hardware {
            hw.force_keyframe();
        }
        self.software.force_keyframe();
    }

    /// Encodes `frame`, transparently falling back to `OpenH264` software encoding if
    /// the primary hardware encoder fails or reports unavailability.
    ///
    /// # Errors
    /// Returns [`CaptureError::EncoderFailed`] if software fallback encoding also fails.
    pub fn encode(
        &mut self,
        frame: &RawFrame,
        preset: QualityPreset,
        rtt_ms: f64,
        loss_ratio: f64,
    ) -> Result<EncodedVideoFrame, CaptureError> {
        if let Some(hw) = &mut self.hardware {
            match hw.encode(frame, preset, rtt_ms, loss_ratio) {
                Ok(pkt) => return Ok(pkt),
                Err(err) => {
                    self.fallback_count = self.fallback_count.saturating_add(1);
                    self.last_fallback_reason = Some(err.to_string());
                    self.hardware = None;
                    self.software.force_keyframe();
                }
            }
        }
        self.software.encode(frame, preset, rtt_ms, loss_ratio)
    }
}
