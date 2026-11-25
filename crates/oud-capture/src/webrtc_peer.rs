//! Real `webrtc-rs` peer connection with H.264 RTP video track, low-latency
//! input data channel, and host-side `SessionGrant` gatekeeper enforcement.

use std::sync::Arc;
use std::time::Instant;

use bytes::Bytes;
use chrono::{DateTime, Utc};
use oud_protocol::grant::FrameKind;
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, Mutex};
use webrtc::api::interceptor_registry::register_default_interceptors;
use webrtc::api::media_engine::{MediaEngine, MIME_TYPE_H264};
use webrtc::api::APIBuilder;
use webrtc::data_channel::data_channel_init::RTCDataChannelInit;
use webrtc::data_channel::data_channel_message::DataChannelMessage;
use webrtc::data_channel::RTCDataChannel;
use webrtc::ice_transport::ice_candidate::RTCIceCandidateInit;
use webrtc::interceptor::registry::Registry;
use webrtc::media::Sample;
use webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::peer_connection::RTCPeerConnection;
use webrtc::rtp_transceiver::rtp_codec::RTCRtpCodecCapability;
use webrtc::track::track_local::track_local_static_sample::TrackLocalStaticSample;
use webrtc::track::track_local::TrackLocal;

use crate::capture::RawFrame;
use crate::encoder::{AdaptiveH264Pipeline, EncodedVideoFrame};
use crate::error::CaptureError;
use crate::gatekeeper::HostFrameGatekeeper;
use crate::ice_config::{IceConfig, TransportMode};
use crate::preset::QualityPreset;

/// Inbound host control/input messages sent over WebRTC data channels (`oud-input`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum HostInboundMessage {
    /// Remote pointer move/click or keyboard press.
    MouseKeyboardInput {
        /// Client monotonic sequence number.
        seq: u64,
        /// Event description (e.g. `"move"` or `"key"`).
        action: String,
    },
    /// Directional clipboard text sync payload.
    ClipboardText {
        /// UTF-8 clipboard payload.
        text: String,
    },
    /// File transfer chunk metadata + payload.
    FileChunk {
        /// Transfer UUID string.
        transfer_id: String,
        /// Byte payload.
        bytes_len: usize,
    },
    /// Screen blanking request (always rejected in `v0.1` Asks-First mode).
    BlankScreenRequest,
}

impl HostInboundMessage {
    /// Maps this message to its [`FrameKind`] for `SessionGrant` enforcement.
    #[must_use]
    pub const fn frame_kind(&self) -> FrameKind {
        match self {
            Self::MouseKeyboardInput { .. } => FrameKind::MouseKeyboard,
            Self::ClipboardText { .. } => FrameKind::ClipboardSync,
            Self::FileChunk { .. } => FrameKind::FileTransfer,
            Self::BlankScreenRequest => FrameKind::BlankScreen,
        }
    }
}

/// Host WebRTC peer holding the H.264 video track, data channels, and `HostFrameGatekeeper`.
pub struct HostMediaPeer {
    peer: Arc<RTCPeerConnection>,
    video_track: Arc<TrackLocalStaticSample>,
    input_channel: Arc<RTCDataChannel>,
    gatekeeper: Arc<Mutex<HostFrameGatekeeper>>,
    pipeline: Arc<Mutex<AdaptiveH264Pipeline>>,
    transport_mode: TransportMode,
    ice_rx: Mutex<mpsc::UnboundedReceiver<RTCIceCandidateInit>>,
    authorized_inputs: Arc<Mutex<Vec<HostInboundMessage>>>,
}

