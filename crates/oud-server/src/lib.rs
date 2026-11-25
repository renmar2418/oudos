#![forbid(unsafe_code)]
//! `oud-server`: Self-hosted control plane, signaling broker, and audit API.

pub mod audit;
pub mod auth;
pub mod db;
pub mod error;
pub mod rate_limit;
pub mod signaling;
pub mod support_links;

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::routing::{delete, get, post, put};
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use oud_policy::{merge_policies_strict, EffectivePolicy, PolicyLayer, PolicyScope};
use oud_protocol::identity::{DeviceId, IdentityKeyPair, IdentityPublicKey, IdentitySignature};
use oud_protocol::PROTOCOL_VERSION;
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::AuthenticatedUser;
use crate::db::Database;
use crate::error::ServerError;
use crate::signaling::SignalingHub;

/// Shared application state for `oud-server`.
#[derive(Clone)]
pub struct AppState {
    /// SQLite database handle.
    pub db: Arc<Database>,
    /// Server Ed25519 identity keypair for pre-paired enrollment proofs.
    pub server_keypair: Arc<IdentityKeyPair>,
    /// Raw server domain (e.g. `support.example.org`).
    pub server_domain: String,
    /// WebSocket signaling hub.
    pub signaling: Arc<SignalingHub>,
    /// UTC clock source (defaults to `Utc::now`; injectable for deterministic expiry tests).
    pub clock: Arc<dyn Fn() -> DateTime<Utc> + Send + Sync>,
}

impl AppState {
    /// Creates a new [`AppState`] backed by `db_path` (use `":memory:"` for tests).
    ///
    /// # Errors
    /// Returns [`ServerError::Internal`] if SQLite initialization fails.
    pub fn new(
        db_path: &str,
        server_domain: impl Into<String>,
        server_keypair: IdentityKeyPair,
    ) -> Result<Self, ServerError> {
        Self::with_clock(db_path, server_domain, server_keypair, Arc::new(Utc::now))
    }

    /// Creates a new [`AppState`] with an injectable UTC clock.
    ///
    /// # Errors
    /// Returns [`ServerError::Internal`] if SQLite initialization fails.
    pub fn with_clock(
        db_path: &str,
        server_domain: impl Into<String>,
        server_keypair: IdentityKeyPair,
        clock: Arc<dyn Fn() -> DateTime<Utc> + Send + Sync>,
    ) -> Result<Self, ServerError> {
        let db = Arc::new(Database::open(db_path)?);
        Ok(Self {
            db,
            server_keypair: Arc::new(server_keypair),
            server_domain: server_domain.into(),
            signaling: Arc::new(SignalingHub::default()),
            clock,
        })
    }
}

/// Builds the complete Axum [`Router`] for `oud-server`.
pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/api/v1/server-info", get(server_info))
        .route("/api/v1/auth/register", post(auth::register_user))
        .route("/api/v1/auth/challenge", post(auth::issue_challenge))
        .route("/api/v1/auth/verify", post(auth::verify_challenge))
        .route("/api/v1/devices/enroll", post(enroll_device))
        .route("/api/v1/devices", get(list_devices))
        .route("/api/v1/devices/:device_id/presence", post(update_presence))
        .route(
            "/api/v1/devices/:device_id/block",
            post(support_links::block_requester_handler),
        )
        .route(
            "/api/v1/devices/:device_id/block/:requester_pubkey",
            delete(support_links::unblock_requester_handler),
        )
        .route(
            "/api/v1/devices/:device_id/blocks",
            get(support_links::list_blocked_requesters_handler),
        )
        .route(
            "/api/v1/support-links",
            post(support_links::create_support_link),
        )
        .route(
            "/api/v1/support-links/redeem",
            post(support_links::redeem_support_link),
        )
        .route(
            "/api/v1/abuse/report",
            post(support_links::submit_abuse_report),
        )
        .route(
            "/api/v1/abuse/reports",
            get(support_links::list_abuse_reports),
        )
        .route("/api/v1/sessions/request", post(request_session))
        .route("/api/v1/sessions/:device_id/outcome", post(record_outcome))
        .route("/api/v1/policies/:device_id", get(get_effective_policy))
        .route(
            "/api/v1/policies/:scope/:target_id",
            put(upsert_policy_layer),
        )
        .route(
            "/api/v1/audit",
            post(audit::append_audit_entry).get(audit::list_audit_entries),
        )
        .route("/api/v1/audit/export.csv", get(audit::export_audit_csv))
        .route("/ws/signal", get(signaling::ws_signal_handler))
        .with_state(state)
}

/// Returns the server's supported protocol version string.
#[must_use]
pub fn server_protocol_banner() -> (&'static str, PolicyScope) {
    (PROTOCOL_VERSION, PolicyScope::Organisation)
}

