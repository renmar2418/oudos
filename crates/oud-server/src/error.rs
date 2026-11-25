//! HTTP and WebSocket error types for `oud-server`.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;
use thiserror::Error;

/// Unified error type for `oud-server` handlers and storage.
#[derive(Debug, Error)]
pub enum ServerError {
    /// Client sent invalid or malformed input.
    #[error("bad request: {0}")]
    BadRequest(String),
    /// Missing or invalid authentication / MFA.
    #[error("unauthorized: {0}")]
    Unauthorized(String),
    /// Authenticated caller lacks permission or policy rejected relaxation.
    #[error("forbidden: {0}")]
    Forbidden(String),
    /// Requested resource was not found.
    #[error("not found: {0}")]
    NotFound(String),
    /// Resource already exists or sequence/hash conflict.
    #[error("conflict: {0}")]
    Conflict(String),
    /// Support link or ephemeral resource has expired.
    #[error("gone: {0}")]
    Gone(String),
    /// Requester is locked out or rate-limited.
    #[error("rate limited: {0}")]
    RateLimited(String),
    /// Internal database or cryptographic error.
    #[error("internal error: {0}")]
    Internal(String),
}

#[derive(Serialize)]
struct ErrorBody<'a> {
    code: &'a str,
    error: String,
}

impl IntoResponse for ServerError {
    fn into_response(self) -> Response {
        let (status, code) = match &self {
            Self::BadRequest(_) => (StatusCode::BAD_REQUEST, "BAD_REQUEST"),
            Self::Unauthorized(_) => (StatusCode::UNAUTHORIZED, "UNAUTHORIZED"),
            Self::Forbidden(_) => (StatusCode::FORBIDDEN, "FORBIDDEN"),
            Self::NotFound(_) => (StatusCode::NOT_FOUND, "NOT_FOUND"),
            Self::Conflict(_) => (StatusCode::CONFLICT, "CONFLICT"),
            Self::Gone(_) => (StatusCode::GONE, "GONE"),
            Self::RateLimited(_) => (StatusCode::TOO_MANY_REQUESTS, "RATE_LIMITED"),
            Self::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR"),
        };

        let body = Json(ErrorBody {
            code,
            error: self.to_string(),
        });
        (status, body).into_response()
    }
}

impl From<rusqlite::Error> for ServerError {
    fn from(err: rusqlite::Error) -> Self {
        Self::Internal(err.to_string())
    }
}

impl From<oud_protocol::ProtocolError> for ServerError {
    fn from(err: oud_protocol::ProtocolError) -> Self {
        Self::BadRequest(err.to_string())
    }
}

impl From<oud_policy::PolicyError> for ServerError {
    fn from(err: oud_policy::PolicyError) -> Self {
        Self::Forbidden(err.to_string())
    }
}
