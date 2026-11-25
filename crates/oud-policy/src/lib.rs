#![forbid(unsafe_code)]
//! `oud-policy`: Monotonic policy evaluation across Organisation, Group, and
//! Device Owner scopes where strictest rules always win.
//!
//! ## Monotonicity Rules
//! - **Allow flags** (`allow_unattended`, `allow_file_transfer`,
//!   `allow_mouse_keyboard`, `allow_clipboard_sync`):
//!   `effective = org && group && owner` (only `false` can tighten).
//! - **Requirement flags** (`ask_before_connection`, `require_matching_code`):
//!   `effective = org || group || owner` (only `true` can tighten).
//! - **Numeric & duration caps** (`idle_timeout`, `max_grant_duration_secs`,
//!   `max_file_size_bytes`): `effective = min(org, group, owner)`.
//! - **`v0.1` Asks-First Lock:** `ask_before_connection` is locked `true`,
//!   `require_matching_code` is locked `true`, and `allow_unattended` is
//!   locked `false` regardless of layer values.

use std::cmp::Ordering;

use oud_protocol::grant::{GrantDuration, GrantPermissions, THIS_SESSION_MAX_SECS};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Default maximum file transfer size cap (100 MiB) when enabled.
pub const DEFAULT_MAX_FILE_SIZE_BYTES: u64 = 100 * 1024 * 1024;

/// Default maximum grant duration cap in `v0.1` (`THIS_SESSION_MAX_SECS`, 4 hours).
#[allow(clippy::cast_sign_loss)]
pub const V01_MAX_GRANT_DURATION_SECS: u64 = THIS_SESSION_MAX_SECS as u64;

/// Baseline policy scope hierarchy order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyScope {
    /// Organisation-wide baseline rules.
    Organisation,
    /// Device group rules.
    Group,
    /// Local device owner overrides (tightening only).
    DeviceOwner,
    /// Hard-coded `v0.1` Asks-First invariant lock.
    V01ScopeLock,
}

/// Errors returned when validating or deserializing policy overrides.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum PolicyError {
    /// A lower-precedence layer attempted to relax a stricter upstream rule.
    #[error("policy relaxation rejected at {scope:?} for `{rule}`")]
    RelaxationRejected {
        /// Scope that attempted the relaxation.
        scope: PolicyScope,
        /// Rule name that was relaxed.
        rule: &'static str,
    },
    /// A policy cap was set to zero where a positive bound is required.
    #[error("invalid zero bound for `{rule}`")]
    InvalidZeroBound {
        /// Rule name with invalid zero bound.
        rule: &'static str,
    },
}

/// Allowed idle timeout options, ordered strictly from strictest (`Minutes15`)
/// to most permissive (`Never`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum IdleTimeout {
    /// 15 minutes (strictest).
    Minutes15,
    /// 30 minutes (default).
    #[default]
    Minutes30,
    /// 1 hour.
    Hours1,
    /// Never disconnect on idle (most permissive).
    Never,
}

impl IdleTimeout {
    /// Returns the timeout in seconds, or `None` for [`IdleTimeout::Never`].
    #[must_use]
    pub const fn as_secs(self) -> Option<u64> {
        match self {
            Self::Minutes15 => Some(15 * 60),
            Self::Minutes30 => Some(30 * 60),
            Self::Hours1 => Some(60 * 60),
            Self::Never => None,
        }
    }

    const fn rank(self) -> u8 {
        match self {
            Self::Minutes15 => 0,
            Self::Minutes30 => 1,
            Self::Hours1 => 2,
            Self::Never => 3,
        }
    }
}

impl Ord for IdleTimeout {
    fn cmp(&self, other: &Self) -> Ordering {
        self.rank().cmp(&other.rank())
    }
}

