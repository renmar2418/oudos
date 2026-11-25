//! Property tests for monotonic policy merging in `oud-policy`.

use oud_policy::{merge_policies, IdleTimeout, PolicyLayer, V01_MAX_GRANT_DURATION_SECS};
use proptest::prelude::*;

fn arb_opt_bool() -> impl Strategy<Value = Option<bool>> {
    prop_oneof![Just(None), Just(Some(false)), Just(Some(true))]
}

fn arb_opt_timeout() -> impl Strategy<Value = Option<IdleTimeout>> {
    prop_oneof![
        Just(None),
        Just(Some(IdleTimeout::Minutes15)),
        Just(Some(IdleTimeout::Minutes30)),
        Just(Some(IdleTimeout::Hours1)),
        Just(Some(IdleTimeout::Never)),
    ]
}

fn arb_policy_layer() -> impl Strategy<Value = PolicyLayer> {
    (
        arb_opt_bool(),
        arb_opt_bool(),
        arb_opt_bool(),
        arb_opt_bool(),
        arb_opt_bool(),
        arb_opt_bool(),
        arb_opt_timeout(),
        prop::option::of(1_u64..100_000_u64),
        prop::option::of(0_u64..500_000_000_u64),
    )
        .prop_map(
            |(
                ask_before_connection,
                require_matching_code,
                allow_unattended,
                allow_file_transfer,
                allow_mouse_keyboard,
                allow_clipboard_sync,
                idle_timeout,
                max_grant_duration_secs,
                max_file_size_bytes,
            )| PolicyLayer {
                ask_before_connection,
                require_matching_code,
                allow_unattended,
                allow_file_transfer,
                allow_mouse_keyboard,
                allow_clipboard_sync,
                idle_timeout,
                max_grant_duration_secs,
                max_file_size_bytes,
            },
        )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10_000))]

    #[test]
    fn owner_or_rogue_layer_never_relaxes_upstream_or_v01_invariants(
        org in arb_policy_layer(),
        group in arb_policy_layer(),
        owner in arb_policy_layer(),
    ) {
        let upstream = merge_policies(&org, &group, &PolicyLayer::default());
        let with_owner = merge_policies(&org, &group, &owner);

        // v0.1 Asks-First invariants are unbreakable.
        prop_assert!(with_owner.rules.ask_before_connection);
        prop_assert!(with_owner.rules.require_matching_code);
        prop_assert!(!with_owner.rules.allow_unattended);
        prop_assert!(with_owner.rules.max_grant_duration_secs <= V01_MAX_GRANT_DURATION_SECS);

        // Allow-flags can never flip from false -> true downstream.
        if !upstream.rules.allow_file_transfer {
            prop_assert!(!with_owner.rules.allow_file_transfer);
        }
        if !upstream.rules.allow_mouse_keyboard {
            prop_assert!(!with_owner.rules.allow_mouse_keyboard);
        }
        if !upstream.rules.allow_clipboard_sync {
            prop_assert!(!with_owner.rules.allow_clipboard_sync);
        }

        // Numeric and timeout caps can never increase downstream.
        prop_assert!(with_owner.rules.idle_timeout <= upstream.rules.idle_timeout);
        prop_assert!(with_owner.rules.max_grant_duration_secs <= upstream.rules.max_grant_duration_secs);
        prop_assert!(with_owner.rules.max_file_size_bytes <= upstream.rules.max_file_size_bytes);
    }
}
