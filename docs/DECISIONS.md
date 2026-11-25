# Oudos — Locked Decision Log (`ADR-001`)

* **Status:** Accepted & Locked
* **License:** GNU AGPL-3.0 (`AGPL-3.0-only`) across the entire monorepo
* **Source Reference:** See also [DECISION LOG.md](../docs%20-%20optional/DECISION%20LOG.md)

## 1. Identity & Branding
- **Project Name:** `Oudos` (`oud-` crate/binary prefix).
- **Visual System:** Facing Brackets with Center Dot (Idle / In Session / Alert tray silhouettes), Primary Teal `#0F7B6C`, Mint Accent `#5FD9BF` (dark chrome `#0E1214` only), Crimson `#B42318`, Warning Amber `#B54708`.
- **Typography:** `Space Grotesk` (headings), `Inter` (UI body, min `14px` on consent cards), `JetBrains Mono` (device IDs, 6-digit SAS, latency, UTC timestamps).

## 2. Pure FOSS & $0 Launch Constraints
- Zero paywalled tabs or crippled Community Edition; full web console, monotonic policy engine, and hash-chained CSV audit logs self-host via `docker compose up`.
- Dev/CI on Windows 11 + WSL2 Ubuntu 24.04 and free public GitHub Actions runners (`tauri-apps/tauri-action`).
- Direct P2P WebRTC default with optional self-hosted `coturn` or Cloudflare Realtime TURN free tier; SignPath Foundation OSS Windows signing post-`v0.1.0`.

## 3. Core Architecture Stack
- **Host Agent:** Rust + Tauri v2 (`apps/host-desktop`, `crates/oud-capture`).
- **Web & Phone Viewer:** React + TypeScript + Tailwind CSS + Vite PWA (`apps/web-pwa`).
- **Mobile Wrapper:** Capacitor (`apps/mobile-capacitor`) in `v0.2`/`v0.3`.
- **Control Plane & Signaling:** Rust (Axum) + SQLite/PostgreSQL + WebSockets (`crates/oud-server`).
- **Shared Core:** `crates/oud-protocol` & `crates/oud-policy`.

## 4. Non-Negotiable Security & Scope Locks
- **Pre-Paired Server Anti-Spoofing:** Hosts enroll server public keys explicitly and render the raw pinned server domain (or amber `Personal / Unverified`), never spoofable "Verified" badges.
- **v0.1 Scope Lock:** Strictly Asks-First (`This session` or `1 hour`); no Unattended Mode, no `"Until I turn it off"`, no Blank Remote Screen. Windows 10/11 host + React PWA + Docker control plane.
- **Host-Enforced Gatekeeper:** Host signs `SessionGrant` and drops unauthorized input/clipboard/file frames before OS APIs.
- **Protected Consent UI:** 5-second anti-scam cooling-off timer for first-time requesters, `LLMHF_INJECTED` rejection, and remote cursor rect blocking over consent/banner windows.
