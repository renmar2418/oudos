#![forbid(unsafe_code)]
//! `oud-policy`: Monotonic policy evaluation across Organisation, Group, and
//! Device Owner scopes where strictest rules always win.

/// Baseline policy scope hierarchy order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyScope {
    /// Organisation-wide baseline rules.
    Organisation,
    /// Device group rules.
    Group,
    /// Local device owner overrides (tightening only).
    DeviceOwner,
}

#[cfg(test)]
mod tests {
    use super::PolicyScope;

    #[test]
    fn policy_scopes_are_distinct() {
        assert_ne!(PolicyScope::Organisation, PolicyScope::DeviceOwner);
    }
}
