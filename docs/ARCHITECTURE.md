# Oudos Architecture

## System Overview

Oudos separates **control-plane coordination** (`oud-server`) from **end-to-end encrypted peer media and host consent enforcement** (`apps/host-desktop` + `apps/web-pwa`).

```text
+-------------------+        WSS Signaling (SDP/ICE)        +-------------------+
|   Viewer Client   | <-----------------------------------> |    oud-server     |
|  (apps/web-pwa)   |        (Ciphertext / Metadata)        |  (Axum + SQLite)  |
+-------------------+                                       +-------------------+
          ^                                                           ^
          |                                                           |
          |  Direct P2P WebRTC (DTLS-SRTP + Data Channels)            | WSS / HTTPS
          |  or TURN Relay (Ciphertext Only)                          |
          v                                                           v
+-------------------------------------------------------------------------------+
|  PC Host Agent (apps/host-desktop + oud-capture + oud-protocol + oud-policy)  |
|  - Enforces SessionGrant at protocol boundary                                 |
|  - Derives 6-digit SAS locally from handshake transcript                      |
|  - Renders protected consent card & persistent End Session banner             |
+-------------------------------------------------------------------------------+
```

## Workspace Crates & Applications

| Component | Responsibility |
| :--- | :--- |
| `crates/oud-protocol` | Ed25519 identity keys, X25519 ephemeral exchange, SAS derivation, `SessionGrant`, consent state machine, SHA-256 audit hash-chain |
| `crates/oud-policy` | Monotonic `Organisation -> Group -> Device Owner` policy merger |
| `crates/oud-capture` | Platform screen capture and host-gated input injection traits (`WindowsWgc` in `v0.1`) |
| `crates/oud-server` | Passkey/MFA auth, server key enrollment, 9-digit device registry, presence, WebSocket signaling, rate limits, append-only audit API |
| `apps/host-desktop` | Tauri v2 desktop host agent, consent card (`520x660`), monochrome tray icons, persistent top-most session banner |
| `apps/web-pwa` | React + TS + Tailwind PWA viewer, policy console, and audit log exporter |
| `apps/mobile-capacitor` | Capacitor mobile shell around `apps/web-pwa` (`v0.2` Android, `v0.3` iOS) |

## Fail-Closed Boundary

- Control-plane loss prevents **new** sessions from starting.
- Already-active P2P sessions continue until explicitly ended, idle-timed out, or grant-expired.
- Permission changes on the host take effect immediately in memory and drop disallowed frames before OS APIs.