impl HostMediaPeer {
    /// Creates a new `webrtc-rs` host peer configured with `ice_config`, `gatekeeper`, and `pipeline`.
    ///
    /// # Errors
    /// Returns [`CaptureError`] if ICE config, `MediaEngine`, or `RTCPeerConnection` creation fails.
    pub async fn new(
        ice_config: &IceConfig,
        gatekeeper: HostFrameGatekeeper,
        pipeline: AdaptiveH264Pipeline,
    ) -> Result<Self, CaptureError> {
        let rtc_config = ice_config.to_rtc_configuration()?;
        let api = build_webrtc_api()?;
        let peer = Arc::new(
            api.new_peer_connection(rtc_config)
                .await
                .map_err(|e| CaptureError::WebRtc(e.to_string()))?,
        );

        let video_track = Arc::new(TrackLocalStaticSample::new(
            RTCRtpCodecCapability {
                mime_type: MIME_TYPE_H264.to_owned(),
                clock_rate: 90_000,
                channels: 0,
                sdp_fmtp_line:
                    "level-asymmetry-allowed=1;packetization-mode=1;profile-level-id=42001f"
                        .to_owned(),
                rtcp_feedback: vec![],
            },
            "oud-video".to_owned(),
            "oudos-host-stream".to_owned(),
        ));

        peer.add_track(Arc::clone(&video_track) as Arc<dyn TrackLocal + Send + Sync>)
            .await
            .map_err(|e| CaptureError::WebRtc(e.to_string()))?;

        let input_channel = peer
            .create_data_channel(
                "oud-input",
                Some(RTCDataChannelInit {
                    ordered: Some(false),
                    max_retransmits: Some(0),
                    ..Default::default()
                }),
            )
            .await
            .map_err(|e| CaptureError::WebRtc(e.to_string()))?;

        let gatekeeper_arc = Arc::new(Mutex::new(gatekeeper));
        let authorized_inputs = Arc::new(Mutex::new(Vec::new()));

        let gk_cb = Arc::clone(&gatekeeper_arc);
        let auth_cb = Arc::clone(&authorized_inputs);
        input_channel.on_message(Box::new(move |msg: DataChannelMessage| {
            let gk = Arc::clone(&gk_cb);
            let authorized = Arc::clone(&auth_cb);
            Box::pin(async move {
                if let Ok(parsed) = serde_json::from_slice::<HostInboundMessage>(&msg.data) {
                    let mut guard = gk.lock().await;
                    if guard
                        .authorize_frame(parsed.frame_kind(), Utc::now())
                        .is_ok()
                    {
                        authorized.lock().await.push(parsed);
                    }
                }
            })
        }));

        let (ice_tx, ice_rx) = mpsc::unbounded_channel();
        peer.on_ice_candidate(Box::new(move |cand| {
            let tx = ice_tx.clone();
            Box::pin(async move {
                if let Some(c) = cand {
                    if let Ok(init) = c.to_json() {
                        let _ = tx.send(init);
                    }
                }
            })
        }));

        let transport_mode = if ice_config.force_relay {
            TransportMode::TurnRelay
        } else {
            TransportMode::DirectP2p
        };

        Ok(Self {
            peer,
            video_track,
            input_channel,
            gatekeeper: gatekeeper_arc,
            pipeline: Arc::new(Mutex::new(pipeline)),
            transport_mode,
            ice_rx: Mutex::new(ice_rx),
            authorized_inputs,
        })
    }

    /// Creates an SDP offer for WebSocket signaling relay.
    ///
    /// # Errors
    /// Returns [`CaptureError::WebRtc`] if offer generation or local description setup fails.
    pub async fn create_offer(&self) -> Result<RTCSessionDescription, CaptureError> {
        let offer = self
            .peer
            .create_offer(None)
            .await
            .map_err(|e| CaptureError::WebRtc(e.to_string()))?;
        self.peer
            .set_local_description(offer.clone())
            .await
            .map_err(|e| CaptureError::WebRtc(e.to_string()))?;
        Ok(offer)
    }

    /// Sets the remote SDP answer from the viewer peer.
    ///
    /// # Errors
    /// Returns [`CaptureError::WebRtc`] if SDP answer application fails.
    pub async fn accept_answer(&self, answer: RTCSessionDescription) -> Result<(), CaptureError> {
        self.peer
            .set_remote_description(answer)
            .await
            .map_err(|e| CaptureError::WebRtc(e.to_string()))
    }

    /// Adds a remote ICE candidate received via signaling.
    ///
    /// # Errors
    /// Returns [`CaptureError::WebRtc`] if candidate addition fails.
    pub async fn add_ice_candidate(
        &self,
        candidate: RTCIceCandidateInit,
    ) -> Result<(), CaptureError> {
        self.peer
            .add_ice_candidate(candidate)
            .await
            .map_err(|e| CaptureError::WebRtc(e.to_string()))
    }

