# Oudos Threat Model (`v0.1`)

Oudos is engineered to resist coercion, interception, and privilege escalation while communicating security guarantees honestly (never claiming "100% scam-proof").

## Mandatory Threat Vectors & Mitigations

| # | Threat Vector | Primary Mitigation |
| :--- | :--- | :--- |
| 1 | **Phone-scam coercion** | 5-second disabled `Allow` cooling-off timer + blunt amber warning for first-time requesters; view-only default; raw server domain display |
| 2 | **MITM / malicious relay** | End-to-end X25519 + Ed25519 authenticated handshake; local commit-before-reveal 6-digit SAS; relays forward ciphertext only |
| 3 | **9-digit ID enumeration** | IDs are non-secret locators; requires authenticated requester, server enrollment, and per-requester/device rate limits |
| 4 | **Rogue admin loosening owner rules** | Monotonic policy engine (`AND` for booleans, `min()` for caps) prevents downstream or upstream relaxation of stricter owner rules |
| 5 | **Viewer account takeover** | Passkey/MFA auth, per-session host consent + 6-digit SAS match, and rapid-denial/mismatch lockouts |
| 6 | **Malicious file / clipboard payload** | Both `clipboard_sync` and `file_transfer` default to `false`; chunked transfers enforce size/type limits and host-side grant checks |
| 7 | **Synthetic input clicking Allow / hiding banner** | Reject `LLMHF_INJECTED` synthetic input on consent/banner surfaces and block remote pointer coordinates over their bounding rects (`v0.1`); Secure Desktop in `v0.2` |
| 8 | **Supply-chain installer tampering** | Reproducible GitHub Actions builds with published `SHA256SUMS` and SignPath Foundation OSS code signing post-`v0.1.0` |
| 9 | **Audit log forgery / deletion** | Host Ed25519 signatures + SHA-256 hash chaining stored both locally on host and on `oud-server` with CSV export |
| 10 | **Server / relay insider access** | E2EE media/data channels; servers never hold session encryption keys or SAS values; metadata-only logs |
| 11 | **Replay of old grants** | Grants bind to unique UUIDv7 `grant_id` and `session_id`, ephemeral handshake keys, and strict UTC expiration |
| 12 | **Request flooding** | Per-requester and per-device sliding-window rate limits and 24-hour lockout after 3 denials/mismatches |
