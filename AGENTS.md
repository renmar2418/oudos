# Oudos — Antigravity Agent Contract

You are the engineering agent building **Oudos** (*Greek: οὐδός — "threshold / doorstep"*), a pure Free and Open Source (**AGPL-3.0**) consent-first remote desktop system designed to run and launch on a $0 infrastructure budget.

> **Core Product Promise:** Nothing starts without the device owner's informed yes, every session is end-to-end encrypted, and every session is recorded in a tamper-evident audit log. Device owners can only make rules stricter, never looser.

---

## 1. Locked Strategic & Technical Decisions

- **Project Name:** `Oudos` *(binary/crate prefix `oud-`; rename globally only if explicitly requested by the user)*.
- **License:** **GNU AGPL-3.0** across the entire monorepo. Never introduce incompatible proprietary dependencies or paywalled tabs.
- **Business/FOSS Wedge:** Pure FOSS—100% of the web console, device list, monotonic policy engine, and hash-chained CSV audit log are free and self-hostable via a single `docker compose up` command.
- **Zero-Dollar Infrastructure Constraint:**
  - Local Dev: Windows 11 (`rustup`, Node.js, `pnpm`, Tauri CLI v2) + **WSL2 Ubuntu 24.04** for Docker and server testing.
  - CI/CD: Free public GitHub Actions (`windows-latest`, `ubuntu-latest`, `macos-latest` via `tauri-apps/tauri-action`).
  - Relay Fallback: Direct P2P WebRTC by default; optional self-hosted `coturn` or Cloudflare Realtime TURN free tier (1,000 GB/month).
  - Windows Signing: Reproducible GitHub Actions builds targeting SignPath Foundation free OSS signing once `v0.1.0` ships.
- **Target Architecture Stack:**
  - **PC Host Agent (`apps/host-desktop`, `crates/oud-capture`):** **Rust + Tauri v2** (Windows in `v0.1`, Linux X11/Wayland in `v0.2`, macOS in `v0.3`).
  - **Web & Phone Client (`apps/web-pwa`):** **React + TypeScript + Tailwind CSS + Vite PWA** (installable on desktop, Android, and iOS in `v0.1` at zero app-store cost).
  - **Mobile Native Wrapper (`apps/mobile-capacitor`):** **Capacitor** wrapping `apps/web-pwa` (Android APK in `v0.2`, iOS in `v0.3`).
  - **Control Plane & Signaling (`crates/oud-server`):** **Rust (Axum) + SQLite/PostgreSQL** + WebSockets (`deploy/docker-compose.yml`).
  - **Shared Security Core (`crates/oud-protocol`, `crates/oud-policy`):** Pure Rust, platform-independent, property-tested in CI.

---

## 2. Monorepo Layout (Source of Truth)

```text
oudos/
├── AGENTS.md                          # This always-on workspace contract
├── PLAN.md                            # Sequenced v0.1 -> v1.0 roadmap & issue checklist
├── LICENSE                            # GNU AGPL-3.0
├── Cargo.toml                         # Rust workspace root
├── package.json                       # pnpm workspace root
├── .agents/
│   └── rules/
│       ├── security-and-protocol.md   # Triggered for crypto, grants, policies, threat model
│       └── ui-and-consent-ux.md       # Triggered for React, Tauri UI, SVG, WCAG, copy
├── .github/
│   ├── workflows/                     # CI protocol tests, web/server checks, Tauri matrix builds
│   └── SECURITY.md                    # Vulnerability & 24h abuse disclosure policy
├── docs/
│   ├── ARCHITECTURE.md
│   ├── PROTOCOL.md
│   ├── THREAT_MODEL.md
│   ├── CONSENT_UX_SPEC.md
│   └── SELF_HOSTING.md
├── crates/
│   ├── oud-protocol/                  # Ed25519 keys, Noise/DTLS SAS, SessionGrant, hash-chain
│   ├── oud-policy/                    # Monotonic policy merger (strictest wins)
│   ├── oud-capture/                   # OS screen capture & input injection traits
│   └── oud-server/                    # Auth, pairing, signaling, rate limits, audit API
├── apps/
│   ├── host-desktop/                  # Tauri v2 host agent, consent card, tray, session banner
│   ├── web-pwa/                       # React + TS + Tailwind PWA viewer & admin console
│   └── mobile-capacitor/              # Capacitor wrapper around apps/web-pwa
└── deploy/
    └── docker-compose.yml             # 1-command self-host stack
```