    /// Drains newly gathered local ICE candidates ready to send over signaling.
    pub async fn drain_local_ice_candidates(&self) -> Vec<RTCIceCandidateInit> {
        let mut rx = self.ice_rx.lock().await;
        let mut out = Vec::new();
        while let Ok(c) = rx.try_recv() {
            out.push(c);
        }
        out
    }

    /// Returns the current `RTCPeerConnectionState`.
    #[must_use]
    pub fn connection_state(&self) -> RTCPeerConnectionState {
        self.peer.connection_state()
    }

    /// Returns active transport mode (`DirectP2p` or `TurnRelay`).
    #[must_use]
    pub const fn transport_mode(&self) -> TransportMode {
        self.transport_mode
    }

    /// Returns shared reference to the host [`HostFrameGatekeeper`].
    #[must_use]
    pub const fn gatekeeper(&self) -> &Arc<Mutex<HostFrameGatekeeper>> {
        &self.gatekeeper
    }

    /// Returns shared reference to the adaptive H.264 pipeline.
    #[must_use]
    pub const fn pipeline(&self) -> &Arc<Mutex<AdaptiveH264Pipeline>> {
        &self.pipeline
    }

    /// Encodes and writes a captured desktop frame onto the WebRTC H.264 RTP track
    /// ONLY if [`HostFrameGatekeeper`] permits `FrameKind::ScreenView` at `now`.
    ///
    /// # Errors
    /// Returns [`CaptureError::FrameBlockedByGrant`] if no valid grant permits screen capture,
    /// or [`CaptureError::EncoderFailed`] / [`CaptureError::WebRtc`] on media errors.
    pub async fn send_video_frame(
        &self,
        frame: &RawFrame,
        preset: QualityPreset,
        rtt_ms: f64,
        loss_ratio: f64,
        now: DateTime<Utc>,
    ) -> Result<EncodedVideoFrame, CaptureError> {
        {
            let mut gk = self.gatekeeper.lock().await;
            gk.authorize_frame(FrameKind::ScreenView, now)?;
        }

        let encoded = {
            let mut pipe = self.pipeline.lock().await;
            pipe.encode(frame, preset, rtt_ms, loss_ratio)?
        };

        let sample = Sample {
            data: Bytes::from(encoded.annex_b.clone()),
            duration: encoded.duration,
            ..Default::default()
        };
        self.video_track
            .write_sample(&sample)
            .await
            .map_err(|e| CaptureError::WebRtc(e.to_string()))?;

        Ok(encoded)
    }

    /// Enforces [`HostFrameGatekeeper`] synchronously on a decoded inbound message before OS dispatch.
    ///
    /// # Errors
    /// Returns [`CaptureError::FrameBlockedByGrant`] if the active grant disallows this frame.
    pub async fn enforce_inbound_message(
        &self,
        msg: HostInboundMessage,
        now: DateTime<Utc>,
    ) -> Result<HostInboundMessage, CaptureError> {
        let mut gk = self.gatekeeper.lock().await;
        gk.authorize_frame(msg.frame_kind(), now)?;
        drop(gk);
        self.authorized_inputs.lock().await.push(msg.clone());
        Ok(msg)
    }

    /// Drains inbound messages that passed host [`HostFrameGatekeeper`] enforcement.
    pub async fn drain_authorized_inputs(&self) -> Vec<HostInboundMessage> {
        let mut guard = self.authorized_inputs.lock().await;
        std::mem::take(&mut *guard)
    }

    /// Closes the host peer connection and data channel cleanly.
    ///
    /// # Errors
    /// Returns [`CaptureError::WebRtc`] if close fails.
    pub async fn close(&self) -> Result<(), CaptureError> {
        let _ = self.input_channel.close().await;
        self.peer
            .close()
            .await
            .map_err(|e| CaptureError::WebRtc(e.to_string()))
    }
}

