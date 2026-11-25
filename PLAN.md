# Oudos — Sequenced Implementation Plan (`v0.1` -> `v1.0`)

## How to Start in Google Antigravity (Copy & Paste This First Prompt)

Once you place `AGENTS.md`, `.agents/rules/*.md`, and `PLAN.md` into your `oudos/` workspace folder, open the folder in **Google Antigravity** (keep the Agent in **Planning mode** to review each step) and send:

> *"Read `AGENTS.md` and `PLAN.md`. Start Milestone `v0.1`, Issue `#1` (Repo & Governance Setup). Propose the exact files and commands first, wait for my approval before running terminal commands, and verify all Exit Gates for Issue `#1` before moving to Issue `#2`."*

---

## Day-1 Local Setup Commands (Windows 11 + WSL2 — $0 Cost)

Run once before handing off to Antigravity:

```powershell
# 1. Install WSL2 Ubuntu 24.04 for Docker & control-plane testing (Admin PowerShell)
wsl --install -d Ubuntu-24.04

# 2. Install Windows dev toolchain
winget install Rustlang.Rustup OpenJS.NodeJS.LTS
corepack enable
cargo install tauri-cli --version "^2.0.0"

# 3. Create workspace folder
mkdir oudos && cd oudos
git init -b main
```

---

## Milestone `v0.1` — Windows Host + React PWA + Self-Hosted Core (Weeks 1–12 | $0 Cost)

**Scope Lock:** Strictly **Asks-First** (no Unattended Mode, no "Until I turn it off", no Blank Remote Screen). Windows 10/11 Host + React/TypeScript PWA Viewer + 1-command `docker compose` server.

- [x] **Issue `#1`: Repo, License & Governance Setup**
  - Create Cargo workspace (`crates/oud-protocol`, `oud-policy`, `oud-capture`, `oud-server`), pnpm workspace (`apps/web-pwa`, `apps/host-desktop`, `apps/mobile-capacitor`), `LICENSE` (AGPL-3.0), `.github/SECURITY.md`, `.github/workflows/ci-protocol-security.yml`, and docs (`ARCHITECTURE.md`, `PROTOCOL.md`, `THREAT_MODEL.md`, `CONSENT_UX_SPEC.md`, `SELF_HOSTING.md`).
  - **Exit Gate:** `cargo test --workspace` and `pnpm typecheck` run green in CI.

- [x] **Issue `#2`: `oud-protocol` — Identity Keys, SAS & Consent State Machine**
  - Implement Ed25519 identity keys, X25519 ephemeral handshake, commit-before-reveal 6-digit SAS derivation, signed `SessionGrant` structs, and SHA-256 hash-chained audit events.
  - **Exit Gate:** `proptest` runs 10,000+ state-machine sequences proving zero paths reach `GRANT_SIGNED_ACTIVE` without a valid host signature, and every negative transition (deny, 2m timeout, mismatch, key change, revoke) fails closed.

- [x] **Issue `#3`: `oud-policy` — Monotonic Policy Engine**
  - Implement `Org -> Group -> Owner` strictest-wins merger across the 5 baseline rules (ask before connection, require matching code, unattended locked OFF in `v0.1`, file transfer off by default, idle timeout).
  - **Exit Gate:** Unit tests prove an owner or rogue config can never relax a stricter upstream rule.

- [x] **Issue `#4`: `oud-server` — Control Plane, Signaling & Audit API**
  - Build Axum + SQLite backend: passkey/MFA auth, pre-paired server public-key enrollment, 9-digit device registry, live presence (`Online`/`In session`/`Offline`), WebSocket SDP/ICE signaling, per-requester/device rate limiting, and append-only audit log with CSV export.
  - **Exit Gate:** Integration tests verify enrollment, rate-limit lockout, fail-closed refusal when signaling drops, and verifiable hash-chained CSV export.

- [x] **Issue `#5`: `web-pwa` — Design System & Core 5 Screens**
  - Build React + TS + Tailwind + Vite PWA implementing Devices, Requesting/Waiting (6-digit SAS + 2-min countdown), Active Session dark viewer (`#0E1214`, quality selector, latency, `Send Shortcuts`), Policies (with admin lock icons), and Audit Log (filters + CSV download).
  - **Exit Gate:** Lighthouse PWA check passes; `axe-core` reports zero critical WCAG 2.2 AA violations.

- [ ] **Issue `#6`: `web-pwa` — Missing MVP Flows & Edge States**
  - Implement Add Device pairing wizard, Sessions tab, `Screen unavailable` remediation card, Owner Denied, Request Expired, Code Mismatch, blocking Device Key Changed alert, Host Offline mid-session, and Session Ended summary.
  - **Exit Gate:** Playwright E2E suite renders and asserts all 10 edge states.

- [ ] **Issue `#7`: Windows Host Capture & WebRTC Media Pipeline**
  - Implement `oud-capture` on Windows (Desktop Duplication / Windows Graphics Capture + hardware H.264 with software fallback) + WebRTC P2P (`webrtc-rs`) + Cloudflare free TURN / `coturn` fallback + Auto/Sharp/Fast quality presets.
  - **Exit Gate:** Same-region benchmark achieves p50 <= 80 ms latency, 1080p@30fps in Auto mode, and Allow-to-first-frame <= 3 s (p95).

