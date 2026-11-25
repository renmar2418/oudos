//! Latency, framerate, encoder fallback, and `v0.1` SLO telemetry tracking.

use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::encoder::EncoderKind;
use crate::ice_config::TransportMode;
use crate::preset::QualityPreset;

/// Summary benchmark & session quality report checked against `v0.1` SLO targets.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TelemetrySnapshot {
    /// Active quality preset.
    pub preset: QualityPreset,
    /// Active H.264 encoder backend.
    pub active_encoder: EncoderKind,
    /// Number of times pipeline fell back from hardware to software H.264.
    pub encoder_fallback_count: u64,
    /// Active transport mode (`direct_p2p` or `turn_relay`).
    pub transport_mode: TransportMode,
    /// Measured milliseconds from host `Allow` grant signing to first delivered video frame.
    pub allow_to_first_frame_ms: Option<f64>,
    /// Median end-to-end frame delivery latency in milliseconds.
    pub p50_latency_ms: f64,
    /// 95th-percentile end-to-end frame delivery latency in milliseconds.
    pub p95_latency_ms: f64,
    /// Measured frames per second across the window.
    pub measured_fps: f64,
    /// Total encoded & transmitted frames.
    pub frames_sent: u64,
    /// Total frames dropped at the host grant gatekeeper.
    pub frames_dropped_by_grant: u64,
}

impl TelemetrySnapshot {
    /// Verifies `v0.1` same-region Exit Gate SLOs:
    /// - `p50_latency_ms <= 80.0`
    /// - `allow_to_first_frame_ms <= 3000.0`
    /// - `measured_fps >= 30.0` when running `QualityPreset::Auto`
    #[must_use]
    pub fn meets_v01_slos(&self) -> bool {
        let first_frame_ok = self.allow_to_first_frame_ms.is_some_and(|ms| ms <= 3000.0);
        let p50_ok = self.frames_sent > 0 && self.p50_latency_ms <= 80.0;
        let fps_ok = match self.preset {
            QualityPreset::Auto => self.measured_fps >= 30.0,
            QualityPreset::Sharp | QualityPreset::Fast => self.measured_fps > 0.0,
        };
        first_frame_ok && p50_ok && fps_ok
    }
}

/// Real-time telemetry recorder measuring frame latencies without storing any content.
#[derive(Debug)]
pub struct SessionTelemetry {
    preset: QualityPreset,
    active_encoder: EncoderKind,
    encoder_fallback_count: u64,
    transport_mode: TransportMode,
    grant_signed_at: Instant,
    first_frame_at: Option<Instant>,
    last_frame_at: Option<Instant>,
    latencies_ms: Vec<f64>,
    frames_sent: u64,
}

impl SessionTelemetry {
    /// Starts telemetry tracking when the host signs the [`oud_protocol::SessionGrant`].
    #[must_use]
    pub fn start(
        preset: QualityPreset,
        active_encoder: EncoderKind,
        transport_mode: TransportMode,
        grant_signed_at: Instant,
    ) -> Self {
        Self {
            preset,
            active_encoder,
            encoder_fallback_count: 0,
            transport_mode,
            grant_signed_at,
            first_frame_at: None,
            last_frame_at: None,
            latencies_ms: Vec::new(),
            frames_sent: 0,
        }
    }

    /// Updates active encoder and fallback count after a hardware-to-software fallback.
    pub fn record_encoder_state(&mut self, encoder: EncoderKind, fallback_count: u64) {
        self.active_encoder = encoder;
        self.encoder_fallback_count = fallback_count;
    }

    /// Records a delivered video frame's end-to-end duration and delivery timestamp.
    pub fn record_delivered_frame(&mut self, latency: Duration, delivered_at: Instant) {
        if self.first_frame_at.is_none() {
            self.first_frame_at = Some(delivered_at);
        }
        self.last_frame_at = Some(delivered_at);
        self.frames_sent = self.frames_sent.saturating_add(1);
        self.latencies_ms.push(latency.as_secs_f64() * 1000.0);
    }

    /// Produces a [`TelemetrySnapshot`] with computed `p50`, `p95`, and `fps`.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn snapshot(&self, frames_dropped_by_grant: u64) -> TelemetrySnapshot {
        let allow_to_first_frame_ms = self.first_frame_at.map(|ts| {
            ts.saturating_duration_since(self.grant_signed_at)
                .as_secs_f64()
                * 1000.0
        });

        let p50_latency_ms = percentile(&self.latencies_ms, 0.50);
        let p95_latency_ms = percentile(&self.latencies_ms, 0.95);

        let measured_fps = match (self.first_frame_at, self.last_frame_at) {
            (Some(first), Some(last)) if self.frames_sent > 1 => {
                let span_secs = last.saturating_duration_since(first).as_secs_f64();
                if span_secs > 0.000_1 {
                    (self.frames_sent.saturating_sub(1) as f64) / span_secs
                } else {
                    f64::from(self.preset.baseline_profile().target_fps)
                }
            }
            _ => 0.0,
        };

        TelemetrySnapshot {
            preset: self.preset,
            active_encoder: self.active_encoder.clone(),
            encoder_fallback_count: self.encoder_fallback_count,
            transport_mode: self.transport_mode,
            allow_to_first_frame_ms,
            p50_latency_ms,
            p95_latency_ms,
            measured_fps,
            frames_sent: self.frames_sent,
            frames_dropped_by_grant,
        }
    }
}

#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
fn percentile(values: &[f64], q: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let idx = (((sorted.len() - 1) as f64) * q.clamp(0.0, 1.0)).round() as usize;
    sorted[idx]
}
