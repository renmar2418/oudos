//! ICE, STUN, Cloudflare Realtime TURN free-tier, and self-hosted `coturn` fallback configuration.

use serde::{Deserialize, Serialize};
use webrtc::ice_transport::ice_server::RTCIceServer;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::policy::bundle_policy::RTCBundlePolicy;
use webrtc::peer_connection::policy::ice_transport_policy::RTCIceTransportPolicy;

use crate::error::CaptureError;

/// Default public STUN endpoint for P2P NAT traversal (zero-cost tier).
pub const DEFAULT_STUN_URL: &str = "stun:stun.cloudflare.com:3478";

/// Transport path selected or configured for WebRTC media delivery.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransportMode {
    /// Direct peer-to-peer ICE path (default).
    DirectP2p,
    /// Relayed path through configured TURN server.
    TurnRelay,
}

/// Optional TURN relay fallback configuration (`v0.1` $0-cost options).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "provider", rename_all = "snake_case")]
pub enum TurnFallbackConfig {
    /// Cloudflare Realtime TURN free tier (1,000 GB/month free quota).
    CloudflareRealtimeTurn {
        /// TURN/TURNS URLs (e.g. `turn:turn.cloudflare.com:3478?transport=udp`).
        turn_urls: Vec<String>,
        /// Ephemeral TURN username issued by Cloudflare Realtime API.
        username: String,
        /// Ephemeral TURN credential issued by Cloudflare Realtime API.
        credential: String,
    },
    /// Self-hosted `coturn` instance packaged in `deploy/docker-compose.yml`.
    SelfHostedCoturn {
        /// Hostname or IP of the `coturn` service.
        host: String,
        /// UDP/TCP TURN port (typically `3478`).
        port: u16,
        /// Optional TLS TURNS port (typically `5349`).
        tls_port: Option<u16>,
        /// TURN REST/static username.
        username: String,
        /// TURN REST/static credential.
        credential: String,
    },
}

impl TurnFallbackConfig {
    /// Builds a Cloudflare Realtime TURN fallback configuration with standard ports.
    #[must_use]
    pub fn cloudflare_default(username: impl Into<String>, credential: impl Into<String>) -> Self {
        Self::CloudflareRealtimeTurn {
            turn_urls: vec![
                "turn:turn.cloudflare.com:3478?transport=udp".to_owned(),
                "turn:turn.cloudflare.com:3478?transport=tcp".to_owned(),
                "turns:turn.cloudflare.com:5349?transport=tcp".to_owned(),
            ],
            username: username.into(),
            credential: credential.into(),
        }
    }

    /// Builds a self-hosted `coturn` fallback configuration.
    #[must_use]
    pub fn self_hosted_coturn(
        host: impl Into<String>,
        port: u16,
        tls_port: Option<u16>,
        username: impl Into<String>,
        credential: impl Into<String>,
    ) -> Self {
        Self::SelfHostedCoturn {
            host: host.into(),
            port,
            tls_port,
            username: username.into(),
            credential: credential.into(),
        }
    }

