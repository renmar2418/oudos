# Oudos Build Progress

- **current_milestone:** `v0.1`
- **current_issue:** `#6`
- **status:** `READY_FOR_NEXT_CHAT` *(Allowed: READY | IN_PROGRESS | AWAITING_HUMAN_REVIEW | READY_FOR_NEXT_CHAT | BLOCKED | MILESTONE_COMPLETE)*
- **issues_completed_this_session:** `3`
- **auto_advance:** `true`
- **hard_stop_after_issues:** `#2, #7, #8, #12, #17, #20, #22`

## Completed Log
- **Issue `#1` (`4bff486`):** Scaffolded Cargo workspace (`oud-protocol`, `oud-policy`, `oud-capture`, `oud-server`), pnpm workspace (`apps/web-pwa`, `apps/host-desktop`, `apps/mobile-capacitor`), `LICENSE` (AGPL-3.0), `README.md`, and `docs/*`. Verified `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` (4/4 unit tests), `pnpm typecheck`, and `pnpm --dir apps/web-pwa run build`.
- **Issue `#2` (`7bc203c` + hardening fix):** Implemented `oud-protocol` (`error.rs`, `identity.rs`, `handshake.rs`, `grant.rs`, `state_machine.rs`, `audit.rs`, `lib.rs`, `tests/state_machine_proptest.rs`) and hardened `commit_nonce` transcript binding, whole-second `SessionGrant` canonicalization + tamper tests, 3-entry single-byte audit tamper test, `GrantUpdatedImmediately` idle-timeout settling, and moved `next-issue-loop.md` to `.agents/workflows/`. Verified `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace` (13 unit tests + 8 integration/proptest tests with 10,000 cases).
- **Issue `#3` (`e82432d`):** Implemented `oud-policy` monotonic policy engine (`Organisation -> Group -> DeviceOwner` + `v0.1` Asks-First lock, 5 baseline rules, `IdleTimeout` ranking, `PolicyLockProvenance`, `EffectivePolicy::clamp_permissions`, `merge_policies`, `merge_policies_strict`). Verified `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace` (5 unit tests + 10,000-case `policy_proptest`).
- **Issue `#4` (`910dc20`):** Implemented `oud-server` Axum + SQLite control plane (`auth.rs`, `db.rs`, `rate_limit.rs`, `signaling.rs`, `audit.rs`, `error.rs`, `lib.rs`, `main.rs`) with passkey/MFA auth, pre-paired server pubkey enrollment, 9-digit device registry & presence, fail-closed WebSocket signaling, 3-strike lockout, monotonic policy API, and verifiable hash-chained CSV audit export. Verified `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace` (including 5 `server_integration` tests).
- **Issue `#5` (`8e552cc`):** Implemented `apps/web-pwa` React + TypeScript + Tailwind v4 + Vite PWA (`FacingBracketsIcon`, `DevicesScreen`, `RequestingScreen`, `ActiveSessionScreen`, `PoliciesScreen`, `AuditLogScreen`, `manifest.webmanifest`, `sw.js`). Verified `pnpm typecheck`, `pnpm --dir apps/web-pwa run build`, and `pnpm --dir apps/web-pwa run test` (9/9 Vitest + `axe-core` WCAG 2.2 AA tests across all 5 core screens).

## Active Blockers
- None
