# Oudos

**Oudos** (*Ancient Greek: οὐδός — "threshold / doorstep"*) is a Free and Open Source (**GNU AGPL-3.0**) consent-first remote desktop system.

> Nothing starts without the device owner's informed yes, every session is end-to-end encrypted, and every session is recorded in a tamper-evident audit log. Device owners can only make rules stricter, never looser.

**Current Release Status:** `v0.1.0` pre-release preparation (Windows 10/11 Host + React PWA Viewer + 1-command Docker Self-Hosted Control Plane; see [CHANGELOG.md](CHANGELOG.md) and [PLAN.md](PLAN.md)).

> [!NOTE]
> **Honest Security Posture:** Oudos is built to disrupt rushed phone-scam scripts, enforce host-side permissions, and preserve tamper-evident evidence—no remote-access software can eliminate social engineering or claim to be "100% scam-proof."

---

## Core Principles

- **Strictly Asks-First (`v0.1`):** No unattended mode, no `"Until I turn it off"` grants, and no blank remote screen. Durations are capped at `This session` or `1 hour`.
- **Host-Enforced Gatekeeper:** Capabilities (`view_screen`, `mouse_keyboard`, `clipboard_sync`, `file_transfer`) live in a host-signed `SessionGrant`. Unauthorized input, clipboard, or file frames are dropped locally at the host protocol boundary before touching OS APIs.
- **Handshake-Derived 6-Digit SAS:** Derived locally on both peers via commit-before-reveal `HKDF-SHA256(transcript_hash, "oudos-sas-v1")`. Signaling servers never generate or see the code.
- **Pre-Paired Server Trust:** Hosts only accept consent prompts from explicitly enrolled server public keys and display the raw pinned domain (or an amber `Personal / Unverified` warning).
- **Protected Consent UI:** 5-second anti-scam cooling-off timer for first-time requesters, `LLMHF_INJECTED` / `LLKHF_INJECTED` synthetic-input rejection, remote pointer rect blocking over consent/banner windows, and a non-dismissible top-most `End session` banner.
- **Monotonic Policy Engine:** Effective rules merge `Organisation -> Group -> Device Owner` (`AND` for allow-flags, `min()` for numeric caps).
- **Tamper-Evident Audit Log:** Metadata-only (zero pixels, keystrokes, clipboard text, or file contents), host-signed, and SHA-256 hash-chained with CSV export.

---

## Quick Start

### 1. Self-Host the Control Plane & Web Console (Under 3 Minutes)

100% of the console, device registry, 15-minute support links, monotonic policy engine, and hash-chained CSV audit log are free and self-hostable with **zero paywalled features** (see [docs/SELF_HOSTING.md](docs/SELF_HOSTING.md)):

```bash
cp .env.example deploy/.env
docker compose -f deploy/docker-compose.yml up -d --build
curl -fsS http://localhost:8080/health
```

Open `http://localhost:8080` for the web PWA console. Optional profiles:
- `--profile tls` — Automatic HTTPS via Caddy (`OUD_SERVER_DOMAIN` required)
- `--profile coturn` — Self-hosted TURN relay fallback (default is `$0` direct P2P WebRTC with public STUN; Cloudflare Realtime TURN free tier is also supported)

### 2. Local Development & Full Verification

**Prerequisites:** Rust stable (`rustfmt`, `clippy`), Node.js 22+, `pnpm` (`corepack enable`), and Windows 10/11 for the Tauri v2 host agent.

```powershell
# Install JS workspace dependencies
pnpm install --frozen-lockfile

# Root Rust workspace (protocol, policy, capture, server)
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

# Standalone Windows Tauri v2 host agent
cargo fmt --manifest-path apps/host-desktop/src-tauri/Cargo.toml --all -- --check
cargo clippy --manifest-path apps/host-desktop/src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path apps/host-desktop/src-tauri/Cargo.toml
cargo build --manifest-path apps/host-desktop/src-tauri/Cargo.toml --bin oud-host-desktop

# Frontend typecheck, build, accessibility (Vitest + axe-core), and Playwright E2E
pnpm typecheck
pnpm build
pnpm --dir apps/host-desktop run test
pnpm --dir apps/web-pwa run test
pnpm --dir apps/web-pwa run test:e2e
```

---

## Windows Code Signing & Checksums (SignPath)

- Reproducible Windows `.msi` / NSIS installers are built on `windows-latest` by [release-builds.yml](.github/workflows/release-builds.yml) along with `SHA256SUMS-windows.txt`.
- Free open-source Windows code signing via **SignPath Foundation** is planned once `v0.1.0` is tagged and published; until SignPath onboarding is active, always verify the published `SHA256SUMS-windows.txt` checksum before running draft/pre-release binaries.

---

## Monorepo Layout

- `crates/oud-protocol` — Ed25519 identity keys, X25519 handshake, SAS derivation, `SessionGrant`, consent state machine, hash-chain audit primitives.
- `crates/oud-policy` — Monotonic `Org -> Group -> Owner` policy merger.
- `crates/oud-capture` — Windows DXGI/GDI screen capture, H.264 pipeline, `webrtc-rs` transport, input isolation, clipboard/file transfer, and host gatekeeper.
- `crates/oud-server` — Self-hostable Axum control plane, signaling, rate limits, 15-min support links, and audit API.
- `apps/web-pwa` — React + TypeScript + Tailwind PWA viewer and admin console.
- `apps/host-desktop` — Tauri v2 Windows host agent, protected consent UI, tray icons, and persistent banner.
- `apps/mobile-capacitor` — Capacitor mobile wrapper (`v0.2+`).
- `deploy/` — `docker-compose.yml`, Dockerfiles, Nginx, Caddy, and `coturn` configuration.

---

## Documentation

- [Changelog](CHANGELOG.md)
- [Implementation Roadmap (`PLAN.md`)](PLAN.md)
- [Locked Decisions (`ADR-001`)](docs/DECISIONS.md)
- [Architecture](docs/ARCHITECTURE.md)
- [Protocol & Cryptography](docs/PROTOCOL.md)
- [Threat Model](docs/THREAT_MODEL.md)
- [Consent UX Specification](docs/CONSENT_UX_SPEC.md)
- [10-Second Usability Test Script](docs/USABILITY_TEST_SCRIPT.md)
- [Self-Hosting Guide](docs/SELF_HOSTING.md)
- [Security & Anti-Abuse Policy](.github/SECURITY.md)

## License

Licensed under **GNU AGPL-3.0** (`AGPL-3.0-only`). See [LICENSE](LICENSE).
