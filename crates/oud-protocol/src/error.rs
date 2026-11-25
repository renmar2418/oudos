//! Error types for `oud-protocol` cryptographic, grant, state-machine, and
//! audit verification failures. All errors fail closed.

use thiserror::Error;

/// Fail-closed protocol errors returned by `oud-protocol`.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ProtocolError {
    /// Device ID did not contain exactly 9 ASCII digits.
    #[error("device ID must contain exactly 9 digits")]
    InvalidDeviceId,

    /// Malformed or wrong-length public key encoding.
    #[error("invalid public key encoding")]
    InvalidPublicKey,

    /// Malformed or wrong-length signature encoding.
    #[error("invalid signature encoding")]
    InvalidSignatureEncoding,

    /// Ed25519 signature verification failed.
    #[error("cryptographic signature verification failed")]
    SignatureVerificationFailed,

    /// Pinned TOFU host public key did not match the presented key.
    #[error("pinned host identity key changed; connection halted")]
    HostKeyChanged,

    /// Server public key has not been explicitly enrolled on the host.
    #[error("server is not enrolled on this host")]
    ServerNotEnrolled,

    /// Requester is not authenticated.
    #[error("requester is not authenticated")]
    UnauthenticatedRequester,

    /// Requester is locked out or rate-limited due to denials or SAS mismatches.
    #[error("requester is rate-limited or locked out")]
    RateLimitLockedOut,

    /// Handshake commit-before-reveal verification failed.
    #[error("SAS commitment verification failed")]
    CommitmentMismatch,

    /// Handshake HKDF key derivation failed.
    #[error("key derivation failed")]
    KeyDerivationFailed,

    /// 6-digit SAS code did not match.
    #[error("6-digit verification code mismatch")]
    SasMismatch,

    /// Grant or consent violates a `v0.1` Asks-First invariant.
    #[error("grant violates v0.1 Asks-First bounds: {0}")]
    V01PolicyViolation(&'static str),

    /// `SessionGrant` has expired or has an invalid time window.
    #[error("session grant expired or invalid time window")]
    GrantExpired,

    /// `SessionGrant` identity/session/server binding did not match active context.
    #[error("session grant binding mismatch")]
    GrantBindingMismatch,

    /// Mandatory 5-second cooling-off period for first-time requesters is still active.
    #[error("first-time requester 5-second cooling-off period has not elapsed")]
    CoolingOffActive,

    /// Consent request exceeded the 2-minute TTL.
    #[error("consent request expired after 2-minute TTL")]
    ConsentTtlExpired,

    /// 6-digit SAS code has not been verified yet.
    #[error("6-digit SAS verification has not been completed")]
    SasNotVerified,

    /// Operation is invalid in the current consent state.
    #[error("invalid state transition from {state} on event {event}")]
    InvalidTransition {
        /// Current state name.
        state: &'static str,
        /// Attempted event name.
        event: &'static str,
    },

    /// Canonical serialization failed.
    #[error("canonical serialization error: {0}")]
    SerializationError(String),

    /// Tamper-evident hash chain verification failed at `index`.
    #[error("audit chain verification failed at entry {index}: {reason}")]
    AuditChainBroken {
        /// Zero-based entry index where verification failed.
        index: usize,
        /// Human-readable cause.
        reason: &'static str,
    },
}