impl PartialOrd for IdleTimeout {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Complete policy rule set representing either an explicit baseline or an
/// evaluated effective policy.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyRuleSet {
    /// Rule 1: Host must explicitly approve every incoming connection.
    pub ask_before_connection: bool,
    /// Rule 2: Both peers must confirm matching 6-digit SAS code.
    pub require_matching_code: bool,
    /// Rule 3: Unattended access (locked `false` in `v0.1`).
    pub allow_unattended: bool,
    /// Rule 4: Chunked file transfer permission ceiling (defaults to `false`).
    pub allow_file_transfer: bool,
    /// Remote mouse & keyboard input ceiling (defaults to `true` as policy
    /// ceiling; still defaults to `false` per-session on the host consent card).
    pub allow_mouse_keyboard: bool,
    /// Directional clipboard sync ceiling (defaults to `true` as policy
    /// ceiling; still defaults to `false` per-session on the host consent card).
    pub allow_clipboard_sync: bool,
    /// Rule 5: Idle disconnect timeout.
    pub idle_timeout: IdleTimeout,
    /// Maximum grant duration in seconds (capped by `V01_MAX_GRANT_DURATION_SECS`).
    pub max_grant_duration_secs: u64,
    /// Maximum file transfer size in bytes.
    pub max_file_size_bytes: u64,
}

impl Default for PolicyRuleSet {
    fn default() -> Self {
        Self {
            ask_before_connection: true,
            require_matching_code: true,
            allow_unattended: false,
            allow_file_transfer: false,
            allow_mouse_keyboard: true,
            allow_clipboard_sync: true,
            idle_timeout: IdleTimeout::Minutes30,
            max_grant_duration_secs: V01_MAX_GRANT_DURATION_SECS,
            max_file_size_bytes: DEFAULT_MAX_FILE_SIZE_BYTES,
        }
    }
}

/// Optional per-layer overrides for a single policy scope (`Organisation`,
/// `Group`, or `DeviceOwner`).
///
/// Unspecified (`None`) fields inherit from the upstream scope.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct PolicyLayer {
    /// Optional override for `ask_before_connection` (can only tighten to `true`).
    pub ask_before_connection: Option<bool>,
    /// Optional override for `require_matching_code` (can only tighten to `true`).
    pub require_matching_code: Option<bool>,
    /// Optional override for `allow_unattended` (locked `false` in `v0.1`).
    pub allow_unattended: Option<bool>,
    /// Optional override for `allow_file_transfer` (can only tighten to `false`).
    pub allow_file_transfer: Option<bool>,
    /// Optional override for `allow_mouse_keyboard` (can only tighten to `false`).
    pub allow_mouse_keyboard: Option<bool>,
    /// Optional override for `allow_clipboard_sync` (can only tighten to `false`).
    pub allow_clipboard_sync: Option<bool>,
    /// Optional override for `idle_timeout` (can only tighten via `min()`).
    pub idle_timeout: Option<IdleTimeout>,
    /// Optional override for `max_grant_duration_secs` (can only tighten via `min()`).
    pub max_grant_duration_secs: Option<u64>,
    /// Optional override for `max_file_size_bytes` (can only tighten via `min()`).
    pub max_file_size_bytes: Option<u64>,
}

/// Records which scope locked each rule so UIs can render admin lock icons.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyLockProvenance {
    /// Scope locking `ask_before_connection`.
    pub ask_before_connection_locked_by: Option<PolicyScope>,
    /// Scope locking `require_matching_code`.
    pub require_matching_code_locked_by: Option<PolicyScope>,
    /// Scope locking `allow_unattended`.
    pub allow_unattended_locked_by: Option<PolicyScope>,
    /// Scope locking `allow_file_transfer` to `false`.
    pub allow_file_transfer_locked_by: Option<PolicyScope>,
    /// Scope locking `allow_mouse_keyboard` to `false`.
    pub allow_mouse_keyboard_locked_by: Option<PolicyScope>,
    /// Scope locking `allow_clipboard_sync` to `false`.
    pub allow_clipboard_sync_locked_by: Option<PolicyScope>,
    /// Strictest scope determining `idle_timeout`.
    pub idle_timeout_set_by: Option<PolicyScope>,
}

/// Resulting effective policy and lock provenance after monotonic merging.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EffectivePolicy {
    /// Monotonically merged rules (strictest wins + `v0.1` locks applied).
    pub rules: PolicyRuleSet,
    /// Provenance indicating which scope locked or constrained each rule.
    pub provenance: PolicyLockProvenance,
}

impl EffectivePolicy {
    /// Clamps requested [`GrantPermissions`] against this effective policy.
    ///
    /// Even if a host or viewer requests a disabled capability, the returned
    /// permissions are monotonically restricted to what policy permits.
    #[must_use]
    pub const fn clamp_permissions(&self, requested: &GrantPermissions) -> GrantPermissions {
        GrantPermissions {
            view_screen: requested.view_screen,
            mouse_keyboard: requested.mouse_keyboard && self.rules.allow_mouse_keyboard,
            clipboard_sync: requested.clipboard_sync && self.rules.allow_clipboard_sync,
            file_transfer: requested.file_transfer && self.rules.allow_file_transfer,
            blank_screen: false, // Locked OFF in v0.1 Asks-First mode.
        }
    }

