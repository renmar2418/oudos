//! WebSocket SDP/ICE & handshake signaling broker with fail-closed drop handling.
//!
//! The signaling broker relays opaque envelopes between authenticated peers
//! without ever generating, storing, or inspecting the 6-digit SAS code.

use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket};
use axum::extract::{Query, State, WebSocketUpgrade};
use axum::response::IntoResponse;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, RwLock};

use crate::error::ServerError;
use crate::AppState;

/// In-memory signaling broker state.
#[derive(Default)]
pub struct SignalingHub {
    peers: RwLock<HashMap<String, mpsc::UnboundedSender<String>>>,
    sessions: RwLock<HashMap<String, PendingSession>>,
}

#[derive(Clone)]
struct PendingSession {
    viewer_peer: String,
    host_peer: String,
    grant_active: bool,
}

/// Query parameters for `GET /ws/signal`.
#[derive(Debug, Deserialize)]
pub struct SignalConnectQuery {
    /// Peer identifier (`device:<9-digits>` or `viewer:<user_id>`).
    pub peer_id: String,
}

/// Opaque signaling envelope relayed between viewer and host.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SignalEnvelope {
    /// Initiate a new signaling session between `viewer_peer` and `host_peer`.
    SessionInit {
        /// Session UUID.
        session_id: String,
        /// Target peer ID.
        target_peer: String,
    },
    /// Opaque SDP/ICE/handshake payload forwarded to `target_peer`.
    Relay {
        /// Session UUID.
        session_id: String,
        /// Target peer ID.
        target_peer: String,
        /// Opaque JSON payload (SDP offer/answer, ICE candidate, or Noise commit/reveal).
        payload: serde_json::Value,
    },
    /// Host notifies broker that `SessionGrant` is signed and P2P is active.
    GrantActivated {
        /// Session UUID.
        session_id: String,
        /// Target peer ID.
        target_peer: String,
    },
    /// Fail-closed notification sent to counterparty when signaling drops mid-negotiation.
    SignalingDropped {
        /// Session UUID.
        session_id: String,
        /// Reason for fail-closed termination.
        reason: String,
    },
    /// Error notice returned to sender when target peer is offline or unavailable.
    Error {
        /// Session UUID.
        session_id: String,
        /// Human-readable reason.
        message: String,
    },
}

/// Upgrades an HTTP request to a signaling WebSocket for an enrolled peer.
///
/// # Errors
/// Returns [`ServerError::BadRequest`] if `peer_id` is empty.
pub async fn ws_signal_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    Query(query): Query<SignalConnectQuery>,
) -> Result<impl IntoResponse, ServerError> {
    if query.peer_id.trim().is_empty() {
        return Err(ServerError::BadRequest("peer_id is required".to_string()));
    }
    let peer_id = query.peer_id;
    Ok(ws.on_upgrade(move |socket| handle_socket(socket, state.signaling, peer_id)))
}

async fn handle_socket(socket: WebSocket, hub: Arc<SignalingHub>, peer_id: String) {
    let (mut ws_sender, mut ws_receiver) = socket.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();

    hub.peers.write().await.insert(peer_id.clone(), tx);

    let send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if ws_sender.send(Message::Text(msg)).await.is_err() {
                break;
            }
        }
    });

    while let Some(Ok(msg)) = ws_receiver.next().await {
        match msg {
            Message::Text(text) => {
                if let Ok(env) = serde_json::from_str::<SignalEnvelope>(&text) {
                    process_envelope(&hub, &peer_id, env).await;
                }
            }
            Message::Close(_) => break,
            _ => {}
        }
    }

    // Peer disconnected: clean up peer and fail-close any negotiating sessions.
    hub.peers.write().await.remove(&peer_id);
    fail_close_pending_sessions(&hub, &peer_id).await;
    send_task.abort();
}

async fn process_envelope(hub: &SignalingHub, sender_peer: &str, env: SignalEnvelope) {
    match env {
        SignalEnvelope::SessionInit {
            session_id,
            target_peer,
        } => {
            let target_online = hub.peers.read().await.contains_key(&target_peer);
            if !target_online {
                send_to_peer(
                    hub,
                    sender_peer,
                    &SignalEnvelope::SignalingDropped {
                        session_id,
                        reason: "target peer offline; fail-closed".to_string(),
                    },
                )
                .await;
                return;
            }
            hub.sessions.write().await.insert(
                session_id.clone(),
                PendingSession {
                    viewer_peer: sender_peer.to_string(),
                    host_peer: target_peer.clone(),
                    grant_active: false,
                },
            );
            send_to_peer(
                hub,
                &target_peer,
                &SignalEnvelope::SessionInit {
                    session_id,
                    target_peer: sender_peer.to_string(),
                },
            )
            .await;
        }
        SignalEnvelope::Relay {
            session_id,
            target_peer,
            payload,
        } => {
            let active = hub.sessions.read().await.contains_key(&session_id);
            if !active {
                send_to_peer(
                    hub,
                    sender_peer,
                    &SignalEnvelope::SignalingDropped {
                        session_id,
                        reason: "session not active; fail-closed".to_string(),
                    },
                )
                .await;
                return;
            }
            if !send_to_peer(
                hub,
                &target_peer,
                &SignalEnvelope::Relay {
                    session_id: session_id.clone(),
                    target_peer: sender_peer.to_string(),
                    payload,
                },
            )
            .await
            {
                // Target dropped mid-negotiation: fail-close immediately.
                hub.sessions.write().await.remove(&session_id);
                send_to_peer(
                    hub,
                    sender_peer,
                    &SignalEnvelope::SignalingDropped {
                        session_id,
                        reason: "target signaling channel dropped".to_string(),
                    },
                )
                .await;
            }
        }
        SignalEnvelope::GrantActivated {
            session_id,
            target_peer,
        } => {
            if let Some(sess) = hub.sessions.write().await.get_mut(&session_id) {
                sess.grant_active = true;
            }
            send_to_peer(
                hub,
                &target_peer,
                &SignalEnvelope::GrantActivated {
                    session_id,
                    target_peer: sender_peer.to_string(),
                },
            )
            .await;
        }
        SignalEnvelope::SignalingDropped { .. } | SignalEnvelope::Error { .. } => {}
    }
}

async fn fail_close_pending_sessions(hub: &SignalingHub, dropped_peer: &str) {
    let mut to_notify = Vec::new();
    {
        let mut sessions = hub.sessions.write().await;
        let ids: Vec<String> = sessions
            .iter()
            .filter(|(_, s)| {
                !s.grant_active && (s.viewer_peer == dropped_peer || s.host_peer == dropped_peer)
            })
            .map(|(id, _)| id.clone())
            .collect();

        for id in ids {
            if let Some(sess) = sessions.remove(&id) {
                let other = if sess.viewer_peer == dropped_peer {
                    sess.host_peer
                } else {
                    sess.viewer_peer
                };
                to_notify.push((id, other));
            }
        }
    }

    for (session_id, other_peer) in to_notify {
        send_to_peer(
            hub,
            &other_peer,
            &SignalEnvelope::SignalingDropped {
                session_id,
                reason: "counterparty signaling dropped before grant".to_string(),
            },
        )
        .await;
    }
}

async fn send_to_peer(hub: &SignalingHub, peer_id: &str, env: &SignalEnvelope) -> bool {
    let Ok(json) = serde_json::to_string(env) else {
        return false;
    };
    let peers = hub.peers.read().await;
    if let Some(tx) = peers.get(peer_id) {
        tx.send(json).is_ok()
    } else {
        false
    }
}
