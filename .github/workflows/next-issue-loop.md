---
description: Autonomous gated loop that executes Oudos PLAN.md issues in order, verifies exit gates, commits, and advances until a hard stop or milestone completion.
---

# Oudos Automated Issue Loop

Execute issues sequentially from `PLAN.md` using `PROGRESS.md` as the single source of truth.

## Loop Algorithm (Repeat Per Issue)

### Step 1: Load State
1. Read `PROGRESS.md` to identify `current_milestone` and `current_issue`.
2. Read `PLAN.md`, `AGENTS.md`, `docs/DECISIONS.md`, `.agents/rules/security-and-protocol.md`, and `.agents/rules/ui-and-consent-ux.md`.
3. If `status == BLOCKED` or `status == AWAITING_HUMAN_REVIEW`, **STOP immediately** and report why.

### Step 2: Plan Current Issue
1. Update `PROGRESS.md` -> set `status: IN_PROGRESS`.
2. List the exact files to create/modify and the exact Exit Gate tests from `PLAN.md` for `current_issue`.
3. Verify the plan does not violate any invariant:
   - AGPL-3.0 license preserved.
   - `v0.1` stays strictly Asks-First (no Unattended Mode).
   - 6-digit SAS is derived from the key-agreement transcript, never issued by the server.
   - Host enforces signed `SessionGrant` locally and drops out-of-scope frames.
   - Policies merge monotonically (owners can only tighten, never loosen).
   - Audit logs store metadata only, never screen/input/clipboard/file content.
   - Pre-paired server trust only; raw enrolled server domain shown (no spoofable Verified badge).

### Step 3: Implement & Verify Exit Gate
1. Implement `current_issue` cleanly without placeholder stubs or skipped tests.
2. Run all applicable verification commands:
   - Rust workspace: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`
   - Web PWA (once created): `pnpm typecheck`, `pnpm build`, and accessibility/unit tests
   - Issue-specific Exit Gate from `PLAN.md`
3. **Retry Limit:** If verification fails, diagnose and fix up to **3 attempts** total.
   - Never delete, disable, or weaken an existing security/property test to make CI pass.
   - If still failing after 3 attempts, set `PROGRESS.md` -> `status: BLOCKED`, log the exact error, and **STOP**.

### Step 4: Record, Commit & Decide Whether to Auto-Advance
1. Mark `current_issue` complete (`[x]`) in `PLAN.md`.
2. Create a clean git commit: `feat(v0.1): complete issue #<N> - <short title>`.
3. Append a 3-line entry to `PROGRESS.md` (commit hash, tests run, files changed) and increment `current_issue` to `#<N+1>`.
4. Check **Hard-Stop Circuit Breakers** before starting `#<N+1>`:
   - **Security Review Gate:** Always STOP and ask for human review after completing **Issue `#2`** (Crypto/SAS/Consent State Machine), **Issue `#7`** (Windows Capture/WebRTC), and **Issue `#8`** (Protected Consent UI & Banner).
   - **External Action Gate:** Always STOP if the next issue requires human accounts, keys, hardware testing, or user testing (e.g., Issue `#12` 10-second human usability test, Cloudflare TURN keys, SignPath submission).
   - **Milestone Gate:** Always STOP at the end of a milestone (`#12` for `v0.1`, `#17` for `v0.2`, `#20` for `v0.3`) so the user can tag the release and verify GitHub Actions builds before opening the next milestone.
   - **Context Hygiene Gate:** After completing **3 consecutive issues** in one chat session, set `status: READY_FOR_NEXT_CHAT` and STOP with a concise handoff summary so the user can run `/next-issue-loop` in a fresh chat without context drift.
5. If **no** circuit breaker triggered, automatically proceed to Step 1 for `#<N+1>`.