    /// Returns `true` if the requested [`GrantDuration`] fits within
    /// `max_grant_duration_secs`.
    #[must_use]
    pub const fn is_duration_allowed(&self, duration: GrantDuration) -> bool {
        let secs = duration.max_duration().num_seconds();
        if secs <= 0 {
            return false;
        }
        #[allow(clippy::cast_sign_loss)]
        let secs_u64 = secs as u64;
        secs_u64 <= self.rules.max_grant_duration_secs
    }
}

/// Merges `Organisation -> Group -> DeviceOwner` layers monotonically so
/// strictest rules always win, then enforces `v0.1` Asks-First locks.
///
/// Any attempt by a downstream or rogue layer to relax an upstream rule is
/// ignored (the stricter value wins). If you want to reject relaxation
/// attempts with an explicit error instead, call [`merge_policies_strict`].
#[must_use]
pub fn merge_policies(
    org: &PolicyLayer,
    group: &PolicyLayer,
    owner: &PolicyLayer,
) -> EffectivePolicy {
    let mut rules = PolicyRuleSet::default();
    if let Some(true) = org.allow_file_transfer {
        rules.allow_file_transfer = true;
    }
    if let Some(timeout) = org.idle_timeout {
        rules.idle_timeout = timeout;
    }
    if let Some(secs) = org.max_grant_duration_secs {
        rules.max_grant_duration_secs = secs.min(V01_MAX_GRANT_DURATION_SECS);
    }
    if let Some(bytes) = org.max_file_size_bytes {
        rules.max_file_size_bytes = bytes;
    }

    let layers = [
        (PolicyScope::Organisation, org),
        (PolicyScope::Group, group),
        (PolicyScope::DeviceOwner, owner),
    ];

    let mut ask_locked = Some(PolicyScope::V01ScopeLock);
    let mut code_locked = Some(PolicyScope::V01ScopeLock);
    let mut unattended_locked = Some(PolicyScope::V01ScopeLock);
    let mut file_locked: Option<PolicyScope> = if rules.allow_file_transfer {
        None
    } else {
        Some(PolicyScope::Organisation)
    };
    let mut input_locked: Option<PolicyScope> = None;
    let mut clipboard_locked: Option<PolicyScope> = None;
    let mut idle_set_by: Option<PolicyScope> = org.idle_timeout.map(|_| PolicyScope::Organisation);

    for (scope, layer) in layers {
        apply_layer_and_provenance(
            scope,
            layer,
            &mut rules,
            &mut ask_locked,
            &mut code_locked,
            &mut unattended_locked,
            &mut file_locked,
            &mut input_locked,
            &mut clipboard_locked,
            &mut idle_set_by,
        );
    }

    // Re-assert v0.1 Asks-First invariants unconditionally.
    rules.ask_before_connection = true;
    rules.require_matching_code = true;
    rules.allow_unattended = false;
    rules.max_grant_duration_secs = rules
        .max_grant_duration_secs
        .min(V01_MAX_GRANT_DURATION_SECS);

    EffectivePolicy {
        rules,
        provenance: PolicyLockProvenance {
            ask_before_connection_locked_by: ask_locked,
            require_matching_code_locked_by: code_locked,
            allow_unattended_locked_by: unattended_locked,
            allow_file_transfer_locked_by: file_locked,
            allow_mouse_keyboard_locked_by: input_locked,
            allow_clipboard_sync_locked_by: clipboard_locked,
            idle_timeout_set_by: idle_set_by,
        },
    }
}

