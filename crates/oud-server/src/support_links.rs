//! One-time 15-minute support links bound to the enrolled server identity,
//! plus host-side requester blocking and abuse reporting endpoints.

use axum::extract::{Path, State};
use axum::Json;
use chrono::{DateTime, Duration, Utc};
use oud_protocol::identity::{DeviceId, IdentityPublicKey, IdentitySignature};
use rand::RngCore;
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::auth::AuthenticatedUser;
use crate::error::ServerError;
use crate::rate_limit;
use crate::AppState;

/// Hard maximum lifetime in seconds for one-time support links (15 minutes).
pub const SUPPORT_LINK_MAX_TTL_SECS: i64 = 900;

/// Maximum length for abuse report summary text.
pub const MAX_ABUSE_SUMMARY_CHARS: usize = 500;

/// Request payload for `POST /api/v1/support-links`.
#[derive(Debug, Deserialize)]
pub struct CreateSupportLinkRequest {
    /// Target 9-digit device ID.
    pub device_id: DeviceId,
    /// Requested lifetime in seconds (`1..=900`, defaults to `900`).
    pub ttl_secs: Option<i64>,
}

/// Response payload for `POST /api/v1/support-links`.
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateSupportLinkResponse {
    /// Unique support link identifier (`UUIDv7`).
    pub link_id: String,
    /// Single-use secret token (stored hashed on the server).
    pub token: String,
    /// Raw HTTPS support URL bound to the server's raw domain.
    pub support_url: String,
    /// Bound 9-digit device ID.
    pub device_id: DeviceId,
    /// Bound host Ed25519 identity public key.
    pub device_pubkey: IdentityPublicKey,
    /// Raw enrolled server domain.
    pub server_domain: String,
    /// Server Ed25519 identity public key.
    pub server_pubkey: IdentityPublicKey,
    /// Server Ed25519 signature over canonical link binding.
    pub server_signature: IdentitySignature,
    /// UTC expiration timestamp (at most 15 minutes from issuance).
    pub expires_at_utc: DateTime<Utc>,
}

/// Request payload for `POST /api/v1/support-links/redeem`.
#[derive(Debug, Deserialize)]
pub struct RedeemSupportLinkRequest {
    /// One-time support link token.
    pub token: String,
    /// Optional raw server domain pinned by the client/host (must match server).
    pub expected_server_domain: Option<String>,
    /// Optional server Ed25519 public key pinned by the client/host (must match server).
    pub expected_server_pubkey: Option<IdentityPublicKey>,
}

/// Response payload for `POST /api/v1/support-links/redeem`.
#[derive(Debug, Serialize, Deserialize)]
pub struct RedeemSupportLinkResponse {
    /// Redeemed support link identifier.
    pub link_id: String,
    /// Newly allocated session `UUIDv7`.
    pub session_id: Uuid,
    /// Target 9-digit device ID.
    pub device_id: DeviceId,
    /// Target device Ed25519 public key.
    pub device_pubkey: IdentityPublicKey,
    /// Raw server domain.
    pub server_domain: String,
    /// Server Ed25519 public key.
    pub server_pubkey: IdentityPublicKey,
}

/// Abuse category reported by a device owner or admin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AbuseCategory {
    /// Caller impersonated a bank, tech support, or trusted entity.
    Impersonation,
    /// Repeated unwanted connection requests.
    Harassment,
    /// Suspected phone/refund/virus scam attempt.
    ScamAttempt,
    /// Other abuse category.
    Other,
}

impl AbuseCategory {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Impersonation => "impersonation",
            Self::Harassment => "harassment",
            Self::ScamAttempt => "scam_attempt",
            Self::Other => "other",
        }
    }
}

/// Request payload for `POST /api/v1/abuse/report`.
#[derive(Debug, Deserialize)]
pub struct AbuseReportRequest {
    /// Target 9-digit device ID.
    pub device_id: DeviceId,
    /// Offending requester Ed25519 public key.
    pub requester_pubkey: IdentityPublicKey,
    /// Abuse category.
    pub category: AbuseCategory,
    /// Short plain-text incident summary (never secrets/credentials).
    pub summary: String,
    /// Whether to automatically block this requester for `device_id` (defaults `true`).
    pub also_block: Option<bool>,
}

