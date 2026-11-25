//! Append-only, tamper-evident hash-chained audit log API and CSV export.

use axum::extract::{Query, State};
use axum::http::header::{CONTENT_DISPOSITION, CONTENT_TYPE};
use axum::http::HeaderMap;
use axum::response::IntoResponse;
use axum::Json;
use oud_protocol::audit::{compute_entry_hash, SignedAuditEntry, GENESIS_HASH};
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};

use crate::auth::AuthenticatedUser;
use crate::error::ServerError;
use crate::AppState;

/// Query parameters for filtering `GET /api/v1/audit`.
#[derive(Debug, Deserialize)]
pub struct AuditQuery {
    /// Filter by host `device_pubkey` (`ed25519:...`).
    pub device_pubkey: Option<String>,
    /// Filter by `session_id` UUID.
    pub session_id: Option<String>,
}

/// Response returned after appending a [`SignedAuditEntry`].
#[derive(Debug, Serialize, Deserialize)]
pub struct AppendAuditResponse {
    /// Accepted sequence number.
    pub sequence: u64,
    /// Hex-encoded `entry_hash`.
    pub entry_hash_hex: String,
}

/// Appends a host-signed [`SignedAuditEntry`] to the tamper-evident server log.
///
/// Verifies:
/// 1. `server_domain` matches this server's configured domain.
/// 2. `entry.verify_signature()` succeeds against `event.device_pubkey`.
/// 3. `entry.entry_hash` equals `entry.compute_entry_hash()`.
/// 4. `entry.sequence` and `entry.prev_hash` continue the stored chain without gaps.
///
/// # Errors
/// Returns [`ServerError::BadRequest`] on signature/hash mismatch or
/// [`ServerError::Conflict`] on `sequence` / `prev_hash` mismatch.
pub async fn append_audit_entry(
    State(state): State<AppState>,
    Json(entry): Json<SignedAuditEntry>,
) -> Result<Json<AppendAuditResponse>, ServerError> {
    if entry.event.server_domain != state.server_domain {
        return Err(ServerError::BadRequest(format!(
            "audit entry server_domain `{}` does not match `{}`",
            entry.event.server_domain, state.server_domain
        )));
    }

    let computed_hash = compute_entry_hash(&entry.prev_hash, &entry.event)?;
    if computed_hash != entry.entry_hash {
        return Err(ServerError::BadRequest(
            "audit entry_hash mismatch".to_string(),
        ));
    }
    entry
        .event
        .device_pubkey
        .verify(&entry.entry_hash, &entry.host_signature)
        .map_err(|_| ServerError::BadRequest("invalid audit host_signature".to_string()))?;

    let event_json = serde_json::to_string(&entry.event)
        .map_err(|e| ServerError::Internal(format!("serialize audit event: {e}")))?;
    let kind_json = serde_json::to_string(&entry.event.kind)
        .map_err(|e| ServerError::Internal(format!("serialize audit kind: {e}")))?;

    let prev_hex = hex::encode(entry.prev_hash);
    let entry_hex = hex::encode(entry.entry_hash);

    state.db.with_conn(|conn| {
        let last: Option<(i64, String)> = conn
            .query_row(
                "SELECT sequence, entry_hash_hex FROM audit_entries ORDER BY sequence DESC LIMIT 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;

        let (expected_seq, expected_prev_hex) = match last {
            None => (0_u64, hex::encode(GENESIS_HASH)),
            Some((seq, hash_hex)) => {
                let next_seq = u64::try_from(seq)
                    .map_err(|_| ServerError::Internal("negative sequence in db".to_string()))?
                    + 1;
                (next_seq, hash_hex)
            }
        };

        if entry.sequence != expected_seq || prev_hex != expected_prev_hex {
            return Err(ServerError::Conflict(format!(
                "audit chain mismatch: expected seq={expected_seq} prev={expected_prev_hex}, got seq={} prev={prev_hex}",
                entry.sequence
            )));
        }

        let seq_i64 = i64::try_from(entry.sequence)
            .map_err(|_| ServerError::BadRequest("sequence exceeds i64::MAX".to_string()))?;

        conn.execute(
            "INSERT INTO audit_entries (
                sequence, event_id, timestamp_utc, session_id, device_pubkey,
                actor_pubkey, server_domain, kind, prev_hash_hex, entry_hash_hex,
                event_json, host_signature
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            rusqlite::params![
                seq_i64,
                entry.event.event_id.to_string(),
                entry.event.timestamp_utc.to_rfc3339(),
                entry.event.session_id.to_string(),
                entry.event.device_pubkey.to_prefixed_hex(),
                entry.event.actor_pubkey.to_prefixed_hex(),
                entry.event.server_domain,
                kind_json,
                prev_hex,
                entry_hex,
                event_json,
                entry.host_signature.to_prefixed_hex(),
            ],
        )?;

        Ok(())
    })?;

    Ok(Json(AppendAuditResponse {
        sequence: entry.sequence,
        entry_hash_hex: entry_hex,
    }))
}

/// Lists stored [`SignedAuditEntry`] records (requires MFA-verified auth).
///
/// # Errors
/// Returns [`ServerError::Internal`] on SQLite or deserialization error.
pub async fn list_audit_entries(
    _user: AuthenticatedUser,
    State(state): State<AppState>,
    Query(query): Query<AuditQuery>,
) -> Result<Json<Vec<SignedAuditEntry>>, ServerError> {
    let entries = load_all_entries(&state)?;
    let filtered = entries
        .into_iter()
        .filter(|e| {
            if let Some(ref dev) = query.device_pubkey {
                if &e.event.device_pubkey.to_prefixed_hex() != dev {
                    return false;
                }
            }
            if let Some(ref sid) = query.session_id {
                if &e.event.session_id.to_string() != sid {
                    return false;
                }
            }
            true
        })
        .collect();

    Ok(Json(filtered))
}

/// Exports the full hash-chained audit log as verifiable CSV.
///
/// # Errors
/// Returns [`ServerError::Internal`] on SQLite or deserialization error.
pub async fn export_audit_csv(
    _user: AuthenticatedUser,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ServerError> {
    let entries = load_all_entries(&state)?;
    let mut csv = String::from(
        "sequence,event_id,timestamp_utc,session_id,device_pubkey,actor_pubkey,server_domain,kind,prev_hash,entry_hash,host_signature\n",
    );

    for entry in entries {
        let kind_str = serde_json::to_string(&entry.event.kind)
            .map_err(|e| ServerError::Internal(format!("serialize kind: {e}")))?;
        let line = format!(
            "{},{},{},{},{},{},{},{},{},{},{}\n",
            entry.sequence,
            csv_escape(&entry.event.event_id.to_string()),
            csv_escape(&entry.event.timestamp_utc.to_rfc3339()),
            csv_escape(&entry.event.session_id.to_string()),
            csv_escape(&entry.event.device_pubkey.to_prefixed_hex()),
            csv_escape(&entry.event.actor_pubkey.to_prefixed_hex()),
            csv_escape(&entry.event.server_domain),
            csv_escape(&kind_str),
            hex::encode(entry.prev_hash),
            hex::encode(entry.entry_hash),
            csv_escape(&entry.host_signature.to_prefixed_hex()),
        );
        csv.push_str(&line);
    }

    let mut headers = HeaderMap::new();
    headers.insert(
        CONTENT_TYPE,
        axum::http::HeaderValue::from_static("text/csv; charset=utf-8"),
    );
    headers.insert(
        CONTENT_DISPOSITION,
        axum::http::HeaderValue::from_static("attachment; filename=\"oudos-audit-log.csv\""),
    );

    Ok((headers, csv))
}

fn load_all_entries(state: &AppState) -> Result<Vec<SignedAuditEntry>, ServerError> {
    state.db.with_conn(|conn| {
        let mut stmt = conn.prepare(
            "SELECT sequence, prev_hash_hex, entry_hash_hex, event_json, host_signature
             FROM audit_entries ORDER BY sequence ASC",
        )?;

        let rows = stmt.query_map([], |row| {
            let seq: i64 = row.get(0)?;
            let prev_hex: String = row.get(1)?;
            let entry_hex: String = row.get(2)?;
            let event_json: String = row.get(3)?;
            let sig_str: String = row.get(4)?;
            Ok((seq, prev_hex, entry_hex, event_json, sig_str))
        })?;

        let mut out = Vec::new();
        for row in rows {
            let (seq, prev_hex, entry_hex, event_json, sig_str) = row?;
            let sequence = u64::try_from(seq)
                .map_err(|_| ServerError::Internal("negative sequence in db".to_string()))?;
            let prev_hash = decode_32(&prev_hex)?;
            let entry_hash = decode_32(&entry_hex)?;
            let event = serde_json::from_str(&event_json)
                .map_err(|e| ServerError::Internal(format!("corrupt event_json: {e}")))?;
            let host_signature =
                oud_protocol::identity::IdentitySignature::from_prefixed_hex(&sig_str)?;

            out.push(SignedAuditEntry {
                sequence,
                prev_hash,
                entry_hash,
                event,
                host_signature,
            });
        }
        Ok(out)
    })
}

fn decode_32(hex_str: &str) -> Result<[u8; 32], ServerError> {
    let bytes = hex::decode(hex_str)
        .map_err(|e| ServerError::Internal(format!("invalid hex hash in db: {e}")))?;
    let arr: [u8; 32] = bytes
        .try_into()
        .map_err(|_| ServerError::Internal("hash length != 32 in db".to_string()))?;
    Ok(arr)
}

fn csv_escape(value: &str) -> String {
    if value.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}