#[allow(clippy::too_many_arguments)]
fn apply_layer_and_provenance(
    scope: PolicyScope,
    layer: &PolicyLayer,
    rules: &mut PolicyRuleSet,
    ask_locked: &mut Option<PolicyScope>,
    code_locked: &mut Option<PolicyScope>,
    unattended_locked: &mut Option<PolicyScope>,
    file_locked: &mut Option<PolicyScope>,
    input_locked: &mut Option<PolicyScope>,
    clipboard_locked: &mut Option<PolicyScope>,
    idle_set_by: &mut Option<PolicyScope>,
) {
    if let Some(true) = layer.ask_before_connection {
        rules.ask_before_connection = true;
        if *ask_locked == Some(PolicyScope::V01ScopeLock) && scope != PolicyScope::DeviceOwner {
            *ask_locked = Some(scope);
        }
    }
    if let Some(true) = layer.require_matching_code {
        rules.require_matching_code = true;
        if *code_locked == Some(PolicyScope::V01ScopeLock) && scope != PolicyScope::DeviceOwner {
            *code_locked = Some(scope);
        }
    }
    if let Some(false) = layer.allow_unattended {
        rules.allow_unattended = false;
        if *unattended_locked == Some(PolicyScope::V01ScopeLock)
            && scope != PolicyScope::DeviceOwner
        {
            *unattended_locked = Some(scope);
        }
    }
    if let Some(false) = layer.allow_file_transfer {
        rules.allow_file_transfer = false;
        if file_locked.is_none() {
            *file_locked = Some(scope);
        }
    }
    if let Some(false) = layer.allow_mouse_keyboard {
        rules.allow_mouse_keyboard = false;
        if input_locked.is_none() {
            *input_locked = Some(scope);
        }
    }
    if let Some(false) = layer.allow_clipboard_sync {
        rules.allow_clipboard_sync = false;
        if clipboard_locked.is_none() {
            *clipboard_locked = Some(scope);
        }
    }
    if scope != PolicyScope::Organisation {
        if let Some(timeout) = layer.idle_timeout {
            if timeout < rules.idle_timeout {
                rules.idle_timeout = timeout;
                *idle_set_by = Some(scope);
            }
        }
        if let Some(secs) = layer.max_grant_duration_secs {
            rules.max_grant_duration_secs = rules.max_grant_duration_secs.min(secs);
        }
        if let Some(bytes) = layer.max_file_size_bytes {
            rules.max_file_size_bytes = rules.max_file_size_bytes.min(bytes);
        }
    }
}

/// Validates that `child` does not attempt to relax any rule already tightened
/// by `upstream`, then merges them monotonically.
///
/// # Errors
/// Returns [`PolicyError::RelaxationRejected`] if `child` explicitly sets a
/// looser value than `upstream` (or violates `v0.1` Asks-First locks), or
/// [`PolicyError::InvalidZeroBound`] if `max_grant_duration_secs` is `0`.
pub fn merge_policies_strict(
    org: &PolicyLayer,
    group: &PolicyLayer,
    owner: &PolicyLayer,
) -> Result<EffectivePolicy, PolicyError> {
    validate_v01_invariants(PolicyScope::Organisation, org)?;
    let mut baseline = PolicyRuleSet {
        allow_file_transfer: org.allow_file_transfer.unwrap_or(false),
        allow_mouse_keyboard: org.allow_mouse_keyboard.unwrap_or(true),
        allow_clipboard_sync: org.allow_clipboard_sync.unwrap_or(true),
        idle_timeout: org.idle_timeout.unwrap_or(IdleTimeout::Minutes30),
        max_grant_duration_secs: org
            .max_grant_duration_secs
            .map_or(V01_MAX_GRANT_DURATION_SECS, |s| {
                s.min(V01_MAX_GRANT_DURATION_SECS)
            }),
        max_file_size_bytes: org
            .max_file_size_bytes
            .unwrap_or(DEFAULT_MAX_FILE_SIZE_BYTES),
        ..PolicyRuleSet::default()
    };

    for (scope, layer) in [
        (PolicyScope::Group, group),
        (PolicyScope::DeviceOwner, owner),
    ] {
        validate_v01_invariants(scope, layer)?;
        validate_layer_against_baseline(scope, layer, &baseline)?;
        baseline = apply_layer_monotonic(&baseline, layer);
    }

    Ok(merge_policies(org, group, owner))
}

fn validate_v01_invariants(scope: PolicyScope, layer: &PolicyLayer) -> Result<(), PolicyError> {
    if layer.ask_before_connection == Some(false) {
        return Err(PolicyError::RelaxationRejected {
            scope,
            rule: "ask_before_connection",
        });
    }
    if layer.require_matching_code == Some(false) {
        return Err(PolicyError::RelaxationRejected {
            scope,
            rule: "require_matching_code",
        });
    }
    if layer.allow_unattended == Some(true) {
        return Err(PolicyError::RelaxationRejected {
            scope,
            rule: "allow_unattended",
        });
    }
    if layer.max_grant_duration_secs == Some(0) {
        return Err(PolicyError::InvalidZeroBound {
            rule: "max_grant_duration_secs",
        });
    }
    Ok(())
}

