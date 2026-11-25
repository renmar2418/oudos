# Oudos — Complete Locked Decision Log (`ADR-001`)

* **Status:** Accepted & Locked
* **Date:** November 25, 2026
* **Scope:** Brand, License, Business Model, Technical Stack, Security Architecture, and Phased Rollout

This document records every foundational decision made prior to writing production code. Contributors and coding agents must treat these decisions as binding unless formally superseded by a new ADR (`ADR-002+`).

---

## 1. Brand & Identity Decisions

* **Locked Project Name:** **Oudos** *(Ancient Greek: οὐδός — "threshold / doorstep")*.
  * *Why not `Latch` or `RemoteLatch`?* Active commercial collision (`remotelatch.com` is an existing paid Rust WebRTC remote-desktop tool; `Latch` also collides with Telefónica Latch and Latch Smart Access).
  * *Why `Oudos` over `Oudus`?* `Oudos` is the authentic Ancient Greek spelling and avoids collision with the Estonian word `õudus` ("horror").
  * *Pre-launch legal note:* Run formal trademark (Class 9 & 42), domain, and app-store checks before registering commercial entities or paid certificates.
* **Crate & Binary Prefix:** `oud-` (`oud-protocol`, `oud-policy`, `oud-capture`, `oud-server`).
* **Visual Direction:** Logo Concept 4 (**Facing Brackets with Center Dot**) paired with monochrome silhouette-distinct tray icons:
  * **Idle:** Open facing brackets, empty center.
  * **In Session:** Facing brackets enclosing a solid filled center dot.
  * **Alert:** Facing brackets with a center `!` and top-right diamond.
* **Colour & Typography System:**
  * Primary Teal `#0F7B6C` (hover `#0A6457`, deep `#074A40`), Mint Accent `#5FD9BF` (dark chrome `#0E1214` only—never light backgrounds), Pale Teal `#E3F1EE`, Ink `#14181B`, Secondary `#4A524E`, Muted `#5C6560`, Surface `#F5F6F4`, Deny/End Crimson `#B42318`, Warning Amber `#B54708`.
  * Fonts: `Space Grotesk` (headings/wordmark), `Inter` (UI/consent copy, min `14px` on consent cards), `JetBrains Mono` (device IDs, 6-digit SAS codes, latency, UTC timestamps).

---

## 2. Licensing, Competition & $0 Budget Decisions

* **Model:** **Option A — Pure Free and Open Source (FOSS)**. Zero paywalled tabs, zero crippled "Community Edition" server.
* **License:** **GNU AGPL-3.0** across the entire monorepo to prevent closed-source commercial cloud clones while remaining 100% OSI-compliant.
* **Competitive Wedge vs. RustDesk OSS:** RustDesk's free server is strictly an ID/relay broker that paywalls web management, policies, and audit logging in Server Pro. Oudos ships a 1-command `docker compose up` self-hosted stack with a free web console, monotonic policy engine, host-enforced capability grants, and hash-chained CSV audit logs out of the box.
* **$0 Launch Infrastructure Strategy:**
  * **Local Dev:** Windows 11 + WSL2 Ubuntu 24.04 (zero dual-booting or extra hardware).
  * **CI & Cross-Platform Packaging:** Free public GitHub Actions runners (`windows-latest`, `ubuntu-latest`, `macos-latest`) via `tauri-apps/tauri-action`.
  * **Relay Bandwidth:** Direct WebRTC P2P by default; fallback via self-hosted `coturn` or Cloudflare Realtime TURN free tier (verify live 1,000 GB/month quota before launch).
  * **Windows Code Signing:** Reproducible GitHub Actions builds targeting **SignPath Foundation** free OSS signing once `v0.1.0` is released.
  * **Future Funding Path:** Non-dilutive FOSS grants (e.g., NLnet Restack—observing their strict human-authored AI policy), institutional sponsorships, and optional paid managed-hosting/support services without ever paywalling code features.