/// Public server identity metadata for pre-paired enrollment.
#[derive(Debug, Serialize, Deserialize)]
pub struct ServerInfoResponse {
    /// Raw server domain displayed on host consent cards.
    pub server_domain: String,
    /// Server Ed25519 public key (`ed25519:...`).
    pub server_pubkey: IdentityPublicKey,
    /// Protocol version.
    pub protocol_version: String,
}

async fn health() -> &'static str {
    "ok"
}

async fn server_info(State(state): State<AppState>) -> Json<ServerInfoResponse> {
    Json(ServerInfoResponse {
        server_domain: state.server_domain.clone(),
        server_pubkey: state.server_keypair.public_key(),
        protocol_version: PROTOCOL_VERSION.to_string(),
    })
}

/// Live presence state for a registered host device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DevicePresence {
    /// Device is connected and ready for requests.
    Online,
    /// Device is currently in an active session.
    InSession,
    /// Device is disconnected.
    Offline,
}

impl DevicePresence {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Online => "online",
            Self::InSession => "in_session",
            Self::Offline => "offline",
        }
    }

    fn parse(s: &str) -> Result<Self, ServerError> {
        match s {
            "online" => Ok(Self::Online),
            "in_session" => Ok(Self::InSession),
            "offline" => Ok(Self::Offline),
            other => Err(ServerError::Internal(format!(
                "unknown device presence `{other}`"
            ))),
        }
    }
}

/// Request payload for `POST /api/v1/devices/enroll`.
#[derive(Debug, Deserialize)]
pub struct EnrollDeviceRequest {
    /// 9-digit display locator.
    pub device_id: DeviceId,
    /// Host Ed25519 public key.
    pub device_pubkey: IdentityPublicKey,
    /// Human-readable device alias.
    pub alias: String,
    /// Operating system label (e.g. `Windows 11`).
    pub os_name: String,
    /// Host signature over `server_domain || ":" || device_id || ":" || device_pubkey`.
    pub host_signature: IdentitySignature,
}

/// Server-signed enrollment proof returned to the host.
#[derive(Debug, Serialize, Deserialize)]
pub struct EnrollDeviceResponse {
    /// Enrolled 9-digit device ID.
    pub device_id: DeviceId,
    /// Enrolled host public key.
    pub device_pubkey: IdentityPublicKey,
    /// Raw server domain.
    pub server_domain: String,
    /// Server public key.
    pub server_pubkey: IdentityPublicKey,
    /// Server Ed25519 signature binding `device_id` and `device_pubkey` to this server.
    pub server_signature: IdentitySignature,
}

async fn enroll_device(
    user: AuthenticatedUser,
    State(state): State<AppState>,
    Json(req): Json<EnrollDeviceRequest>,
) -> Result<Json<EnrollDeviceResponse>, ServerError> {
    let alias = req.alias.trim();
    if alias.is_empty() {
        return Err(ServerError::BadRequest("alias cannot be empty".to_string()));
    }

    let proof_msg = format!(
        "{}:{}:{}",
        state.server_domain,
        req.device_id.as_compact(),
        req.device_pubkey.to_prefixed_hex()
    );
    req.device_pubkey
        .verify(proof_msg.as_bytes(), &req.host_signature)
        .map_err(|_| ServerError::Unauthorized("invalid host enrollment signature".to_string()))?;

    let now = Utc::now().to_rfc3339();
    let device_id_str = req.device_id.as_compact().to_string();
    let pubkey_str = req.device_pubkey.to_prefixed_hex();

    state.db.with_conn(|conn| {
        let existing_pubkey: Option<String> = conn
            .query_row(
                "SELECT device_pubkey FROM devices WHERE device_id = ?1",
                [&device_id_str],
                |r| r.get(0),
            )
            .optional()?;

        if let Some(existing) = existing_pubkey {
            if existing != pubkey_str {
                return Err(ServerError::Conflict(
                    "device_id already bound to a different public key".to_string(),
                ));
            }
            conn.execute(
                "UPDATE devices SET alias = ?1, os_name = ?2, presence = 'online', last_seen_utc = ?3
                 WHERE device_id = ?4",
                [alias, req.os_name.trim(), &now, &device_id_str],
            )?;
        } else {
            conn.execute(
                "INSERT INTO devices (device_id, device_pubkey, alias, os_name, owner_user_id, presence, enrolled_at_utc, last_seen_utc)
                 VALUES (?1, ?2, ?3, ?4, ?5, 'online', ?6, ?6)",
                [
                    &device_id_str,
                    &pubkey_str,
                    alias,
                    req.os_name.trim(),
                    &user.user_id,
                    &now,
                ],
            )?;
        }
        Ok(())
    })?;

    let server_signature = state.server_keypair.sign(proof_msg.as_bytes());
    Ok(Json(EnrollDeviceResponse {
        device_id: req.device_id,
        device_pubkey: req.device_pubkey,
        server_domain: state.server_domain,
        server_pubkey: state.server_keypair.public_key(),
        server_signature,
    }))
}