/// Response payload for `POST /api/v1/abuse/report`.
#[derive(Debug, Serialize, Deserialize)]
pub struct AbuseReportResponse {
    /// Created report identifier.
    pub report_id: String,
    /// Whether the requester was also blocked on `device_id`.
    pub blocked: bool,
    /// UTC timestamp.
    pub created_at_utc: DateTime<Utc>,
    /// Published response SLA target (`"24h"`).
    pub sla_target: String,
}

/// Stored metadata-only abuse report summary.
#[derive(Debug, Serialize, Deserialize)]
pub struct AbuseReportRecord {
    /// Report identifier.
    pub report_id: String,
    /// Target 9-digit device ID.
    pub device_id: DeviceId,
    /// Reporter user ID.
    pub reporter_user_id: String,
    /// Offending requester public key.
    pub requester_pubkey: IdentityPublicKey,
    /// Raw server domain.
    pub server_domain: String,
    /// Category string.
    pub category: String,
    /// Summary text.
    pub summary: String,
    /// Whether requester was blocked.
    pub blocked: bool,
    /// UTC timestamp string.
    pub created_at_utc: String,
}

/// Request payload for `POST /api/v1/devices/:device_id/block`.
#[derive(Debug, Deserialize)]
pub struct BlockRequesterRequest {
    /// Requester Ed25519 public key to block.
    pub requester_pubkey: IdentityPublicKey,
    /// Plain-language reason.
    pub reason: String,
}

