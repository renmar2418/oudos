//! SQLite persistence layer for `oud-server`.

use std::sync::Mutex;

use rusqlite::Connection;

use crate::error::ServerError;

/// Thread-safe SQLite database handle.
pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    /// Opens a SQLite database at `path` (or `:memory:`) and runs migrations.
    ///
    /// # Errors
    /// Returns [`ServerError::Internal`] if SQLite initialization fails.
    pub fn open(path: &str) -> Result<Self, ServerError> {
        let conn = if path == ":memory:" {
            Connection::open_in_memory()?
        } else {
            Connection::open(path)?
        };
        conn.execute_batch(SCHEMA_SQL)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Executes a closure with exclusive access to the underlying SQLite connection.
    ///
    /// # Errors
    /// Returns [`ServerError::Internal`] if the mutex is poisoned.
    pub fn with_conn<T, F>(&self, f: F) -> Result<T, ServerError>
    where
        F: FnOnce(&Connection) -> Result<T, ServerError>,
    {
        let guard = self
            .conn
            .lock()
            .map_err(|_| ServerError::Internal("database mutex poisoned".to_string()))?;
        f(&guard)
    }
}

const SCHEMA_SQL: &str = r"
PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS users (
    user_id TEXT PRIMARY KEY,
    email TEXT NOT NULL UNIQUE,
    passkey_pubkey TEXT NOT NULL,
    mfa_secret_hint TEXT NOT NULL,
    created_at_utc TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS auth_challenges (
    challenge_id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    nonce_hex TEXT NOT NULL,
    expires_at_utc TEXT NOT NULL,
    used INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY (user_id) REFERENCES users(user_id)
);

CREATE TABLE IF NOT EXISTS auth_sessions (
    token_hash TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    mfa_verified INTEGER NOT NULL DEFAULT 0,
    expires_at_utc TEXT NOT NULL,
    FOREIGN KEY (user_id) REFERENCES users(user_id)
);

CREATE TABLE IF NOT EXISTS devices (
    device_id TEXT PRIMARY KEY,
    device_pubkey TEXT NOT NULL UNIQUE,
    alias TEXT NOT NULL,
    os_name TEXT NOT NULL,
    owner_user_id TEXT NOT NULL,
    presence TEXT NOT NULL,
    enrolled_at_utc TEXT NOT NULL,
    last_seen_utc TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS policies (
    scope TEXT NOT NULL,
    target_id TEXT NOT NULL,
    layer_json TEXT NOT NULL,
    updated_at_utc TEXT NOT NULL,
    PRIMARY KEY (scope, target_id)
);

CREATE TABLE IF NOT EXISTS rate_limits (
    requester_key TEXT NOT NULL,
    device_id TEXT NOT NULL,
    denial_streak INTEGER NOT NULL DEFAULT 0,
    request_count INTEGER NOT NULL DEFAULT 0,
    window_start_utc TEXT NOT NULL,
    locked_until_utc TEXT,
    PRIMARY KEY (requester_key, device_id)
);

CREATE TABLE IF NOT EXISTS device_rate_limits (
    device_id TEXT PRIMARY KEY,
    request_count INTEGER NOT NULL DEFAULT 0,
    window_start_utc TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS requester_rate_limits (
    requester_key TEXT PRIMARY KEY,
    request_count INTEGER NOT NULL DEFAULT 0,
    window_start_utc TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS blocked_requesters (
    device_id TEXT NOT NULL,
    requester_key TEXT NOT NULL,
    reason TEXT NOT NULL,
    blocked_at_utc TEXT NOT NULL,
    PRIMARY KEY (device_id, requester_key)
);

CREATE TABLE IF NOT EXISTS support_links (
    link_id TEXT PRIMARY KEY,
    token_hash TEXT NOT NULL UNIQUE,
    device_id TEXT NOT NULL,
    device_pubkey TEXT NOT NULL,
    server_domain TEXT NOT NULL,
    server_pubkey TEXT NOT NULL,
    server_signature TEXT NOT NULL,
    created_by_user_id TEXT NOT NULL,
    created_at_utc TEXT NOT NULL,
    expires_at_utc TEXT NOT NULL,
    redeemed_at_utc TEXT,
    redeemed_by_requester TEXT
);

CREATE TABLE IF NOT EXISTS abuse_reports (
    report_id TEXT PRIMARY KEY,
    device_id TEXT NOT NULL,
    reporter_user_id TEXT NOT NULL,
    requester_key TEXT NOT NULL,
    server_domain TEXT NOT NULL,
    category TEXT NOT NULL,
    summary TEXT NOT NULL,
    blocked INTEGER NOT NULL DEFAULT 1,
    created_at_utc TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS audit_entries (
    sequence INTEGER PRIMARY KEY,
    event_id TEXT NOT NULL UNIQUE,
    timestamp_utc TEXT NOT NULL,
    session_id TEXT NOT NULL,
    device_pubkey TEXT NOT NULL,
    actor_pubkey TEXT NOT NULL,
    server_domain TEXT NOT NULL,
    kind TEXT NOT NULL,
    prev_hash_hex TEXT NOT NULL,
    entry_hash_hex TEXT NOT NULL,
    event_json TEXT NOT NULL,
    host_signature TEXT NOT NULL
);
";