/// Summary record returned by `GET /api/v1/devices`.
#[derive(Debug, Serialize, Deserialize)]
pub struct DeviceRecord {
    /// 9-digit display locator.
    pub device_id: DeviceId,
    /// Host Ed25519 public key.
    pub device_pubkey: IdentityPublicKey,
    /// Device alias.
    pub alias: String,
    /// OS name.
    pub os_name: String,
    /// Live presence state.
    pub presence: DevicePresence,
    /// Last seen timestamp.
    pub last_seen_utc: String,
}

async fn list_devices(
    _user: AuthenticatedUser,
    State(state): State<AppState>,
) -> Result<Json<Vec<DeviceRecord>>, ServerError> {
    state.db.with_conn(|conn| {
        let mut stmt = conn.prepare(
            "SELECT device_id, device_pubkey, alias, os_name, presence, last_seen_utc
             FROM devices ORDER BY alias ASC",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
            ))
        })?;

        let mut out = Vec::new();
        for row in rows {
            let (id_str, pk_str, alias, os_name, pres_str, last_seen_utc) = row?;
            out.push(DeviceRecord {
                device_id: DeviceId::parse(&id_str)?,
                device_pubkey: IdentityPublicKey::from_prefixed_hex(&pk_str)?,
                alias,
                os_name,
                presence: DevicePresence::parse(&pres_str)?,
                last_seen_utc,
            });
        }
        Ok(Json(out))
    })
}

/// Request body for `POST /api/v1/devices/:device_id/presence`.
#[derive(Debug, Deserialize)]
pub struct UpdatePresenceRequest {
    /// Updated presence state.
    pub presence: DevicePresence,
}

async fn update_presence(
    _user: AuthenticatedUser,
    State(state): State<AppState>,
    Path(device_id_raw): Path<String>,
    Json(req): Json<UpdatePresenceRequest>,
) -> Result<Json<DevicePresence>, ServerError> {
    let device_id = DeviceId::parse(&device_id_raw)?;
    let now = Utc::now().to_rfc3339();
    state.db.with_conn(|conn| {
        let updated = conn.execute(
            "UPDATE devices SET presence = ?1, last_seen_utc = ?2 WHERE device_id = ?3",
            [req.presence.as_str(), &now, device_id.as_compact()],
        )?;
        if updated == 0 {
            return Err(ServerError::NotFound("device not found".to_string()));
        }
        Ok(Json(req.presence))
    })
}

/// Request body for `POST /api/v1/sessions/request`.
#[derive(Debug, Deserialize)]
pub struct SessionRequestPayload {
    /// Target 9-digit device ID.
    pub device_id: DeviceId,
}

/// Response body for `POST /api/v1/sessions/request`.
#[derive(Debug, Serialize)]
pub struct SessionRequestResponse {
    /// Newly allocated session `UUIDv7`.
    pub session_id: Uuid,
    /// Target device public key (for TOFU pinning check on viewer).
    pub device_pubkey: IdentityPublicKey,
    /// Raw server domain.
    pub server_domain: String,
}

async fn request_session(
    user: AuthenticatedUser,
    State(state): State<AppState>,
    Json(req): Json<SessionRequestPayload>,
) -> Result<Json<SessionRequestResponse>, ServerError> {
    let device_id_str = req.device_id.as_compact().to_string();
    let requester_key = user.passkey_pubkey.to_prefixed_hex();

    rate_limit::check_and_record_request(
        &state.db,
        &requester_key,
        &device_id_str,
        (state.clock)(),
    )?;

    let pk_str: String = state.db.with_conn(|conn| {
        conn.query_row(
            "SELECT device_pubkey FROM devices WHERE device_id = ?1",
            [&device_id_str],
            |r| r.get(0),
        )
        .optional()?
        .ok_or_else(|| ServerError::NotFound("device not enrolled".to_string()))
    })?;

    Ok(Json(SessionRequestResponse {
        session_id: Uuid::now_v7(),
        device_pubkey: IdentityPublicKey::from_prefixed_hex(&pk_str)?,
        server_domain: state.server_domain,
    }))
}

/// Outcome reported to feed the 3-strike rate limiter.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionOutcomeKind {
    /// Host denied the request.
    Denied,
    /// 6-digit SAS verification failed.
    SasMismatch,
    /// Host signed a `SessionGrant`.
    Granted,
}

/// Request body for `POST /api/v1/sessions/:device_id/outcome`.
#[derive(Debug, Deserialize)]
pub struct SessionOutcomeRequest {
    /// Requester key (`ed25519:...`).
    pub requester_pubkey: IdentityPublicKey,
    /// Outcome kind.
    pub outcome: SessionOutcomeKind,
}