/// Issues a one-time, server-signed support link capped at 15 minutes (`900s`).
///
/// # Errors
/// Returns [`ServerError::BadRequest`] if `ttl_secs` is outside `1..=900` or
/// [`ServerError::NotFound`] if the device is not enrolled.
pub async fn create_support_link(
    user: AuthenticatedUser,
    State(state): State<AppState>,
    Json(req): Json<CreateSupportLinkRequest>,
) -> Result<Json<CreateSupportLinkResponse>, ServerError> {
    let ttl = req.ttl_secs.unwrap_or(SUPPORT_LINK_MAX_TTL_SECS);
    if ttl <= 0 || ttl > SUPPORT_LINK_MAX_TTL_SECS {
        return Err(ServerError::BadRequest(format!(
            "support link ttl_secs must be between 1 and {SUPPORT_LINK_MAX_TTL_SECS} seconds (15 minutes)"
        )));
    }

    let now = truncate_to_seconds((state.clock)());
    let expires_at = now + Duration::seconds(ttl);
    let device_id_str = req.device_id.as_compact().to_string();

    let pk_str: String = state.db.with_conn(|conn| {
        conn.query_row(
            "SELECT device_pubkey FROM devices WHERE device_id = ?1",
            [&device_id_str],
            |r| r.get(0),
        )
        .optional()?
        .ok_or_else(|| ServerError::NotFound("device not enrolled".to_string()))
    })?;
    let device_pubkey = IdentityPublicKey::from_prefixed_hex(&pk_str)?;

    let link_id = Uuid::now_v7().to_string();
    let mut raw_secret = [0u8; 24];
    rand::thread_rng().fill_bytes(&mut raw_secret);
    let token = format!("oud_sup_{}", hex::encode(raw_secret));
    let token_hash = hash_token(&token);

    let server_pubkey = state.server_keypair.public_key();
    let binding_bytes = support_link_binding_bytes(
        &link_id,
        &device_id_str,
        &state.server_domain,
        &server_pubkey,
        expires_at,
    );
    let server_signature = state.server_keypair.sign(&binding_bytes);
    let server_pubkey_hex = server_pubkey.to_prefixed_hex();
    let server_sig_hex = server_signature.to_prefixed_hex();

    state.db.with_conn(|conn| {
        conn.execute(
            "INSERT INTO support_links (
                link_id, token_hash, device_id, device_pubkey, server_domain, server_pubkey,
                server_signature, created_by_user_id, created_at_utc, expires_at_utc,
                redeemed_at_utc, redeemed_by_requester
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, NULL, NULL)",
            rusqlite::params![
                &link_id,
                &token_hash,
                &device_id_str,
                &pk_str,
                &state.server_domain,
                &server_pubkey_hex,
                &server_sig_hex,
                &user.user_id,
                &now.to_rfc3339(),
                &expires_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    })?;

    let support_url = format!("https://{}/support/{}", state.server_domain, token);

    Ok(Json(CreateSupportLinkResponse {
        link_id,
        token,
        support_url,
        device_id: req.device_id,
        device_pubkey,
        server_domain: state.server_domain,
        server_pubkey,
        server_signature,
        expires_at_utc: expires_at,
    }))
}

/// Redeems a one-time 15-minute support link, verifying server binding, expiry,
/// single-use constraint, requester blocklist, and rate limits.
///
/// # Errors
/// Fails closed on mismatched server identity (`403`), expired link (`410`),
/// already-used token (`409`), blocked requester (`403`), or rate-limit lockout (`429`).
pub async fn redeem_support_link(
    user: AuthenticatedUser,
    State(state): State<AppState>,
    Json(req): Json<RedeemSupportLinkRequest>,
) -> Result<Json<RedeemSupportLinkResponse>, ServerError> {
    let token = req.token.trim();
    if token.is_empty() {
        return Err(ServerError::BadRequest(
            "support link token is required".to_string(),
        ));
    }

    if let Some(expected_domain) = &req.expected_server_domain {
        if !expected_domain
            .trim()
            .eq_ignore_ascii_case(&state.server_domain)
        {
            return Err(ServerError::Forbidden(
                "support link server domain does not match enrolled server".to_string(),
            ));
        }
    }
    let server_pubkey = state.server_keypair.public_key();
    if let Some(expected_pk) = req.expected_server_pubkey {
        if expected_pk != server_pubkey {
            return Err(ServerError::Forbidden(
                "support link server public key does not match enrolled server".to_string(),
            ));
        }
    }

    let now = (state.clock)();
    let token_hash = hash_token(token);
    let requester_key = user.passkey_pubkey.to_prefixed_hex();

    let (link_id, device_id_str, device_pk_str) =
        load_and_verify_support_link(&state, &token_hash, server_pubkey, now)?;

    rate_limit::check_and_record_request(&state.db, &requester_key, &device_id_str, now)?;

    state.db.with_conn(|conn| {
        let updated = conn.execute(
            "UPDATE support_links
             SET redeemed_at_utc = ?1, redeemed_by_requester = ?2
             WHERE token_hash = ?3 AND redeemed_at_utc IS NULL",
            rusqlite::params![&now.to_rfc3339(), &requester_key, &token_hash],
        )?;
        if updated == 0 {
            return Err(ServerError::Conflict(
                "support link has already been redeemed".to_string(),
            ));
        }
        Ok(())
    })?;

    Ok(Json(RedeemSupportLinkResponse {
        link_id,
        session_id: Uuid::now_v7(),
        device_id: DeviceId::parse(&device_id_str)?,
        device_pubkey: IdentityPublicKey::from_prefixed_hex(&device_pk_str)?,
        server_domain: state.server_domain,
        server_pubkey,
    }))
}

fn load_and_verify_support_link(
    state: &AppState,
    token_hash: &str,
    server_pubkey: IdentityPublicKey,
    now: DateTime<Utc>,
) -> Result<(String, String, String), ServerError> {
    let (
        link_id,
        device_id_str,
        device_pk_str,
        stored_domain,
        stored_pk_str,
        stored_sig_str,
        created_str,
        expires_str,
        redeemed_at,
    ): (
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        Option<String>,
    ) = state.db.with_conn(|conn| {
        conn.query_row(
            "SELECT link_id, device_id, device_pubkey, server_domain, server_pubkey,
                    server_signature, created_at_utc, expires_at_utc, redeemed_at_utc
             FROM support_links WHERE token_hash = ?1",
            [token_hash],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                    r.get(6)?,
                    r.get(7)?,
                    r.get(8)?,
                ))
            },
        )
        .optional()?
        .ok_or_else(|| ServerError::NotFound("support link not found".to_string()))
    })?;

    if stored_domain != state.server_domain || stored_pk_str != server_pubkey.to_prefixed_hex() {
        return Err(ServerError::Forbidden(
            "support link bound to a different server identity".to_string(),
        ));
    }

    let created_at = parse_utc(&created_str)?;
    let expires_at = parse_utc(&expires_str)?;
    let sig = IdentitySignature::from_prefixed_hex(&stored_sig_str)?;
    let binding_bytes = support_link_binding_bytes(
        &link_id,
        &device_id_str,
        &stored_domain,
        &server_pubkey,
        expires_at,
    );
    server_pubkey
        .verify(&binding_bytes, &sig)
        .map_err(|_| ServerError::Forbidden("invalid support link server signature".to_string()))?;

    if redeemed_at.is_some() {
        return Err(ServerError::Conflict(
            "support link has already been redeemed".to_string(),
        ));
    }
    if (expires_at - created_at).num_seconds() > SUPPORT_LINK_MAX_TTL_SECS || now >= expires_at {
        return Err(ServerError::Gone(
            "support link expired after the 15-minute window".to_string(),
        ));
    }

    Ok((link_id, device_id_str, device_pk_str))
}

