# Oudos

**Oudos** (*Ancient Greek: οὐδός — "threshold / doorstep"*) is a Free and Open Source (**GNU AGPL-3.0**) consent-first remote desktop system.

> Nothing starts without the device owner's informed yes, every session is end-to-end encrypted, and every session is recorded in a tamper-evident audit log. Device owners can only make rules stricter, never looser.

## Core Principles

- **Strictly Asks-First (`v0.1`):** No unattended mode, no `"Until I turn it off"` grants, and no blank remote screen.
- **Host-Enforced Gatekeeper:** Capabilities (`view_screen`, `mouse_keyboard`, `clipboard_sync`, `file_transfer`) are encoded in a host-signed `SessionGrant`. Unauthorized frames are dropped at the host protocol boundary before reaching OS APIs.
- **Handshake-Derived 6-Digit SAS:** Derived locally on both peers via commit-before-reveal `HKDF-SHA256(transcript_hash, "oudos-sas-v1")`. Signaling servers never generate or see the code.
- **Pre-Paired Server Trust:** Hosts only accept consent prompts from explicitly enrolled server public keys and display the raw pinned domain.
- **Monotonic Policy Engine:** Effective rules merge `Organisation -> Group -> Device Owner` (`AND` for allow-flags, `min()` for numeric caps).
- **Tamper-Evident Audit Log:** Metadata-only (zero pixels, keystrokes, clipboard text, or file contents), host-signed, and SHA-256 hash-chained with CSV export.

## Monorepo Layout

- `crates/oud-protocol` — Ed25519 identity keys, X25519 handshake, SAS derivation, `SessionGrant`, consent state machine, hash-chain audit primitives.
- `crates/oud-policy` — Monotonic `Org -> Group -> Owner` policy merger.
- `crates/oud-capture` — OS screen capture and host-gated input injection traits.
- `crates/oud-server` — Self-hostable Axum control plane, signaling, and audit API.
- `apps/web-pwa` — React + TypeScript + Tailwind PWA viewer and admin console.
- `apps/host-desktop` — Tauri v2 host agent, protected consent UI, tray icons, and persistent banner.
- `apps/mobile-capacitor` — Capacitor mobile wrapper (`v0.2+`).

## Documentation

- [Locked Decisions (`ADR-001`)](docs/DECISIONS.md)
- [Architecture](docs/ARCHITECTURE.md)
- [Protocol & Cryptography](docs/PROTOCOL.md)
- [Threat Model](docs/THREAT_MODEL.md)
- [Consent UX Specification](docs/CONSENT_UX_SPEC.md)
- [Self-Hosting Guide](docs/SELF_HOSTING.md)
- [Security & Anti-Abuse Policy](.github/SECURITY.md)

## License

Licensed under **GNU AGPL-3.0** (`AGPL-3.0-only`). See [LICENSE](LICENSE).
