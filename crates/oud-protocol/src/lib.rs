#![forbid(unsafe_code)]
//! `oud-protocol`: Pure Rust cryptographic identity, handshake SAS, consent
//! state machine, `SessionGrant`, and tamper-evident audit primitives.

/// Protocol version identifier for the v0.1 Asks-First handshake and grants.
pub const PROTOCOL_VERSION: &str = "oudos-v0.1";

#[cfg(test)]
mod tests {
    use super::PROTOCOL_VERSION;

    #[test]
    fn protocol_version_is_v0_1() {
        assert_eq!(PROTOCOL_VERSION, "oudos-v0.1");
    }
}
