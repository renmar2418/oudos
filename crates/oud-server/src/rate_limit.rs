//! Per-requester and per-device rate limiting and 3-strike lockout.

use chrono::{DateTime, Duration, Utc};
use rusqlite::OptionalExtension;

use crate::db::Database;
use crate::error::ServerError;

/// Maximum connection requests allowed per 60-second window.
pub const MAX_REQUESTS_PER_WINDOW: i64 = 10;

/// Sliding window duration in seconds.
pub const RATE_WINDOW_SECS: i64 = 60;

/// Number of denials or SAS mismatches before automatic lockout.
pub const MAX_DENIAL_STREAK: i64 = 3;

/// Cooldown lockout duration in seconds (15 minutes).
pub const LOCKOUT_DURATION_SECS: i64 = 900;

/// Checks whether `(requester_key, device_id)` is allowed to initiate a
/// connection request at `now`, and increments the request counter.
///
/// # Errors
/// Returns [`ServerError::RateLimited`] if locked out or over the window cap.
pub fn check_and_record_request(
    db: &Database,
    requester_key: &str,
    device_id: &str,
    now: DateTime<Utc>,
) -> Result<(), ServerError> {
    db.with_conn(|conn| {
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
