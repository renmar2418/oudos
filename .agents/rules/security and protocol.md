---
trigger: model_decision
description: "Apply whenever designing, implementing, or reviewing Rust crates (oud-protocol, oud-policy, oud-capture, oud-server), WebRTC signaling, cryptography, host consent enforcement, audit logging, or threat mitigations."
---

# Security, Cryptography & Protocol Rules

## 1. Device Identity & Key Pinning (TOFU)
- Generate a per-device **Ed25519** identity keypair on first host install.
- Store private keys exclusively in OS secure storage (Windows DPAPI/TPM in `v0.1`; Linux Secret Service in `v0.2`; macOS Keychain in `v0.3`). Never store plaintext private keys in JSON/SQLite.
- Pin host public keys on the viewer on first verified connection. If a host's public key changes later, **halt immediately** and display a blocking key-changed recovery prompt.
- Treat 9-digit device IDs as display locators only—never as authentication tokens.

## 2. Authenticated Key Exchange & 6-Digit SAS
- Every session executes an ephemeral **X25519 + Ed25519** authenticated key exchange (Noise `XX` or DTLS-SRTP bound to identity keys) for forward secrecy. Servers and TURN relays must see ciphertext only.
- Derive the 6-digit verification code locally on both peers using a **ZRTP-style commit-before-reveal** hash:
  $$\text{SAS} = \text{Truncate}_{6\text{-digit}}\big(\text{HKDF-SHA256}(\text{transcript\_hash},\ \text{"oudos-sas-v1"})\big)$$
- Lock out the requester after 3 code mismatches or rapid host denials.

## 3. Consent State Machine (Must Be Property-Tested)
Implement the state machine as a pure Rust module in `crates/oud-protocol` with `proptest` coverage of every invalid transition:

```text
IDLE
 -> VERIFYING_IDENTITY (requires authenticated requester + pre-paired server + rate-limit pass)
    -> BLOCKED_ALERT (on key mismatch or unknown server)
 -> AWAITING_HOST_CONSENT (hard 2-minute TTL countdown)
    -> COOLING_OFF_5S (mandatory 5-second disabled Allow button for first-time requesters)
    -> DENIED_LOGGED -> IDLE (on Deny, TTL expiry, or SAS mismatch)
 -> GRANT_SIGNED_ACTIVE (requires host-signed SessionGrant; persistent banner mounted)
    -> GRANT_UPDATED_IMMEDIATELY (when owner toggles a permission off mid-session)
    -> REVOKED_LOGGED -> IDLE (on End session, idle timeout, or grant expiry)
```

- **Fail-Closed Rule:** If the control plane goes offline, an already-active P2P session may continue until revoked or expired, but **zero new sessions** may start.

## 4. Host-Enforced `SessionGrant` Schema
Every allowed session produces an in-memory, host-signed `SessionGrant`:
```json
{
  "grant_id": "uuid-v7",
  "session_id": "uuid-v7",
  "device_pubkey": "ed25519:...",
  "viewer_pubkey": "ed25519:...",
  "server_domain": "support.example.org",
  "permissions": {
    "view_screen": true,
    "mouse_keyboard": false,
    "clipboard_sync": false,
    "file_transfer": false,
    "blank_screen": false
  },
  "issued_at_utc": "2026-11-25T12:00:00Z",
  "expires_at_utc": "2026-11-25T13:00:00Z",
  "host_signature": "ed25519_sig:..."
}
```
- **Defaults:** Only `view_screen` starts `true`. `mouse_keyboard`, `clipboard_sync`, `file_transfer`, and `blank_screen` MUST default to `false`.
- **v0.1 Hard Cap:** `blank_screen` is always `false` in Asks-First mode; grant durations are strictly `This session` or `1 hour`.
- **Protected Consent UI:** In `v0.1` on Windows, reject synthetic/injected input (`LLMHF_INJECTED`) on consent/banner windows and block remote cursor coordinate forwarding over their bounding rects. In `v0.2`, split into a privileged `SYSTEM` service + isolated Windows Secure Desktop surface.

## 5. Monotonic Policy Engine (`crates/oud-policy`)
- Hierarchy: `Organisation -> Group -> Device Owner`.
- Merge rules:
  - Boolean permissions: `effective = org && group && owner`
  - Numeric caps (idle timeout, max grant duration, file size): `effective = min(org, group, owner)`
- Enforce all 5 baseline rules: (1) Ask before connection, (2) Require matching 6-digit code, (3) Unattended mode (locked OFF in `v0.1`), (4) File transfer off by default, (5) Idle timeout (`15m`, `30m`, `1h`, `never`).

## 6. Tamper-Evident Hash-Chained Audit Log
- Log **metadata only** (timestamps, identities, server domain, granted/revoked permissions, duration, file name/size). **Never** log screen pixels, keystrokes, clipboard contents, or file payloads.
- Sign each entry with the host Ed25519 key and chain it:
  $$H_n = \text{SHA-256}(H_{n-1}\ \|\ \text{CanonicalJSON}(\text{Event}_n))$$
- Persist logs both **locally on the host** and on `oud-server` with CSV export.

## 7. Threat Model Checklist (12 Mandatory Vectors)
Verify every PR against: (1) Phone-scam coercion, (2) MITM/malicious relay, (3) 9-digit ID enumeration, (4) Rogue admin loosening owner rules, (5) Viewer account takeover, (6) Malicious file/clipboard payloads, (7) Synthetic input clicking Allow/hiding banner, (8) Supply-chain installer tampering, (9) Audit log forgery/deletion, (10) Server/relay insider access, (11) Replay of old grants, (12) Request flooding.