fn validate_layer_against_baseline(
    scope: PolicyScope,
    layer: &PolicyLayer,
    baseline: &PolicyRuleSet,
) -> Result<(), PolicyError> {
    if layer.ask_before_connection == Some(false) {
        return Err(PolicyError::RelaxationRejected {
            scope,
            rule: "ask_before_connection",
        });
    }
    if layer.require_matching_code == Some(false) {
        return Err(PolicyError::RelaxationRejected {
            scope,
            rule: "require_matching_code",
        });
    }
    if layer.allow_unattended == Some(true) {
        return Err(PolicyError::RelaxationRejected {
            scope,
            rule: "allow_unattended",
        });
    }
    if layer.allow_file_transfer == Some(true) && !baseline.allow_file_transfer {
        return Err(PolicyError::RelaxationRejected {
            scope,
            rule: "allow_file_transfer",
        });
    }
    if layer.allow_mouse_keyboard == Some(true) && !baseline.allow_mouse_keyboard {
        return Err(PolicyError::RelaxationRejected {
            scope,
            rule: "allow_mouse_keyboard",
        });
    }
    if layer.allow_clipboard_sync == Some(true) && !baseline.allow_clipboard_sync {
        return Err(PolicyError::RelaxationRejected {
            scope,
            rule: "allow_clipboard_sync",
        });
    }
    if let Some(timeout) = layer.idle_timeout {
        if timeout > baseline.idle_timeout {
            return Err(PolicyError::RelaxationRejected {
                scope,
                rule: "idle_timeout",
            });
        }
    }
    if let Some(secs) = layer.max_grant_duration_secs {
        if secs == 0 {
            return Err(PolicyError::InvalidZeroBound {
                rule: "max_grant_duration_secs",
            });
        }
        if secs > baseline.max_grant_duration_secs {
            return Err(PolicyError::RelaxationRejected {
                scope,
                rule: "max_grant_duration_secs",
            });
        }
    }
    if let Some(bytes) = layer.max_file_size_bytes {
        if bytes > baseline.max_file_size_bytes {
            return Err(PolicyError::RelaxationRejected {
                scope,
                rule: "max_file_size_bytes",
            });
        }
    }
    Ok(())
}

