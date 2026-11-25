# Security & Anti-Abuse Policy — Oudos

Oudos is built as a consent-first, end-to-end encrypted remote desktop tool. We treat security flaws, consent bypasses, and social-engineering abuse as critical defects.

---

## 1. Reporting a Security Vulnerability

**Please do NOT open a public GitHub Issue for undisclosed security vulnerabilities.**

Instead, report vulnerabilities privately via **GitHub Private Vulnerability Reporting** (`Security` -> `Advisories` -> `Report a vulnerability` in this repository).

Please include:
* Affected component (`oud-protocol`, `oud-policy`, `oud-capture`, `oud-server`, `host-desktop`, `web-pwa`, or `mobile-capacitor`).
* Steps to reproduce, proof-of-concept input, or state-machine trace.
* Expected security invariant vs. observed behavior.

### Response Targets
* **Initial acknowledgement:** Within 48 hours.
* **Triage & severity assessment:** Within 5 business days.
* **Coordinated disclosure:** We publish a plain-language advisory and patch once verified, crediting reporters unless anonymity is requested.

---

## 2. Priority Threat Classes (In Scope)

We prioritize any bug that violates Oudos's core invariants:
1. **Consent Bypass:** Any path reaching screen capture, input injection, clipboard sync, or file transfer without an explicit, valid host-signed `SessionGrant`.
2. **Scope Escalation:** Sending mouse/keyboard, clipboard, or file frames when those capabilities are `false` in the active grant.
3. **Synthetic Input Bypass:** Remote input clicking `Allow`, modifying permission toggles, or hiding/dismissing the persistent `End session` banner.
4. **MITM / SAS Forgery:** Any server or relay capable of influencing or forging the handshake-derived 6-digit verification code.
5. **Identity Spoofing:** An un-enrolled server prompting a host or displaying falsified domain/verification badges.
6. **Policy Relaxation:** Any configuration allowing an owner or downstream client to loosen a stricter upstream monotonic policy.
7. **Audit Tampering:** Forging, reordering, or silently deleting hash-chained audit entries without breaking chain verification.

---

## 3. Abuse & Scam Prevention Reporting (24-Hour SLA Target)

Oudos is designed to disrupt remote-access phone scams, though **no remote-access software can guarantee 100% scam immunity**.

* If you encounter a malicious deployment, spoofed installer, or abuse campaign misusing the Oudos name or binaries, report it immediately via the project's abuse contact listed in the repository profile (target response: **< 24 hours**).
* Because Oudos is **AGPL-3.0 self-hosted software**, the Oudos maintainers do not have access to or control over third-party self-hosted servers—however, the host agent enforces **pre-paired server trust**, **5-second anti-scam cooling-off warnings**, **view-only defaults**, and **local host-side hash-chained audit logs** that victims can export for banks or law enforcement.

---

## 4. Supported Versions

| Version | Security Support Status |
| :--- | :--- |
| `v0.1.x` (Active Development) | Active fixes on `main` |
| Pre-release prototypes | Unsupported — do not deploy in production |