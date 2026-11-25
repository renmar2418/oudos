# Changelog

All notable changes to **Oudos** are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html) under **GNU AGPL-3.0** (`AGPL-3.0-only`).

---

## [0.1.0] — Pre-Release Preparation (Pending Tag & Human Validation)

### Added

#### Cryptographic Protocol & Policy Core (`crates/oud-protocol`, `crates/oud-policy`)
- Per-device **Ed25519** identity keypairs, TOFU public key pinning, and blocking key-changed recovery alerts.
- Ephemeral **X25519 + Ed25519** authenticated handshake with commit-before-reveal 6-digit SAS derivation (`HKDF-SHA256(transcript_hash, "oudos-sas-v1")`)—never generated or seen by signaling servers.
- Property-tested (`proptest`) `ConsentMachine` proving zero paths reach `GRANT_SIGNED_ACTIVE` without a valid host signature and every negative path (deny, 2-minute TTL expiry, SAS mismatch, key change, revoke) fails closed.
- Host-signed `SessionGrant` schema with view-only defaults (`view_screen: true`, `mouse_keyboard: false`, `clipboard_sync: false`, `file_transfer: false`, `blank_screen: false`) and `v0.1` duration cap (`This session` or `1 hour`).
- Monotonic policy engine (`Organisation -> Group -> DeviceOwner`) using logical `AND` for allow-flags and `min()` for numeric caps, locking Unattended Mode OFF in `v0.1`.
- Metadata-only, host-signed, SHA-256 hash-chained audit logs (`H_n = SHA-256(H_{n-1} || CanonicalJSON(Event_n))`) with CSV export and tamper detection.

#### Self-Hostable Control Plane & Anti-Abuse (`crates/oud-server`, `deploy/`)
- Axum + SQLite control plane (`oud-server`) with passkey/MFA challenge auth, persistent server Ed25519 identity keypair, pre-paired server public-key enrollment, 9-digit device registry, live presence (`Online` / `In session` / `Offline`), and fail-closed WebSocket SDP/ICE signaling.
- 15-minute expiring, single-use support links cryptographically bound to the enrolled server domain and public key.
- Sliding-window per-requester, per-device, and pair rate limits, 3-denial/mismatch automatic lockout, requester blocklist, and abuse reporting API (`POST /api/v1/abuse/report`).
- 1-command `docker compose` stack (`deploy/docker-compose.yml`) bundling `oud-server`, `web-pwa` (Nginx), optional Caddy HTTPS (`--profile tls`), and optional self-hosted `coturn` (`--profile coturn`) or Cloudflare Realtime TURN free tier, with zero paywalled features.

#### Windows Host Agent & Media Pipeline (`crates/oud-capture`, `apps/host-desktop`)
- Windows screen capture via DXGI Desktop Duplication with GDI fallback, Media Foundation hardware H.264 discovery + `OpenH264` software fallback, and `webrtc-rs` peer transport (`Auto` / `Sharp` / `Fast` presets).
- Local `HostFrameGatekeeper` validating live `SessionGrant` signatures, expiry, idle timeouts, and dropping unauthorized input, clipboard, or file frames before OS APIs.
- Windows `WH_MOUSE_LL` / `WH_KEYBOARD_LL` synthetic-input (`LLMHF_INJECTED` / `LLKHF_INJECTED`) rejection and remote cursor rect blocking over protected consent and banner surfaces.
- Tauri v2 Windows host agent (`520x660` Consent Card) featuring raw enrolled server domain badge (or explicit amber `Personal / Unverified`), 5-second first-time anti-scam cooling-off timer, 6-digit SAS code, view-only default toggles, `Block & report requester`, non-dismissible top-most `End session` banner, and 3 colour-independent Facing Brackets tray icons (`Idle`, `In Session`, `Alert`).
- Directional text clipboard sync and chunked WebRTC file transfer (off by default, executable extensions blocked, mid-transfer cancel, filename/size metadata logged only).

#### Web & Mobile-Browser PWA Viewer (`apps/web-pwa`)
- React 19 + TypeScript + Tailwind CSS v4 + Vite PWA implementing the core 5 views (**Devices**, **Requesting / Waiting**, **Active Session** dark chrome `#0E1214`, **Policies** with admin lock indicators, **Audit Log** with filters and CSV export) plus **Sessions** tab, **Add Device** pairing wizard, **15-minute support link** & abuse reporting panels, and all 10 mandatory edge states.
- Verified with Vitest + `axe-core` and Playwright Chromium WCAG 2.2 AA accessibility and contrast checks.

### Known `v0.1` Scope Boundaries
- **Strictly Asks-First:** No Unattended Mode, no `"Until I turn it off"` grants, and no Blank Remote Screen in `v0.1`.
- **Supported Platforms (`v0.1`):** Windows 10/11 Host (`oud-host-desktop`), modern desktop/mobile web browsers via `apps/web-pwa`, and Linux/Docker self-hosted control plane (`deploy/docker-compose.yml`).
- **Scheduled for `v0.2` / `v0.3`:**
  - Windows `SYSTEM` service + isolated Secure Desktop (`v0.2`, Issue `#13`)
  - Linux X11/Wayland host via PipeWire + `libei`/`uinput` (`v0.2`, Issue `#14`)
  - Unattended Mode with step-up MFA (`v0.2`, Issue `#15`)
  - Android native Capacitor APK (`v0.2`, Issue `#16`)
  - macOS host, iOS native app, and SSO/OIDC/SIEM governance (`v0.3`, Issues `#18`–`#20`)
- **Code Signing & Usability Sign-Off:** Reproducible `v0.1.0` Windows installers target SignPath Foundation free OSS signing post-`v0.1.0` tag; 5-tester 10-second consent comprehension validation is tracked in `docs/USABILITY_TEST_SCRIPT.md`.