fn apply_layer_monotonic(baseline: &PolicyRuleSet, layer: &PolicyLayer) -> PolicyRuleSet {
    PolicyRuleSet {
        ask_before_connection: baseline.ask_before_connection
            || layer.ask_before_connection.unwrap_or(false),
        require_matching_code: baseline.require_matching_code
            || layer.require_matching_code.unwrap_or(false),
        allow_unattended: false,
        allow_file_transfer: baseline.allow_file_transfer
            && layer.allow_file_transfer.unwrap_or(true),
        allow_mouse_keyboard: baseline.allow_mouse_keyboard
            && layer.allow_mouse_keyboard.unwrap_or(true),
        allow_clipboard_sync: baseline.allow_clipboard_sync
            && layer.allow_clipboard_sync.unwrap_or(true),
        idle_timeout: layer
            .idle_timeout
            .map_or(baseline.idle_timeout, |t| baseline.idle_timeout.min(t)),
        max_grant_duration_secs: layer
            .max_grant_duration_secs
            .map_or(baseline.max_grant_duration_secs, |s| {
                baseline.max_grant_duration_secs.min(s)
            }),
        max_file_size_bytes: layer
            .max_file_size_bytes
            .map_or(baseline.max_file_size_bytes, |b| {
                baseline.max_file_size_bytes.min(b)
            }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_policy_enforces_v01_asks_first_and_file_transfer_off() {
        let effective = merge_policies(
            &PolicyLayer::default(),
            &PolicyLayer::default(),
            &PolicyLayer::default(),
        );
        assert!(effective.rules.ask_before_connection);
        assert!(effective.rules.require_matching_code);
        assert!(!effective.rules.allow_unattended);
        assert!(!effective.rules.allow_file_transfer);
        assert_eq!(effective.rules.idle_timeout, IdleTimeout::Minutes30);
    }

    #[test]
    fn rogue_owner_cannot_relax_org_or_v01_rules() {
        let org = PolicyLayer {
            ask_before_connection: Some(true),
            require_matching_code: Some(true),
            allow_unattended: Some(false),
            allow_file_transfer: Some(false),
            allow_mouse_keyboard: Some(false),
            allow_clipboard_sync: Some(false),
            idle_timeout: Some(IdleTimeout::Minutes15),
            max_grant_duration_secs: Some(1800),
            max_file_size_bytes: Some(1024),
        };
        let group = PolicyLayer::default();
        let rogue_owner = PolicyLayer {
            ask_before_connection: Some(false),
            require_matching_code: Some(false),
            allow_unattended: Some(true),
            allow_file_transfer: Some(true),
            allow_mouse_keyboard: Some(true),
            allow_clipboard_sync: Some(true),
            idle_timeout: Some(IdleTimeout::Never),
            max_grant_duration_secs: Some(86_400),
            max_file_size_bytes: Some(u64::MAX),
        };

        let merged = merge_policies(&org, &group, &rogue_owner);
        assert!(merged.rules.ask_before_connection);
        assert!(merged.rules.require_matching_code);
        assert!(!merged.rules.allow_unattended);
        assert!(!merged.rules.allow_file_transfer);
        assert!(!merged.rules.allow_mouse_keyboard);
        assert!(!merged.rules.allow_clipboard_sync);
        assert_eq!(merged.rules.idle_timeout, IdleTimeout::Minutes15);
        assert_eq!(merged.rules.max_grant_duration_secs, 1800);
        assert_eq!(merged.rules.max_file_size_bytes, 1024);

        let strict_err = merge_policies_strict(&org, &group, &rogue_owner).unwrap_err();
        assert!(matches!(
            strict_err,
            PolicyError::RelaxationRejected {
                scope: PolicyScope::DeviceOwner,
                ..
            }
        ));
    }

    #[test]
    fn owner_can_tighten_beyond_org_and_group() {
        let org = PolicyLayer {
            idle_timeout: Some(IdleTimeout::Hours1),
            max_grant_duration_secs: Some(3600),
            ..PolicyLayer::default()
        };
        let group = PolicyLayer {
            allow_clipboard_sync: Some(false),
            ..PolicyLayer::default()
        };
        let owner = PolicyLayer {
            allow_mouse_keyboard: Some(false),
            idle_timeout: Some(IdleTimeout::Minutes15),
            max_grant_duration_secs: Some(900),
            ..PolicyLayer::default()
        };

        let merged = merge_policies_strict(&org, &group, &owner).expect("tightening succeeds");
        assert!(!merged.rules.allow_clipboard_sync);
        assert!(!merged.rules.allow_mouse_keyboard);
        assert_eq!(merged.rules.idle_timeout, IdleTimeout::Minutes15);
        assert_eq!(merged.rules.max_grant_duration_secs, 900);
        assert_eq!(
            merged.provenance.allow_clipboard_sync_locked_by,
            Some(PolicyScope::Group)
        );
        assert_eq!(
            merged.provenance.allow_mouse_keyboard_locked_by,
            Some(PolicyScope::DeviceOwner)
        );
    }

    #[test]
    fn clamp_permissions_respects_policy_ceilings_and_v01_blank_screen_lock() {
        let org = PolicyLayer {
            allow_mouse_keyboard: Some(false),
            ..PolicyLayer::default()
        };
        let effective = merge_policies(&org, &PolicyLayer::default(), &PolicyLayer::default());
        let requested = GrantPermissions {
            view_screen: true,
            mouse_keyboard: true,
            clipboard_sync: true,
            file_transfer: true,
            blank_screen: true,
        };
        let clamped = effective.clamp_permissions(&requested);
        assert!(clamped.view_screen);
        assert!(!clamped.mouse_keyboard);
        assert!(clamped.clipboard_sync);
        assert!(!clamped.file_transfer);
        assert!(!clamped.blank_screen);
    }

    #[test]
    fn duration_check_enforces_max_grant_duration_secs() {
        let org = PolicyLayer {
            max_grant_duration_secs: Some(1800),
            ..PolicyLayer::default()
        };
        let effective = merge_policies(&org, &PolicyLayer::default(), &PolicyLayer::default());
        assert!(!effective.is_duration_allowed(GrantDuration::OneHour));
        assert!(!effective.is_duration_allowed(GrantDuration::ThisSession));
    }
}