/// Response body for `POST /api/v1/sessions/:device_id/outcome`.
#[derive(Debug, Serialize, Deserialize)]
pub struct SessionOutcomeResponse {
    /// Current consecutive denial/mismatch streak.
    pub denial_streak: i64,
    /// Whether the requester is now locked out.
    pub locked_out: bool,
}

async fn record_outcome(
    _user: AuthenticatedUser,
    State(state): State<AppState>,
    Path(device_id_raw): Path<String>,
    Json(req): Json<SessionOutcomeRequest>,
) -> Result<Json<SessionOutcomeResponse>, ServerError> {
    let device_id = DeviceId::parse(&device_id_raw)?;
    let requester_key = req.requester_pubkey.to_prefixed_hex();
    let dev_str = device_id.as_compact();

    match req.outcome {
        SessionOutcomeKind::Granted => {
            rate_limit::reset_denial_streak(&state.db, &requester_key, dev_str)?;
            Ok(Json(SessionOutcomeResponse {
                denial_streak: 0,
                locked_out: false,
            }))
        }
        SessionOutcomeKind::Denied | SessionOutcomeKind::SasMismatch => {
            let (streak, locked_until) = rate_limit::record_denial_or_mismatch(
                &state.db,
                &requester_key,
                dev_str,
                (state.clock)(),
            )?;
            Ok(Json(SessionOutcomeResponse {
                denial_streak: streak,
                locked_out: locked_until.is_some(),
            }))
        }
    }
}

async fn upsert_policy_layer(
    _user: AuthenticatedUser,
    State(state): State<AppState>,
    Path((scope_raw, target_id)): Path<(String, String)>,
    Json(layer): Json<PolicyLayer>,
) -> Result<Json<PolicyLayer>, ServerError> {
    let scope = match scope_raw.as_str() {
        "organisation" => "organisation",
        "group" => "group",
        "device_owner" => "device_owner",
        _ => {
            return Err(ServerError::BadRequest(
                "scope must be organisation, group, or device_owner".to_string(),
            ))
        }
    };

    // Validate monotonic strictness if updating device_owner against org/group.
    if scope == "device_owner" {
        let (org, group) = load_org_and_group_layers(&state)?;
        let _ = merge_policies_strict(&org, &group, &layer)?;
    } else if scope == "group" {
        let (org, _) = load_org_and_group_layers(&state)?;
        let _ = merge_policies_strict(&org, &layer, &PolicyLayer::default())?;
    } else {
        let _ = merge_policies_strict(&layer, &PolicyLayer::default(), &PolicyLayer::default())?;
    }

    let json = serde_json::to_string(&layer)
        .map_err(|e| ServerError::Internal(format!("serialize policy: {e}")))?;
    let now = Utc::now().to_rfc3339();

    state.db.with_conn(|conn| {
        conn.execute(
            "INSERT INTO policies (scope, target_id, layer_json, updated_at_utc)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(scope, target_id) DO UPDATE SET
               layer_json = excluded.layer_json,
               updated_at_utc = excluded.updated_at_utc",
            [scope, target_id.trim(), &json, &now],
        )?;
        Ok(Json(layer))
    })
}

async fn get_effective_policy(
    _user: AuthenticatedUser,
    State(state): State<AppState>,
    Path(device_id_raw): Path<String>,
) -> Result<Json<EffectivePolicy>, ServerError> {
    let device_id = DeviceId::parse(&device_id_raw)?;
    let (org, group) = load_org_and_group_layers(&state)?;
    let owner = load_layer(&state, "device_owner", device_id.as_compact())?;
    let effective = merge_policies_strict(&org, &group, &owner)?;
    Ok(Json(effective))
}

fn load_org_and_group_layers(state: &AppState) -> Result<(PolicyLayer, PolicyLayer), ServerError> {
    let org = load_layer(state, "organisation", "default")?;
    let group = load_layer(state, "group", "default")?;
    Ok((org, group))
}

fn load_layer(state: &AppState, scope: &str, target_id: &str) -> Result<PolicyLayer, ServerError> {
    state.db.with_conn(|conn| {
        let json_opt: Option<String> = conn
            .query_row(
                "SELECT layer_json FROM policies WHERE scope = ?1 AND target_id = ?2",
                [scope, target_id],
                |r| r.get(0),
            )
            .optional()?;
        match json_opt {
            None => Ok(PolicyLayer::default()),
            Some(s) => serde_json::from_str(&s)
                .map_err(|e| ServerError::Internal(format!("corrupt policy layer: {e}"))),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_reports_v0_1_protocol() {
        let (version, scope) = server_protocol_banner();
        assert_eq!(version, "oudos-v0.1");
        assert_eq!(scope, PolicyScope::Organisation);
    }
}
