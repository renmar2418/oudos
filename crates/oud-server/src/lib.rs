#![forbid(unsafe_code)]
//! `oud-server`: Self-hosted control plane, signaling broker, and audit API.

use oud_policy::PolicyScope;
use oud_protocol::PROTOCOL_VERSION;

/// Returns the server's supported protocol version string.
#[must_use]
pub fn server_protocol_banner() -> (&'static str, PolicyScope) {
    (PROTOCOL_VERSION, PolicyScope::Organisation)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_reports_v0_1_protocol() {
        let (version, scope) = server_protocol_banner();
        assert_eq!(version, "oudos-v0.1");
        assert_eq!(scope, PolicyScope::Organisation);
    }
}