/// Blocks `requester_pubkey` from requesting sessions on `device_id`.
///
/// # Errors
/// Returns [`ServerError::BadRequest`] on empty reason or [`ServerError::NotFound`] if device
/// is not enrolled.
pub async fn block_requester_handler(
    _user: AuthenticatedUser,
    State(state): State<AppState>,
    Path(device_id_raw): Path<String>,
    Json(req): Json<BlockRequesterRequest>,
) -> Result<Json<rate_limit::BlockedRequesterRecord>, ServerError> {
    let device_id = DeviceId::parse(&device_id_raw)?;
    let reason = req.reason.trim();
    if reason.is_empty() {
        return Err(ServerError::BadRequest(
            "block reason is required".to_string(),
        ));
    }
    let now = (state.clock)();
    let dev_str = device_id.as_compact();
    ensure_device_exists(&state, dev_str)?;
    let rec = rate_limit::block_requester(
        &state.db,
        dev_str,
        &req.requester_pubkey.to_prefixed_hex(),
        reason,
        now,
    )?;
    Ok(Json(rec))
}

/// Unblocks `requester_pubkey` for `device_id`.
///
/// # Errors
/// Returns [`ServerError::NotFound`] if no block entry existed.
pub async fn unblock_requester_handler(
    _user: AuthenticatedUser,
    State(state): State<AppState>,
    Path((device_id_raw, requester_pubkey_raw)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, ServerError> {
    let device_id = DeviceId::parse(&device_id_raw)?;
    let requester_pubkey = IdentityPublicKey::from_prefixed_hex(&requester_pubkey_raw)?;
    rate_limit::unblock_requester(
        &state.db,
        device_id.as_compact(),
        &requester_pubkey.to_prefixed_hex(),
    )?;
    Ok(Json(serde_json::json!({ "unblocked": true })))
}

/// Lists all blocked requesters for `device_id`.
///
/// # Errors
/// Returns [`ServerError::BadRequest`] if `device_id` is invalid.
pub async fn list_blocked_requesters_handler(
    _user: AuthenticatedUser,
    State(state): State<AppState>,
    Path(device_id_raw): Path<String>,
) -> Result<Json<Vec<rate_limit::BlockedRequesterRecord>>, ServerError> {
    let device_id = DeviceId::parse(&device_id_raw)?;
    let records = rate_limit::list_blocked_requesters(&state.db, device_id.as_compact())?;
    Ok(Json(records))
}

/// Submits an abuse/scam report and optionally blocks the requester immediately.
///
/// # Errors
/// Returns [`ServerError::BadRequest`] on empty/oversized summary or [`ServerError::NotFound`]
/// if `device_id` is not enrolled.
pub async fn submit_abuse_report(
    user: AuthenticatedUser,
    State(state): State<AppState>,
    Json(req): Json<AbuseReportRequest>,
) -> Result<Json<AbuseReportResponse>, ServerError> {
    let cleaned_summary: String = req
        .summary
        .chars()
        .filter(|c| !c.is_control())
        .collect::<String>()
        .trim()
        .chars()
        .take(MAX_ABUSE_SUMMARY_CHARS)
        .collect();
    if cleaned_summary.is_empty() {
        return Err(ServerError::BadRequest(
            "abuse report summary is required".to_string(),
        ));
    }

    let dev_str = req.device_id.as_compact();
    ensure_device_exists(&state, dev_str)?;

    let now = (state.clock)();
    let report_id = Uuid::now_v7().to_string();
    let requester_key = req.requester_pubkey.to_prefixed_hex();
    let also_block = req.also_block.unwrap_or(true);

    if also_block {
        rate_limit::block_requester(
            &state.db,
            dev_str,
            &requester_key,
            &format!(
                "Abuse report ({}): {}",
                req.category.as_str(),
                cleaned_summary
            ),
            now,
        )?;
    }

    state.db.with_conn(|conn| {
        conn.execute(
            "INSERT INTO abuse_reports (
                report_id, device_id, reporter_user_id, requester_key,
                server_domain, category, summary, blocked, created_at_utc
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            rusqlite::params![
                &report_id,
                dev_str,
                &user.user_id,
                &requester_key,
                &state.server_domain,
                req.category.as_str(),
                &cleaned_summary,
                i64::from(also_block),
                &now.to_rfc3339(),
            ],
        )?;
        Ok(())
    })?;

    Ok(Json(AbuseReportResponse {
        report_id,
        blocked: also_block,
        created_at_utc: now,
        sla_target: "24h".to_string(),
    }))
}

/// Lists submitted abuse reports.
///
/// # Errors
/// Returns [`ServerError::Internal`] on SQLite query errors.
pub async fn list_abuse_reports(
    _user: AuthenticatedUser,
    State(state): State<AppState>,
) -> Result<Json<Vec<AbuseReportRecord>>, ServerError> {
    state.db.with_conn(|conn| {
        let mut stmt = conn.prepare(
            "SELECT report_id, device_id, reporter_user_id, requester_key,
                    server_domain, category, summary, blocked, created_at_utc
             FROM abuse_reports ORDER BY created_at_utc DESC",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
                r.get::<_, String>(6)?,
                r.get::<_, i64>(7)?,
                r.get::<_, String>(8)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (
                report_id,
                dev_str,
                reporter_user_id,
                req_pk_str,
                server_domain,
                category,
                summary,
                blocked_i64,
                created_at_utc,
            ) = row?;
            out.push(AbuseReportRecord {
                report_id,
                device_id: DeviceId::parse(&dev_str)?,
                reporter_user_id,
                requester_pubkey: IdentityPublicKey::from_prefixed_hex(&req_pk_str)?,
                server_domain,
                category,
                summary,
                blocked: blocked_i64 != 0,
                created_at_utc,
            });
        }
        Ok(Json(out))
    })
}

fn ensure_device_exists(state: &AppState, device_id: &str) -> Result<(), ServerError> {
    state.db.with_conn(|conn| {
        let exists: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM devices WHERE device_id = ?1)",
            [device_id],
            |r| r.get(0),
        )?;
        if !exists {
            return Err(ServerError::NotFound("device not enrolled".to_string()));
        }
        Ok(())
    })
}

fn support_link_binding_bytes(
    link_id: &str,
    device_id: &str,
    server_domain: &str,
    server_pubkey: &IdentityPublicKey,
    expires_at: DateTime<Utc>,
) -> Vec<u8> {
    format!(
        "oudos-support-link-v1:{link_id}:{device_id}:{server_domain}:{}:{}",
        server_pubkey.to_prefixed_hex(),
        expires_at.timestamp()
    )
    .into_bytes()
}

fn truncate_to_seconds(dt: DateTime<Utc>) -> DateTime<Utc> {
    DateTime::from_timestamp(dt.timestamp(), 0).unwrap_or(dt)
}

fn hash_token(token: &str) -> String {
    let digest = Sha256::digest(token.as_bytes());
    hex::encode(digest)
}

fn parse_utc(input: &str) -> Result<DateTime<Utc>, ServerError> {
    DateTime::parse_from_rfc3339(input)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|e| ServerError::Internal(format!("invalid timestamp in db: {e}")))
}
