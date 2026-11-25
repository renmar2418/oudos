//! Per-requester and per-device rate limiting, 3-strike lockout, and requester blocking.

use chrono::{DateTime, Duration, Utc};
use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::db::Database;
use crate::error::ServerError;

/// Maximum connection requests allowed per `(requester, device)` in a 60-second window.
pub const MAX_REQUESTS_PER_WINDOW: i64 = 10;

/// Maximum aggregate requests allowed per target device across all requesters in 60 seconds.
pub const MAX_DEVICE_REQUESTS_PER_WINDOW: i64 = 25;

/// Maximum global requests allowed per requester across all devices in 60 seconds.
pub const MAX_REQUESTER_REQUESTS_PER_WINDOW: i64 = 20;

/// Sliding window duration in seconds.
pub const RATE_WINDOW_SECS: i64 = 60;

/// Number of denials or SAS mismatches before automatic lockout.
pub const MAX_DENIAL_STREAK: i64 = 3;

/// Cooldown lockout duration in seconds (15 minutes).
pub const LOCKOUT_DURATION_SECS: i64 = 900;

/// Stored blocked-requester record for a device.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockedRequesterRecord {
    /// Target 9-digit device ID.
    pub device_id: String,
    /// Blocked requester Ed25519 public key (`ed25519:...`).
    pub requester_key: String,
    /// Plain-language block reason.
    pub reason: String,
    /// UTC timestamp when blocked.
    pub blocked_at_utc: String,
}

