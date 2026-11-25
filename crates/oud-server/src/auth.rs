//! Passkey (Ed25519 WebAuthn-style challenge-response) and MFA authentication.

use axum::async_trait;
use axum::extract::{FromRequestParts, State};
use axum::http::header::AUTHORIZATION;
use axum::http::request::Parts;
use axum::Json;
use chrono::{DateTime, Duration, Utc};
use oud_protocol::identity::{IdentityPublicKey, IdentitySignature};
use rand::RngCore;
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::error::ServerError;
use crate::AppState;

/// Challenge lifetime in seconds (2 minutes).
const CHALLENGE_TTL_SECS: i64 = 120;

/// Bearer session lifetime in seconds (8 hours).
const SESSION_TTL_SECS: i64 = 8 * 3600;

/// Authenticated viewer/admin user extracted from `Authorization: Bearer <token>`.
#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    /// User UUID.
    pub user_id: String,
    /// User email address.
    pub email: String,
    /// User's registered passkey public key.
    pub passkey_pubkey: IdentityPublicKey,
}

#[async_trait]
impl FromRequestParts<AppState> for AuthenticatedUser {
    type Rejection = ServerError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let auth_header = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| ServerError::Unauthorized("missing Authorization header".to_string()))?;

        let token = auth_header
            .strip_prefix("Bearer ")
            .ok_or_else(|| ServerError::Unauthorized("expected Bearer token".to_string()))?;

        let token_hash = hash_token(token);
        let now = Utc::now();

        state.db.with_conn(|conn| {
            let row: Option<(String, String, String, i64, String)> = conn
                .query_row(
                    "SELECT u.user_id, u.email, u.passkey_pubkey, s.mfa_verified, s.expires_at_utc
                     FROM auth_sessions s
                     JOIN users u ON u.user_id = s.user_id
                     WHERE s.token_hash = ?1",
                    [&token_hash],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
                )
                .optional()?;

            let (user_id, email, pubkey_str, mfa_verified, expires_str) = row.ok_or_else(|| {
                ServerError::Unauthorized("invalid or unknown session token".to_string())
            })?;

            if mfa_verified != 1 {
                return Err(ServerError::Unauthorized(
                    "MFA verification required".to_string(),
                ));
            }

            let expires_at = DateTime::parse_from_rfc3339(&expires_str)
                .map(|dt| dt.with_timezone(&Utc))
                .map_err(|e| ServerError::Internal(format!("invalid session expiry: {e}")))?;

            if now >= expires_at {
                return Err(ServerError::Unauthorized(
                    "session token expired".to_string(),
                ));
            }

            let passkey_pubkey = IdentityPublicKey::from_prefixed_hex(&pubkey_str)?;
            Ok(Self {
                user_id,
                email,
                passkey_pubkey,
            })
        })
    }
}

/// Request payload for `POST /api/v1/auth/register`.
#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    /// User email address.
    pub email: String,
    /// Ed25519 passkey public key (`ed25519:...`).
    pub passkey_pubkey: IdentityPublicKey,
    /// Enrolled 6-digit MFA code/secret for `v0.1` step-up verification.
    pub mfa_secret: String,
}

/// Response payload for `POST /api/v1/auth/register`.
#[derive(Debug, Serialize, Deserialize)]
pub struct RegisterResponse {
    /// Newly created user UUID.
    pub user_id: String,
    /// User email address.
    pub email: String,
}

/// Request payload for `POST /api/v1/auth/challenge`.
#[derive(Debug, Deserialize)]
pub struct ChallengeRequest {
    /// Registered user email.
    pub email: String,
}

/// Response payload for `POST /api/v1/auth/challenge`.
#[derive(Debug, Serialize, Deserialize)]
pub struct ChallengeResponse {
    /// Challenge identifier.
    pub challenge_id: String,
    /// Random 32-byte hex nonce to sign with the user's passkey.
    pub nonce_hex: String,
    /// UTC expiration timestamp.
    pub expires_at_utc: DateTime<Utc>,
}

/// Request payload for `POST /api/v1/auth/verify`.
#[derive(Debug, Deserialize)]
pub struct VerifyRequest {
    /// Challenge identifier from `/api/v1/auth/challenge`.
    pub challenge_id: String,
    /// Ed25519 signature over `challenge_id || ":" || nonce_hex`.
    pub signature: IdentitySignature,
    /// MFA verification code matching the enrolled MFA secret.
    pub mfa_code: String,
}

/// Response payload for `POST /api/v1/auth/verify`.
#[derive(Debug, Serialize, Deserialize)]
pub struct VerifyResponse {
    /// Bearer token for authenticated API calls.
    pub access_token: String,
    /// Authenticated user ID.
    pub user_id: String,
    /// UTC expiration timestamp.
    pub expires_at_utc: DateTime<Utc>,
}