/// Delivered RTP video packet notification observed by [`ViewerMediaPeer`].
#[derive(Debug, Clone)]
pub struct ReceivedRtpFrame {
    /// RTP sequence number.
    pub sequence_number: u16,
    /// RTP timestamp.
    pub rtp_timestamp: u32,
    /// RTP marker bit (`true` on the final packet of an encoded video frame).
    pub marker: bool,
    /// Payload byte length.
    pub payload_len: usize,
    /// Monotonic arrival instant for latency telemetry.
    pub received_at: Instant,
}

/// Complete reassembled H.264 video frame delivered over RTP.
#[derive(Debug, Clone)]
pub struct DeliveredVideoFrame {
    /// RTP timestamp shared by all packets of this video frame.
    pub rtp_timestamp: u32,
    /// Number of RTP packets reassembled for this frame.
    pub packet_count: usize,
    /// Total received RTP payload bytes.
    pub total_bytes: usize,
    /// Monotonic arrival instant of the frame's marker packet.
    pub delivered_at: Instant,
}

/// Viewer WebRTC peer receiving the H.264 RTP stream and sending input data-channel frames.
pub struct ViewerMediaPeer {
    peer: Arc<RTCPeerConnection>,
    ice_rx: Mutex<mpsc::UnboundedReceiver<RTCIceCandidateInit>>,
    rtp_rx: Mutex<mpsc::UnboundedReceiver<ReceivedRtpFrame>>,
    input_channel: Arc<Mutex<Option<Arc<RTCDataChannel>>>>,
}

impl ViewerMediaPeer {
    /// Creates a new viewer `RTCPeerConnection` configured with `ice_config`.
    ///
    /// # Errors
    /// Returns [`CaptureError`] if ICE config or `RTCPeerConnection` setup fails.
    pub async fn new(ice_config: &IceConfig) -> Result<Self, CaptureError> {
        let rtc_config = ice_config.to_rtc_configuration()?;
        let api = build_webrtc_api()?;
        let peer = Arc::new(
            api.new_peer_connection(rtc_config)
                .await
                .map_err(|e| CaptureError::WebRtc(e.to_string()))?,
        );

        let (ice_tx, ice_rx) = mpsc::unbounded_channel();
        peer.on_ice_candidate(Box::new(move |cand| {
            let tx = ice_tx.clone();
            Box::pin(async move {
                if let Some(c) = cand {
                    if let Ok(init) = c.to_json() {
                        let _ = tx.send(init);
                    }
                }
            })
        }));

        let (rtp_tx, rtp_rx) = mpsc::unbounded_channel();
        peer.on_track(Box::new(move |track, _, _| {
            let tx = rtp_tx.clone();
            Box::pin(async move {
                tokio::spawn(async move {
                    let mut buf = vec![0u8; 1600];
                    while let Ok((pkt, _)) = track.read(&mut buf).await {
                        let _ = tx.send(ReceivedRtpFrame {
                            sequence_number: pkt.header.sequence_number,
                            rtp_timestamp: pkt.header.timestamp,
                            marker: pkt.header.marker,
                            payload_len: pkt.payload.len(),
                            received_at: Instant::now(),
                        });
                    }
                });
            })
        }));

        let input_channel = Arc::new(Mutex::new(None));
        let ch_holder = Arc::clone(&input_channel);
        peer.on_data_channel(Box::new(move |dc| {
            let holder = Arc::clone(&ch_holder);
            Box::pin(async move {
                if dc.label() == "oud-input" {
                    *holder.lock().await = Some(dc);
                }
            })
        }));

        Ok(Self {
            peer,
            ice_rx: Mutex::new(ice_rx),
            rtp_rx: Mutex::new(rtp_rx),
            input_channel,
        })
    }

    /// Accepts a host SDP offer and generates the viewer's SDP answer.
    ///
    /// # Errors
    /// Returns [`CaptureError::WebRtc`] if SDP handling fails.
    pub async fn accept_offer_and_create_answer(
        &self,
        offer: RTCSessionDescription,
    ) -> Result<RTCSessionDescription, CaptureError> {
        self.peer
            .set_remote_description(offer)
            .await
            .map_err(|e| CaptureError::WebRtc(e.to_string()))?;
        let answer = self
            .peer
            .create_answer(None)
            .await
            .map_err(|e| CaptureError::WebRtc(e.to_string()))?;
        self.peer
            .set_local_description(answer.clone())
            .await
            .map_err(|e| CaptureError::WebRtc(e.to_string()))?;
        Ok(answer)
    }