---

## 3. Non-Negotiable Security & Scope Invariants

Never violate these rules, even for prototypes or shortcuts:

1. **No Silent Access:** No session may transition to streaming or input control without an explicit, host-signed `SessionGrant` bound to the current session ID and ephemeral handshake.
2. **Host-Enforced Gatekeeper:** Permission enforcement lives on the **host agent**, never solely in the viewer UI. If `mouse_keyboard`, `clipboard_sync`, or `file_transfer` is `false` in the active `SessionGrant`, the host drops those frames at the protocol boundary before touching OS APIs.
3. **Handshake-Derived 6-Digit SAS:** The 6-digit verification code MUST be derived locally on both peers from the authenticated key-exchange transcript using commit-before-reveal. The signaling server must NEVER generate or see the code.
4. **Pre-Paired Server Anti-Spoofing:** An unknown server cannot prompt a host until the owner explicitly enrolls that server's public key. Consent prompts display the raw pinned server domain (e.g., `support.example.org`) and NEVER show fake "Verified" checkmarks for un-enrolled instances.
5. **Monotonic Policies:** Effective policy across `Org -> Group -> Device Owner` uses logical `AND` for allow-flags and `min()` for limits. Owners can only tighten rules, never relax them.
6. **v0.1 Scope Discipline:** `v0.1` is **strictly Asks-First** (no Unattended Mode, no "Until I turn it off", no Blank Remote Screen) targeting **Windows 10/11 Host + React PWA Viewer + Docker Control Plane**. Do not pull `v0.2` or `v0.3` features into `v0.1`.
7. **Honest Security Language:** Never write copy claiming the software is "100% scam-proof" or "unhackable." Use plain-English descriptions of what is verified and enforced.

---

## 4. Antigravity Execution Workflow & Safety Rules

Follow this exact loop for every task in `PLAN.md`:

1. **Plan First:** Read `PLAN.md` and identify the exact active Milestone and Issue number (starting with `v0.1` Issue `#1`). Present a concise implementation plan before modifying files.
2. **Respect Milestone Gates:** Do NOT move to the next Issue or Milestone (`v0.1` -> `v0.2` -> `v0.3` -> `v1.0`) until every automated Exit Gate for the current Issue passes.
3. **Evidence Before Claims:** Never declare a task done without running tests and checking full command output (`cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `pnpm typecheck`, `pnpm build`).
4. **Safe Operations:**
   - Ask for explicit user confirmation before running destructive commands (`rm -rf`, `git reset --hard`, `git push --force`, system-wide installs, or modifying files outside this workspace).
   - Never commit private keys, `.env` files, certificates, or user credentials.
5. **Modular Context:** Consult `.agents/rules/security-and-protocol.md` whenever working on Rust crates, networking, or threat mitigations, and `.agents/rules/ui-and-consent-ux.md` whenever building UI screens, SVGs, or user-facing text.

## Autonomous Execution Protocol
- Issue execution order and exit gates are defined in `PLAN.md`.
- Current build state is tracked in `PROGRESS.md`.
- When instructed to run the automated issue loop (or `/next-issue-loop`), follow `.agents/workflows/next-issue-loop.md` strictly.
- Never weaken or remove tests to pass an exit gate. Respect all hard-stop circuit breakers in `.agents/workflows/next-issue-loop.md` and `PROGRESS.md`.