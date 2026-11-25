//! Stream quality presets (`Auto`, `Sharp`, `Fast`) and adaptive bitrate rules.

use serde::{Deserialize, Serialize};

/// Quality presets selectable in the Oudos viewer and negotiated with the host encoder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum QualityPreset {
    /// Adaptive 1080p@30fps baseline (`1.5–6.0 Mbps`), stepping down smoothly under congestion.
    #[default]
    Auto,
    /// Crisp screen-content text fidelity (`1080p` up to `60 fps`, `8 Mbps`).
    Sharp,
    /// Low-bandwidth responsive preset (`720p@24fps`, `1.5 Mbps`).
    Fast,
}

/// Concrete encoding parameters derived from a [`QualityPreset`] and network conditions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EncodeProfile {
    /// Target horizontal resolution in pixels.
    pub target_width: u32,
    /// Target vertical resolution in pixels.
    pub target_height: u32,
    /// Target frames per second.
    pub target_fps: u32,
    /// Target bitrate in bits per second.
    pub target_bitrate_bps: u32,
    /// Minimum adaptive bitrate floor in bits per second.
    pub min_bitrate_bps: u32,
    /// Maximum adaptive bitrate ceiling in bits per second.
    pub max_bitrate_bps: u32,
    /// Intra-frame (IDR) interval in frames.
    pub keyframe_interval_frames: u32,
    /// Enables screen-content motion/sharpness tuning.
    pub screen_content_tuning: bool,
}

impl QualityPreset {
    /// Returns the baseline [`EncodeProfile`] for this preset.
    #[must_use]
    pub const fn baseline_profile(self) -> EncodeProfile {
        match self {
            Self::Auto => EncodeProfile {
                target_width: 1920,
                target_height: 1080,
                target_fps: 30,
                target_bitrate_bps: 4_500_000,
                min_bitrate_bps: 1_500_000,
                max_bitrate_bps: 6_000_000,
                keyframe_interval_frames: 60,
                screen_content_tuning: true,
            },
            Self::Sharp => EncodeProfile {
                target_width: 1920,
                target_height: 1080,
                target_fps: 60,
                target_bitrate_bps: 8_000_000,
                min_bitrate_bps: 3_000_000,
                max_bitrate_bps: 12_000_000,
                keyframe_interval_frames: 120,
                screen_content_tuning: true,
            },
            Self::Fast => EncodeProfile {
                target_width: 1280,
                target_height: 720,
                target_fps: 24,
                target_bitrate_bps: 1_500_000,
                min_bitrate_bps: 800_000,
                max_bitrate_bps: 2_500_000,
                keyframe_interval_frames: 48,
                screen_content_tuning: false,
            },
        }
    }

    /// Adapts encoding parameters for observed RTT (ms) and packet loss ratio (`0.0..=1.0`).
    #[must_use]
    pub fn adapt_for_network(self, rtt_ms: f64, loss_ratio: f64) -> EncodeProfile {
        let mut profile = self.baseline_profile();
        if self != Self::Auto {
            return profile;
        }

        let loss = loss_ratio.clamp(0.0, 1.0);
        if loss >= 0.05 || rtt_ms >= 140.0 {
            profile.target_width = 1280;
            profile.target_height = 720;
            profile.target_fps = 24;
            profile.target_bitrate_bps = profile.min_bitrate_bps;
        } else if loss >= 0.02 || rtt_ms >= 90.0 {
            profile.target_bitrate_bps = 2_800_000;
            profile.target_fps = 30;
        } else if rtt_ms <= 35.0 && loss < 0.005 {
            profile.target_bitrate_bps = 5_500_000;
        }
        profile
    }
}
