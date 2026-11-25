# Oudos Protocol & Cryptography (`oudos-v0.1`)

## 1. Device Identity & TOFU Pinning
- Each host generates an **Ed25519** identity keypair on first install; private keys stay in OS secure storage (Windows DPAPI/TPM in `v0.1`).
- Viewers pin the host public key on first verified session (TOFU). Any subsequent key change halts connection establishment with a blocking recovery alert.
- 9-digit device IDs are display locators only, never authentication tokens.

## 2. Ephemeral Handshake & 6-Digit SAS
- Peers perform an authenticated **X25519 + Ed25519** key exchange for forward secrecy.
- Both peers derive the 6-digit Short Authentication String (SAS) locally using commit-before-reveal:
  $$\text{SAS} = \text{Truncate}_{6\text{-digit}}\big(\text{HKDF-SHA256}(\text{transcript\_hash},\ \text{"oudos-sas-v1"})\big)$$
- The signaling server never generates, modifies, or observes the SAS.
- Three code mismatches or rapid host denials trigger requester rate-limit lockout.

## 3. Consent State Machine
```text
IDLE
 -> VERIFYING_IDENTITY (authenticated requester + enrolled server + rate limit pass)
    -> BLOCKED_ALERT (key mismatch or un-enrolled server)
 -> AWAITING_HOST_CONSENT (2-minute hard TTL)
    -> COOLING_OFF_5S (mandatory 5s disabled Allow for first-time requesters)
    -> DENIED_LOGGED -> IDLE (deny, TTL expiry, or SAS mismatch)
 -> GRANT_SIGNED_ACTIVE (host-signed SessionGrant + persistent banner)
    -> GRANT_UPDATED_IMMEDIATELY (mid-session permission revocation)
    -> REVOKED_LOGGED -> IDLE (End session, idle timeout, or grant expiry)
```

## 4. `SessionGrant` Contract
- Signed by host Ed25519 key; bound to `grant_id`, `session_id`, `device_pubkey`, `viewer_pubkey`, `server_domain`, `issued_at_utc`, and `expires_at_utc`.
- Defaults: `view_screen = true`; `mouse_keyboard`, `clipboard_sync`, `file_transfer`, and `blank_screen` all `false`.
- `v0.1` caps: `blank_screen` always `false`; durations restricted to `This session` or `1 hour`.

## 5. Tamper-Evident Audit Chain
- Metadata only (timestamps, identities, server domain, granted/revoked permissions, duration, file name/size). Zero pixels, keystrokes, clipboard text, or file payloads.
- Chained via $H_n = \text{SHA-256}(H_{n-1} \parallel \text{CanonicalJSON}(\text{Event}_n))$ and signed by host Ed25519 key.
