#![forbid(unsafe_code)]
//! `oud-protocol`: Pure Rust cryptographic identity, handshake SAS, consent
//! state machine, `SessionGrant`, and tamper-evident audit primitives.

pub mod audit;
pub mod error;
pub mod grant;
pub mod handshake;
pub mod identity;
pub mod state_machine;

pub use audit::{
    verify_chain, AuditChain, AuditEvent, AuditEventKind, SignedAuditEntry, GENESIS_HASH,
};
pub use error::ProtocolError;
pub use grant::{FrameKind, GrantDuration, GrantPermissions, SessionGrant};
pub use handshake::{
    derive_sas_6digit, EphemeralKeyPair, HandshakeAccept, HandshakeOffer, HandshakeOutcome,
    InitiatorPending, ResponderPending, SasReveal, MAX_SAS_MISMATCHES,
};
pub use identity::{
    verify_host_key_tofu, DeviceId, IdentityKeyPair, IdentityPublicKey, IdentitySignature,
};
pub use state_machine::{
    ActiveSessionContext, BlockedReason, ConsentContext, ConsentEvent, ConsentMachine,
    ConsentState, DenialReason, RevocationReason, VerifyingContext, CONSENT_TTL_SECS,
    COOLING_OFF_SECS,
};

/// Protocol version identifier for the `v0.1` Asks-First handshake and grants.
pub const PROTOCOL_VERSION: &str = "oudos-v0.1";

#[cfg(test)]
mod tests {
    use super::PROTOCOL_VERSION;

    #[test]
    fn protocol_version_is_v0_1() {
        assert_eq!(PROTOCOL_VERSION, "oudos-v0.1");
    }
}