    /// Adds a remote ICE candidate from the host peer.
    ///
    /// # Errors
    /// Returns [`CaptureError::WebRtc`] if candidate addition fails.
    pub async fn add_ice_candidate(
        &self,
        candidate: RTCIceCandidateInit,
    ) -> Result<(), CaptureError> {
        self.peer
            .add_ice_candidate(candidate)
            .await
            .map_err(|e| CaptureError::WebRtc(e.to_string()))
    }

    /// Drains gathered local ICE candidates.
    pub async fn drain_local_ice_candidates(&self) -> Vec<RTCIceCandidateInit> {
        let mut rx = self.ice_rx.lock().await;
        let mut out = Vec::new();
        while let Ok(c) = rx.try_recv() {
            out.push(c);
        }
        out
    }

    /// Waits for the next received H.264 RTP packet on the inbound video track.
    pub async fn recv_rtp_frame(&self) -> Option<ReceivedRtpFrame> {
        self.rtp_rx.lock().await.recv().await
    }

    /// Reads RTP packets until the frame marker bit indicates a complete H.264 video frame.
    pub async fn recv_complete_video_frame(&self) -> Option<DeliveredVideoFrame> {
        let mut rx = self.rtp_rx.lock().await;
        let mut packet_count = 0usize;
        let mut total_bytes = 0usize;
        while let Some(pkt) = rx.recv().await {
            packet_count += 1;
            total_bytes += pkt.payload_len;
            if pkt.marker {
                return Some(DeliveredVideoFrame {
                    rtp_timestamp: pkt.rtp_timestamp,
                    packet_count,
                    total_bytes,
                    delivered_at: pkt.received_at,
                });
            }
        }
        None
    }

    /// Sends an inbound message over the `oud-input` data channel to the host peer.
    ///
    /// # Errors
    /// Returns [`CaptureError::WebRtc`] if the data channel is not open yet or send fails.
    pub async fn send_input_message(&self, msg: &HostInboundMessage) -> Result<(), CaptureError> {
        let guard = self.input_channel.lock().await;
        let dc = guard.as_ref().ok_or_else(|| {
            CaptureError::WebRtc("oud-input data channel not open yet".to_owned())
        })?;
        let bytes = serde_json::to_vec(msg).map_err(|e| CaptureError::WebRtc(e.to_string()))?;
        dc.send(&Bytes::from(bytes))
            .await
            .map_err(|e| CaptureError::WebRtc(e.to_string()))?;
        Ok(())
    }

    /// Returns `true` once the `oud-input` data channel is attached and open.
    pub async fn is_input_channel_ready(&self) -> bool {
        let guard = self.input_channel.lock().await;
        guard.as_ref().is_some_and(|dc| {
            dc.ready_state() == webrtc::data_channel::data_channel_state::RTCDataChannelState::Open
        })
    }

    /// Returns the current `RTCPeerConnectionState`.
    #[must_use]
    pub fn connection_state(&self) -> RTCPeerConnectionState {
        self.peer.connection_state()
    }

    /// Closes the viewer peer connection cleanly.
    ///
    /// # Errors
    /// Returns [`CaptureError::WebRtc`] if close fails.
    pub async fn close(&self) -> Result<(), CaptureError> {
        self.peer
            .close()
            .await
            .map_err(|e| CaptureError::WebRtc(e.to_string()))
    }
}

fn build_webrtc_api() -> Result<webrtc::api::API, CaptureError> {
    let mut media_engine = MediaEngine::default();
    media_engine
        .register_default_codecs()
        .map_err(|e| CaptureError::WebRtc(e.to_string()))?;
    let mut registry = Registry::new();
    registry = register_default_interceptors(registry, &mut media_engine)
        .map_err(|e| CaptureError::WebRtc(e.to_string()))?;
    Ok(APIBuilder::new()
        .with_media_engine(media_engine)
        .with_interceptor_registry(registry)
        .build())
}