/// Registers a new user with a passkey public key and MFA secret.
///
/// # Errors
/// Returns [`ServerError::BadRequest`] on invalid input or [`ServerError::Conflict`]
/// if the email is already registered.
pub async fn register_user(
    State(state): State<AppState>,
    Json(req): Json<RegisterRequest>,
) -> Result<Json<RegisterResponse>, ServerError> {
    let email = req.email.trim().to_lowercase();
    if email.is_empty() || !email.contains('@') {
        return Err(ServerError::BadRequest("valid email required".to_string()));
    }
    if req.mfa_secret.trim().len() < 6 {
        return Err(ServerError::BadRequest(
            "mfa_secret must be at least 6 characters".to_string(),
        ));
    }

    let user_id = Uuid::now_v7().to_string();
    let pubkey_hex = req.passkey_pubkey.to_prefixed_hex();
    let mfa_hash = hash_token(req.mfa_secret.trim());
    let now = Utc::now().to_rfc3339();

    state.db.with_conn(|conn| {
        let exists: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM users WHERE email = ?1)",
            [&email],
            |r| r.get(0),
        )?;
        if exists {
            return Err(ServerError::Conflict(
                "email already registered".to_string(),
            ));
        }

        conn.execute(
            "INSERT INTO users (user_id, email, passkey_pubkey, mfa_secret_hint, created_at_utc)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            [&user_id, &email, &pubkey_hex, &mfa_hash, &now],
        )?;
        Ok(())
    })?;

    Ok(Json(RegisterResponse { user_id, email }))
}

/// Issues a single-use passkey login challenge for `email`.
///
/// # Errors
/// Returns [`ServerError::NotFound`] if the user does not exist.
pub async fn issue_challenge(
    State(state): State<AppState>,
    Json(req): Json<ChallengeRequest>,
) -> Result<Json<ChallengeResponse>, ServerError> {
    let email = req.email.trim().to_lowercase();
    let challenge_id = Uuid::now_v7().to_string();
    let mut nonce = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut nonce);
    let nonce_hex = hex::encode(nonce);
    let expires_at = Utc::now() + Duration::seconds(CHALLENGE_TTL_SECS);

    state.db.with_conn(|conn| {
        let user_id: Option<String> = conn
            .query_row(
                "SELECT user_id FROM users WHERE email = ?1",
                [&email],
                |r| r.get(0),
            )
            .optional()?;

        let user_id = user_id.ok_or_else(|| ServerError::NotFound("user not found".to_string()))?;

        conn.execute(
            "INSERT INTO auth_challenges (challenge_id, user_id, nonce_hex, expires_at_utc, used)
             VALUES (?1, ?2, ?3, ?4, 0)",
            [
                &challenge_id,
                &user_id,
                &nonce_hex,
                &expires_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    })?;

    Ok(Json(ChallengeResponse {
        challenge_id,
        nonce_hex,
        expires_at_utc: expires_at,
    }))
}

/// Verifies a signed passkey challenge and MFA code, returning a bearer token.
///
/// # Errors
/// Returns [`ServerError::Unauthorized`] if challenge, signature, or MFA fails.
pub async fn verify_challenge(
    State(state): State<AppState>,
    Json(req): Json<VerifyRequest>,
) -> Result<Json<VerifyResponse>, ServerError> {
    let now = Utc::now();
    let mut raw_token = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut raw_token);
    let access_token = format!("oud_tok_{}", hex::encode(raw_token));
    let token_hash = hash_token(&access_token);
    let expires_at = now + Duration::seconds(SESSION_TTL_SECS);
    let mfa_hash = hash_token(req.mfa_code.trim());

    let user_id = state.db.with_conn(|conn| {
        let row: Option<(String, String, String, i64, String, String)> = conn
            .query_row(
                "SELECT c.user_id, c.nonce_hex, c.expires_at_utc, c.used, u.passkey_pubkey, u.mfa_secret_hint
                 FROM auth_challenges c
                 JOIN users u ON u.user_id = c.user_id
                 WHERE c.challenge_id = ?1",
                [&req.challenge_id],
                |r| {
                    Ok((
                        r.get(0)?,
                        r.get(1)?,
                        r.get(2)?,
                        r.get(3)?,
                        r.get(4)?,
                        r.get(5)?,
                    ))
                },
            )
            .optional()?;

        let (user_id, nonce_hex, challenge_exp_str, used, pubkey_str, expected_mfa_hash) =
            row.ok_or_else(|| ServerError::Unauthorized("unknown challenge".to_string()))?;

        if used != 0 {
            return Err(ServerError::Unauthorized(
                "challenge already used".to_string(),
            ));
        }

        let challenge_exp = DateTime::parse_from_rfc3339(&challenge_exp_str)
            .map(|dt| dt.with_timezone(&Utc))
            .map_err(|e| ServerError::Internal(format!("invalid challenge expiry: {e}")))?;

        if now >= challenge_exp {
            return Err(ServerError::Unauthorized("challenge expired".to_string()));
        }

        let pubkey = IdentityPublicKey::from_prefixed_hex(&pubkey_str)?;
        let message = format!("{}:{nonce_hex}", req.challenge_id);
        pubkey
            .verify(message.as_bytes(), &req.signature)
            .map_err(|_| ServerError::Unauthorized("invalid passkey signature".to_string()))?;

        if mfa_hash != expected_mfa_hash {
            return Err(ServerError::Unauthorized("invalid MFA code".to_string()));
        }

        conn.execute(
            "UPDATE auth_challenges SET used = 1 WHERE challenge_id = ?1",
            [&req.challenge_id],
        )?;

        conn.execute(
            "INSERT INTO auth_sessions (token_hash, user_id, mfa_verified, expires_at_utc)
             VALUES (?1, ?2, 1, ?3)",
            [&token_hash, &user_id, &expires_at.to_rfc3339()],
        )?;

        Ok(user_id)
    })?;

    Ok(Json(VerifyResponse {
        access_token,
        user_id,
        expires_at_utc: expires_at,
    }))
}

fn hash_token(token: &str) -> String {
    let digest = Sha256::digest(token.as_bytes());
    hex::encode(digest)
}
