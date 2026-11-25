# Oudos

> **Ask first, then help.**  
> Pure open-source (**AGPL-3.0**), consent-first, end-to-end encrypted remote desktop access.

**Oudos** (*Ancient Greek: οὐδός — "threshold / doorstep"*) is a free, self-hostable remote desktop platform built around one rule: **nothing starts without the device owner's informed yes, and every session is on the record.**

Unlike legacy remote-access tools that treat security warnings as afterthoughts or lock audit trails behind paid enterprise tiers, Oudos enforces permissions locally on the host device and ships the entire web console, policy engine, and tamper-evident audit log 100% free under **GNU AGPL-3.0**.

---

## Why Oudos?

| Capability | Typical Legacy / Freemium Tools | Oudos (Pure FOSS — AGPL-3.0) |
| :--- | :--- | :--- |
| **Default Access Model** | Fast connection first; manual hardening required | **Asks-First by default**, fails closed, zero hardening tax |
| **Permission Enforcement** | Often split across UI/server rules | **Host-enforced signed `SessionGrant`**; unauthorized frames dropped locally |
| **MITM & Identity Protection** | Self-reported IDs or server-generated PINs | **Pinned Ed25519 device keys** + **handshake-derived 6-digit SAS code** |
| **Self-Hosted Anti-Spoofing** | Any server can display arbitrary labels | **Pre-paired server trust** + raw pinned server domain display |
| **Policy Hierarchy** | Admin overrides owner | **Monotonic policies** (`Org -> Group -> Owner`); owners can only tighten rules |
| **Web Console & Audit Log** | Frequently paywalled in "Pro" server editions | **100% free** self-hosted console + hash-chained local & server CSV audit logs |
| **Anti-Scam Friction** | Bolt-on popups | **10-second plain-language consent card**, 5s cooling-off timer, view-only default, no blank screen in Asks-First mode |

> **Honest security note:** No remote-access tool can claim to be "100% scam-proof." Oudos is architected to disrupt rushed social-engineering scripts, give non-technical owners clear control, and preserve tamper-evident evidence of every session.

---

## Architecture Overview

Oudos is built as a single monorepo targeting a **$0 launch and self-hosting footprint**:

* **`crates/oud-protocol`** — Pure Rust cryptographic core: Ed25519 device identity keys, ephemeral X25519 handshake, commit-before-reveal 6-digit SAS derivation, host-signed `SessionGrant` capability tokens, deterministic consent state machine, and SHA-256 hash-chained audit events.
* **`crates/oud-policy`** — Pure Rust monotonic policy engine (`Org -> Group -> Device Owner`, strictest rule always wins).
* **`crates/oud-capture`** — Platform capture and input-injection traits (Windows Graphics Capture / DXGI in `v0.1`, Linux PipeWire/X11 in `v0.2`, macOS ScreenCaptureKit in `v0.3`).
* **`crates/oud-server`** — Self-hosted Axum + SQLite/PostgreSQL control plane, passkey/MFA auth, WebSocket SDP/ICE signaling, rate limiting, and audit log API.
* **`apps/host-desktop`** — Cross-platform **Rust + Tauri v2** PC host agent featuring the protected consent card, monochrome tray states, and persistent non-clickable `End session` banner.
* **`apps/web-pwa`** — **React + TypeScript + Tailwind CSS + Vite** Progressive Web App viewer and admin console (works on desktop, Android, and iOS browsers with zero app-store fees).
* **`apps/mobile-capacitor`** — **Capacitor** native mobile wrapper around `apps/web-pwa` (Android APK in `v0.2`, iOS in `v0.3`).
* **`deploy/docker-compose.yml`** — One-command self-hosting stack (`oud-server` + `web-pwa` + optional `coturn` / Cloudflare Realtime TURN fallback).

---

## Phased Roadmap (`v0.1` -> `v1.0`)

Every milestone is gated by automated protocol, security, and usability checks in `PLAN.md`:

* **`v0.1` (Active Target — $0 Cost):** Strictly **Asks-First** mode, **Windows 10/11 Host**, **React/TS PWA Viewer**, 1-command `docker compose` server, host-enforced grants, monotonic policies, and hash-chained CSV audit logs.
* **`v0.2` ($0 Cost):** **Linux Host** (Ubuntu 22.04/24.04 X11 + Wayland PipeWire), Windows `SYSTEM` Service & Secure Desktop isolation, **Unattended Mode** with step-up MFA, and **Android Capacitor APK**.
* **`v0.3` (Grant / Sponsor Funded):** **macOS Host** (ScreenCaptureKit + notarised `.dmg`), **iOS Capacitor App**, SSO/OIDC, SIEM audit streaming, and mid-session permission escalation.
* **`v1.0`:** Independent third-party security audit, Guardian Mode (co-approver for at-risk relatives), and Sensitive-Window Protection (auto-masking banking and password-manager windows).

---

## Local Development Setup ($0 Windows + WSL2 Toolchain)

### Prerequisites
1. **Windows 11** with **WSL2 Ubuntu 24.04** (`wsl --install -d Ubuntu-24.04`) for Docker and control-plane testing.
2. **Rust** (`winget install Rustlang.Rustup`) and **Tauri CLI v2** (`cargo install tauri-cli --version "^2.0.0"`).
3. **Node.js 22 LTS + pnpm** (`winget install OpenJS.NodeJS.LTS && corepack enable`).

### Quick Start (Once Issue `#1` Is Scaffolded)
```bash
# Run Rust protocol, policy, and server tests
cargo test --workspace

# Start the React PWA viewer locally
cd apps/web-pwa && pnpm install && pnpm dev

# Boot the full self-hosted stack (inside WSL2 Ubuntu 24.04)
docker compose -f deploy/docker-compose.yml up -d
```

---

## Documentation & Governance

* [`PLAN.md`](./PLAN.md) — Sequenced `v0.1` -> `v1.0` roadmap, milestones, and issue exit gates.
* [`docs/DECISIONS.md`](./docs/DECISIONS.md) — Complete Architecture & Strategy Decision Log (`ADR-001`).
* [`AGENTS.md`](./AGENTS.md) — Workspace engineering contract for AI-assisted development.
* [`.github/SECURITY.md`](./.github/SECURITY.md) — Vulnerability disclosure and 24-hour abuse reporting policy.

---

## License

Licensed under the **GNU Affero General Public License v3.0 (AGPL-3.0)**. See [`LICENSE`](./LICENSE) for the full text.