- [ ] **Issue `#8`: Protected Host Consent UI, Tray Icons & Persistent Banner**
  - Build Tauri v2 host prompt (`520x660`) with raw enrolled server domain badge, 5s first-time anti-scam cooling-off timer, Screen-only default toggles, monochrome Facing-Brackets tray icons (Idle / In Session / Alert), and top-most `End session` banner.
  - Enforce `LLMHF_INJECTED` synthetic-input rejection and remote pointer rect blocking over consent/banner windows.
  - **Exit Gate:** Automated input-injection test confirms remote input cannot click `Allow` or dismiss the banner, and unchecked input/file frames are dropped locally at the protocol gatekeeper.

- [ ] **Issue `#9`: Clipboard Sync & Opt-In File Transfer**
  - Implement directional text clipboard sync and chunked WebRTC data-channel file transfer (off by default, size/type limits, mid-transfer cancel, filename/size metadata logged).
  - **Exit Gate:** Tests verify frames fail closed whenever unchecked in `SessionGrant`.

- [ ] **Issue `#10`: Anti-Abuse Controls & 15-Minute Support Links**
  - Implement 15-minute expiring one-time support links bound to enrolled server identity, requester cooldowns after 3 denials/mismatches, and host-side Block/Report actions.
  - **Exit Gate:** Expired links (>15m) rejected; repeated denials trigger automatic lockout.

- [ ] **Issue `#11`: 1-Command Self-Hosting (`deploy/docker-compose.yml`)**
  - Package `oud-server` + `web-pwa` + optional `coturn` / Cloudflare TURN config and `docs/SELF_HOSTING.md`.
  - **Exit Gate:** Fresh Ubuntu 24.04 WSL2 instance boots the complete stack with working audit console in under 3 minutes.

- [ ] **Issue `#12`: `v0.1.0` Release, Usability Validation & SignPath**
  - Run 10-second comprehension tests on the consent card with 5 non-technical testers; tag `v0.1.0` to trigger reproducible GitHub Actions Windows MSI builds; apply to SignPath Foundation for free OSS code signing.
  - **Exit Gate:** 5/5 testers pass comprehension check; reproducible `v0.1.0` published on GitHub Releases.

---

## Milestone `v0.2` — Linux Host, Windows Hardening, Unattended Mode & Android APK (Weeks 13–20 | $0 Cost)

*Begin immediately after `v0.1.0` Exit Gates pass.*

- [ ] **Issue `#13`: Windows Privileged Service & Secure Desktop** — Split Windows agent into `SYSTEM` service + user helper for isolated consent rendering and UAC/`Ctrl+Alt+Del` handling.
- [ ] **Issue `#14`: Linux Host (Ubuntu 22.04/24.04 X11 + Wayland)** — PipeWire `ScreenCast` portal + `libei`/`uinput` with dialog-storm guards and fail-closed `logind` seat checks; CI builds `.deb`/`.AppImage` on `ubuntu-latest`.
- [ ] **Issue `#15`: Unattended Mode with Step-Up MFA** — Admin-enrolled unattended hosts, viewer step-up MFA, cached signed policies (24 h max offline window), and periodic grant reminders.
- [ ] **Issue `#16`: Android Native Viewer (`apps/mobile-capacitor`)** — Wrap `apps/web-pwa` with Capacitor for touch gestures and virtual keyboard; distribute free signed APK via GitHub Releases / F-Droid.
- [ ] **Issue `#17`: Funding & Audit Prep** — Review NLnet Restack (note: verify NLnet's strict human-authored Generative AI policy before applying) and GitHub Sponsors to fund an independent security audit and the Apple Developer fee.

---

## Milestone `v0.3` — macOS Host, iOS App & Enterprise Governance (Weeks 21–28 | Grant/Sponsor Funded)

- [ ] **Issue `#18`: macOS Host (macOS 13+)** — `ScreenCaptureKit` + `CGEvent`, guided Screen Recording & Accessibility wizard, notarised `.dmg` via GitHub Actions `macos-latest`.
- [ ] **Issue `#19`: iOS Native Viewer (`apps/mobile-capacitor`)** — View-first iOS Capacitor client.
- [ ] **Issue `#20`: SSO/OIDC, SIEM Export & Rich Policies** — OIDC/SAML login, syslog/CEF/webhook audit streaming, allowed hours, IP/country allowlists, mid-session permission escalation requests.

---

## Milestone `v1.0` — Audited Public-Interest Release (Months 8–12)

- [ ] **Issue `#21`: Third-Party Security Audit & Protocol Fuzzing** — Publish full audit report and fixes in `docs/audits/`.
- [ ] **Issue `#22`: Advanced Trust & Safety** — Guardian Mode (trusted co-approver for at-risk relatives), Sensitive-Window Protection (auto-mask banking/password-manager windows), optional encrypted session recording.