---

## 3. Core Architecture & Technology Stack

* **PC Host Agent:** **Rust + Tauri v2** (shared Rust core across Windows, Linux, and macOS).
* **Web & Phone Client:** **React + TypeScript + Tailwind CSS + Vite PWA** (zero app-store cost in `v0.1`; single shared UI codebase).
* **Native Mobile Clients:** **Capacitor** wrapping `apps/web-pwa` (free signed Android APK via GitHub Releases / F-Droid in `v0.2`; iOS App Store in `v0.3` once the $99/yr Apple Developer fee is funded).
* **Control Plane & Signaling:** **Rust (Axum) + SQLite/PostgreSQL** + WebSockets packaged in `deploy/docker-compose.yml`.

---

## 4. Non-Negotiable Security & Anti-Scam Decisions

1. **Pre-Paired Server Anti-Spoofing (Both A + B Locked):**
   * **A:** Never show a "Verified" green checkmark for un-enrolled self-hosted servers; always display the raw pinned server domain (e.g., `support.example.org`) or an explicit amber `Personal / Unverified` tag.
   * **B:** Require explicit host enrollment of a server's public key before that server is permitted to deliver consent prompts.
2. **Cryptographic Consent Invariants:**
   * Per-device Ed25519 keys stored in OS secure storage; viewers pin host keys (TOFU) and halt on key changes.
   * Ephemeral X25519 + Ed25519 handshake with locally derived, commit-before-reveal 6-digit SAS code (never server-generated).
   * Host-signed `SessionGrant` checked at the host protocol boundary; unauthorized input, clipboard, or file frames are dropped before reaching OS APIs.
3. **Anti-Scam UX Defaults (Especially for Non-Technical / Zero-Finance-Knowledge Users):**
   * Mandatory 5-second cooling-off timer + blunt amber warning for first-time requesters (*"Real banks will NEVER ask you to share your screen"*).
   * View-only default (`mouse_keyboard`, `clipboard_sync`, `file_transfer` all `false` by default).
   * Blank Remote Screen completely disabled in Asks-First mode.
   * Protected top-most session banner with local-input-only `End session` button (`LLMHF_INJECTED` rejection + cursor rect blocking in `v0.1`; Windows Secure Desktop isolation in `v0.2`).
4. **Monotonic Policy Engine:** Effective rules merge strictly `Org -> Group -> Device Owner` via logical `AND` for allow-flags and `min()` for numeric caps. Owners can only tighten rules, never relax them.
5. **Tamper-Evident Audit Logging:** Metadata-only (zero pixels, keystrokes, clipboard text, or file contents), host-signed, SHA-256 hash-chained, stored both locally on the host and on the control plane with CSV export.

---

## 5. Phased Scope Decisions (`v0.1` -> `v1.0`)

* **`v0.1` Scope Lock (Ship First at $0):**
  * **Strictly Asks-First** — Unattended Mode and `"Until I turn it off"` grants are explicitly deferred to `v0.2` to cut ~30% scope and keep the anti-scam promise airtight.
  * **Platforms:** Windows 10/11 Host + React/TS PWA Viewer + Docker Control Plane.
  * **Performance Targets:** Same-region latency $p_{50} \le 80\text{ ms}$, $p_{95} \le 150\text{ ms}$; 1080p at 30fps Auto mode; Allow-to-first-frame $\le 3\text{ s}$ ($p_{95}$).
* **`v0.2` ($0):** Linux Host (X11/Wayland PipeWire with dialog-storm guards and `logind` seat checks), Windows `SYSTEM` Service + Secure Desktop, Unattended Mode with step-up MFA, and Android Capacitor APK.
* **`v0.3` (Funded):** macOS Host (ScreenCaptureKit + notarised `.dmg`), iOS Capacitor app, SSO/OIDC, and SIEM export.
* **`v1.0`:** Independent third-party security audit, Guardian Mode, and Sensitive-Window Protection.