/// Checks blocklist, 3-strike lockout, per-`(requester, device)` cap, per-device cap,
/// and per-requester global cap at `now`, incrementing counters when permitted.
///
/// # Errors
/// Returns [`ServerError::Forbidden`] if blocked or [`ServerError::RateLimited`] if locked out
/// or exceeding any sliding-window cap.
pub fn check_and_record_request(
    db: &Database,
    requester_key: &str,
    device_id: &str,
    now: DateTime<Utc>,
) -> Result<(), ServerError> {
    db.with_conn(|conn| {
        let blocked_reason: Option<String> = conn
            .query_row(
                "SELECT reason FROM blocked_requesters WHERE device_id = ?1 AND requester_key = ?2",
                [device_id, requester_key],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(reason) = blocked_reason {
            return Err(ServerError::Forbidden(format!(
                "requester blocked by device owner: {reason}"
            )));
        }

        check_pair_rate_limit(conn, requester_key, device_id, now)?;
        check_single_key_window(
            conn,
            "device_rate_limits",
            "device_id",
            device_id,
            MAX_DEVICE_REQUESTS_PER_WINDOW,
            "device rate limit exceeded in 60s window",
            now,
        )?;
        check_single_key_window(
            conn,
            "requester_rate_limits",
            "requester_key",
            requester_key,
            MAX_REQUESTER_REQUESTS_PER_WINDOW,
            "requester global rate limit exceeded in 60s window",
            now,
        )?;
        Ok(())
    })
}

fn check_pair_rate_limit(
    conn: &Connection,
    requester_key: &str,
    device_id: &str,
    now: DateTime<Utc>,
) -> Result<(), ServerError> {
    let existing: Option<(i64, i64, String, Option<String>)> = conn
        .query_row(
            "SELECT denial_streak, request_count, window_start_utc, locked_until_utc
             FROM rate_limits WHERE requester_key = ?1 AND device_id = ?2",
            [requester_key, device_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()?;

    match existing {
        None => {
            conn.execute(
                "INSERT INTO rate_limits (requester_key, device_id, denial_streak, request_count, window_start_utc, locked_until_utc)
                 VALUES (?1, ?2, 0, 1, ?3, NULL)",
                [requester_key, device_id, &now.to_rfc3339()],
            )?;
            Ok(())
        }
        Some((denial_streak, mut req_count, window_start_str, locked_until_str)) => {
            if let Some(locked_str) = locked_until_str {
                let locked_until = parse_utc(&locked_str)?;
                if now < locked_until {
                    return Err(ServerError::RateLimited(format!(
                        "requester locked out until {}",
                        locked_until.to_rfc3339()
                    )));
                }
            }

            let mut window_start = parse_utc(&window_start_str)?;
            if now - window_start >= Duration::seconds(RATE_WINDOW_SECS) {
                window_start = now;
                req_count = 0;
            }

            if req_count >= MAX_REQUESTS_PER_WINDOW {
                return Err(ServerError::RateLimited(
                    "too many connection requests in 60s window".to_string(),
                ));
            }

            req_count += 1;
            conn.execute(
                "UPDATE rate_limits
                 SET request_count = ?1, window_start_utc = ?2, denial_streak = ?3, locked_until_utc = NULL
                 WHERE requester_key = ?4 AND device_id = ?5",
                rusqlite::params![
                    req_count,
                    window_start.to_rfc3339(),
                    denial_streak,
                    requester_key,
                    device_id
                ],
            )?;
            Ok(())
        }
    }
}

fn check_single_key_window(
    conn: &Connection,
    table: &str,
    key_col: &str,
    key_val: &str,
    limit: i64,
    err_msg: &str,
    now: DateTime<Utc>,
) -> Result<(), ServerError> {
    let select_sql =
        format!("SELECT request_count, window_start_utc FROM {table} WHERE {key_col} = ?1");
    let existing: Option<(i64, String)> = conn
        .query_row(&select_sql, [key_val], |r| Ok((r.get(0)?, r.get(1)?)))
        .optional()?;

    match existing {
        None => {
            let insert_sql = format!(
                "INSERT INTO {table} ({key_col}, request_count, window_start_utc) VALUES (?1, 1, ?2)"
            );
            conn.execute(&insert_sql, [key_val, &now.to_rfc3339()])?;
            Ok(())
        }
        Some((mut count, start_str)) => {
            let mut window_start = parse_utc(&start_str)?;
            if now - window_start >= Duration::seconds(RATE_WINDOW_SECS) {
                window_start = now;
                count = 0;
            }
            if count >= limit {
                return Err(ServerError::RateLimited(err_msg.to_string()));
            }
            count += 1;
            let update_sql = format!(
                "UPDATE {table} SET request_count = ?1, window_start_utc = ?2 WHERE {key_col} = ?3"
            );
            conn.execute(
                &update_sql,
                rusqlite::params![count, window_start.to_rfc3339(), key_val],
            )?;
            Ok(())
        }
    }
}

/// Blocks `requester_key` on `device_id`.
///
/// # Errors
/// Returns [`ServerError::Internal`] on SQLite failure.
pub fn block_requester(
    db: &Database,
    device_id: &str,
    requester_key: &str,
    reason: &str,
    now: DateTime<Utc>,
) -> Result<BlockedRequesterRecord, ServerError> {
    let blocked_at = now.to_rfc3339();
    db.with_conn(|conn| {
        conn.execute(
            "INSERT INTO blocked_requesters (device_id, requester_key, reason, blocked_at_utc)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(device_id, requester_key) DO UPDATE SET
               reason = excluded.reason,
               blocked_at_utc = excluded.blocked_at_utc",
            [device_id, requester_key, reason, &blocked_at],
        )?;
        Ok(BlockedRequesterRecord {
            device_id: device_id.to_string(),
            requester_key: requester_key.to_string(),
            reason: reason.to_string(),
            blocked_at_utc: blocked_at,
        })
    })
}

/// Unblocks `requester_key` on `device_id`.
///
/// # Errors
/// Returns [`ServerError::NotFound`] if the requester was not blocked.
pub fn unblock_requester(
    db: &Database,
    device_id: &str,
    requester_key: &str,
) -> Result<(), ServerError> {
    db.with_conn(|conn| {
        let deleted = conn.execute(
            "DELETE FROM blocked_requesters WHERE device_id = ?1 AND requester_key = ?2",
            [device_id, requester_key],
        )?;
        if deleted == 0 {
            return Err(ServerError::NotFound(
                "blocked requester entry not found".to_string(),
            ));
        }
        Ok(())
    })
}

/// Lists blocked requesters for `device_id`.
///
/// # Errors
/// Returns [`ServerError::Internal`] on SQLite failure.
pub fn list_blocked_requesters(
    db: &Database,
    device_id: &str,
) -> Result<Vec<BlockedRequesterRecord>, ServerError> {
    db.with_conn(|conn| {
        let mut stmt = conn.prepare(
            "SELECT device_id, requester_key, reason, blocked_at_utc
             FROM blocked_requesters WHERE device_id = ?1 ORDER BY blocked_at_utc DESC",
        )?;
        let rows = stmt.query_map([device_id], |r| {
            Ok(BlockedRequesterRecord {
                device_id: r.get(0)?,
                requester_key: r.get(1)?,
                reason: r.get(2)?,
                blocked_at_utc: r.get(3)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    })
}

/// Records a host denial or SAS mismatch for `(requester_key, device_id)`.
///
/// After [`MAX_DENIAL_STREAK`] consecutive denials/mismatches, locks out the
/// requester for [`LOCKOUT_DURATION_SECS`].
///
/// # Errors
/// Returns [`ServerError::Internal`] on SQLite failure.
pub fn record_denial_or_mismatch(
    db: &Database,
    requester_key: &str,
    device_id: &str,
    now: DateTime<Utc>,
) -> Result<(i64, Option<DateTime<Utc>>), ServerError> {
    db.with_conn(|conn| {
        let existing: Option<i64> = conn
            .query_row(
                "SELECT denial_streak FROM rate_limits WHERE requester_key = ?1 AND device_id = ?2",
                [requester_key, device_id],
                |row| row.get(0),
            )
            .optional()?;

        let new_streak = existing.unwrap_or(0) + 1;
        let locked_until = if new_streak >= MAX_DENIAL_STREAK {
            Some(now + Duration::seconds(LOCKOUT_DURATION_SECS))
        } else {
            None
        };
        let locked_str = locked_until.map(|dt| dt.to_rfc3339());

        conn.execute(
            "INSERT INTO rate_limits (requester_key, device_id, denial_streak, request_count, window_start_utc, locked_until_utc)
             VALUES (?1, ?2, ?3, 0, ?4, ?5)
             ON CONFLICT(requester_key, device_id) DO UPDATE SET
               denial_streak = excluded.denial_streak,
               locked_until_utc = excluded.locked_until_utc",
            rusqlite::params![
                requester_key,
                device_id,
                new_streak,
                now.to_rfc3339(),
                locked_str
            ],
        )?;

        Ok((new_streak, locked_until))
    })
}

/// Resets the denial streak after a successful host-signed grant.
///
/// # Errors
/// Returns [`ServerError::Internal`] on SQLite failure.
pub fn reset_denial_streak(
    db: &Database,
    requester_key: &str,
    device_id: &str,
) -> Result<(), ServerError> {
    db.with_conn(|conn| {
        conn.execute(
            "UPDATE rate_limits SET denial_streak = 0, locked_until_utc = NULL
             WHERE requester_key = ?1 AND device_id = ?2",
            [requester_key, device_id],
        )?;
        Ok(())
    })
}

fn parse_utc(input: &str) -> Result<DateTime<Utc>, ServerError> {
    DateTime::parse_from_rfc3339(input)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|e| ServerError::Internal(format!("invalid timestamp in db: {e}")))
}