    /// Validates TURN fields and converts to a `webrtc-rs` [`RTCIceServer`].
    ///
    /// # Errors
    /// Returns [`CaptureError::InvalidIceConfig`] if URLs, host, or credentials are empty.
    pub fn to_rtc_ice_server(&self) -> Result<RTCIceServer, CaptureError> {
        match self {
            Self::CloudflareRealtimeTurn {
                turn_urls,
                username,
                credential,
            } => {
                if turn_urls.is_empty() {
                    return Err(CaptureError::InvalidIceConfig(
                        "Cloudflare TURN requires at least one URL".to_owned(),
                    ));
                }
                if username.trim().is_empty() || credential.trim().is_empty() {
                    return Err(CaptureError::InvalidIceConfig(
                        "Cloudflare TURN requires non-empty username and credential".to_owned(),
                    ));
                }
                Ok(RTCIceServer {
                    urls: turn_urls.clone(),
                    username: username.clone(),
                    credential: credential.clone(),
                })
            }
            Self::SelfHostedCoturn {
                host,
                port,
                tls_port,
                username,
                credential,
            } => {
                let trimmed = host.trim();
                if trimmed.is_empty() || *port == 0 {
                    return Err(CaptureError::InvalidIceConfig(
                        "coturn requires non-empty host and non-zero port".to_owned(),
                    ));
                }
                if username.trim().is_empty() || credential.trim().is_empty() {
                    return Err(CaptureError::InvalidIceConfig(
                        "coturn requires non-empty username and credential".to_owned(),
                    ));
                }
                let mut urls = vec![
                    format!("turn:{trimmed}:{port}?transport=udp"),
                    format!("turn:{trimmed}:{port}?transport=tcp"),
                ];
                if let Some(tls) = tls_port {
                    if *tls == 0 {
                        return Err(CaptureError::InvalidIceConfig(
                            "coturn tls_port must be non-zero".to_owned(),
                        ));
                    }
                    urls.push(format!("turns:{trimmed}:{tls}?transport=tcp"));
                }
                Ok(RTCIceServer {
                    urls,
                    username: username.clone(),
                    credential: credential.clone(),
                })
            }
        }
    }
}

/// Complete ICE configuration supporting direct P2P by default with optional TURN fallback.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IceConfig {
    /// STUN URLs used for direct P2P NAT discovery (empty in isolated host-only LAN/loopback).
    pub stun_urls: Vec<String>,
    /// Optional TURN relay fallback (`CloudflareRealtimeTurn` or `SelfHostedCoturn`).
    pub turn_fallback: Option<TurnFallbackConfig>,
    /// When `true`, forces ICE relay candidates only (`RTCIceTransportPolicy::Relay`).
    pub force_relay: bool,
}

impl Default for IceConfig {
    fn default() -> Self {
        Self::direct_p2p_default()
    }
}

impl IceConfig {
    /// Direct P2P default with Cloudflare STUN and no forced relay.
    #[must_use]
    pub fn direct_p2p_default() -> Self {
        Self {
            stun_urls: vec![DEFAULT_STUN_URL.to_owned()],
            turn_fallback: None,
            force_relay: false,
        }
    }

    /// Host-only local/loopback P2P configuration with zero external STUN dependencies.
    #[must_use]
    pub const fn local_loopback() -> Self {
        Self {
            stun_urls: Vec::new(),
            turn_fallback: None,
            force_relay: false,
        }
    }

    /// Direct P2P default with an attached TURN fallback for restrictive symmetric NATs.
    #[must_use]
    pub fn p2p_with_turn_fallback(turn: TurnFallbackConfig) -> Self {
        Self {
            stun_urls: vec![DEFAULT_STUN_URL.to_owned()],
            turn_fallback: Some(turn),
            force_relay: false,
        }
    }

    /// Converts this configuration into a `webrtc-rs` [`RTCConfiguration`].
    ///
    /// # Errors
    /// Returns [`CaptureError::InvalidIceConfig`] if `force_relay` is `true` without
    /// a valid TURN fallback or if TURN credentials are invalid.
    pub fn to_rtc_configuration(&self) -> Result<RTCConfiguration, CaptureError> {
        let mut ice_servers = Vec::new();
        if !self.stun_urls.is_empty() {
            ice_servers.push(RTCIceServer {
                urls: self.stun_urls.clone(),
                ..Default::default()
            });
        }
        if let Some(turn) = &self.turn_fallback {
            ice_servers.push(turn.to_rtc_ice_server()?);
        } else if self.force_relay {
            return Err(CaptureError::InvalidIceConfig(
                "force_relay requires a configured TURN fallback".to_owned(),
            ));
        }

        Ok(RTCConfiguration {
            ice_servers,
            ice_transport_policy: if self.force_relay {
                RTCIceTransportPolicy::Relay
            } else {
                RTCIceTransportPolicy::All
            },
            bundle_policy: RTCBundlePolicy::MaxBundle,
            ..Default::default()
        })
    }
}
