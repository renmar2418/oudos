# Consent-First Remote Desktop: Features, Non-Functional and Technical Requirements

Working name: \[YOUR APP\] (placeholder in the prototype). Prepared 25 Nov 2026 from the five-screen prototype in Remote\_Desktop\_Access.html plus public web research. Priority tags: \[M\] must for MVP, \[S\] should for V1, \[C\] could, later.

## 0. How to use this brief

Paste the whole brief into your business-model and brand-logo skill. Sections 1-2 give context, 3-6 are the requirements, 7 lists gaps in the prototype, 8 is the input for the business model, 9 is the input for the brand and logo. Anything marked (assumption) is my proposal, not something the prototype says. Numeric targets are starting points to validate, not commitments. Market figures come from third-party pages and change often, so re-check them before using them in a pitch.

## 1. What the prototype shows

Product in one sentence: a remote desktop app where nothing starts without the device owner's informed yes, every session is end-to-end encrypted, and every session is written to an audit log. The prototype's own promise line: Sessions are encrypted end to end and written to the audit log. Its policy principle: owners can only make rules stricter, never looser.

Five screens:

1. Devices: searchable list with filters (All, Online, In session, Offline). Each device shows name, OS, 9-digit ID and access mode (Unattended or Asks first), with actions Connect, Ask to join, Notify me when online, plus a Screen unavailable state.
2. Connecting: three steps (device verified, waiting for owner approval with a 2-minute expiry, open secure channel), a 6-digit code both people compare, and a summary of what is requested (screen, mouse and keyboard, clipboard, file transfer).
3. Live session: remote screen with Encrypted badge and latency (38 ms in the sample), quality modes Auto, Sharp, Fast, plus Clipboard, Files, Full screen, End session. A side panel shows who connected, who approved, what is allowed and not allowed, with toggles for sending keyboard shortcuts, showing the remote cursor and blanking the remote screen.
4. Host consent prompt: 'Sam T. wants to connect', code match, permission checkboxes, duration (This session only, 1 hour, Until I turn it off), Deny or Allow, and a note that a banner stays on screen with an End session control.
5. Access policies and audit log: five connection rules, then a log with When, Who, Device, What happened, Length and Export CSV.

Sample data worth reusing in mockups: Reception-PC (Windows 11, unattended), Design-Mac-02 (macOS 15, asks first), Lab-Server-01 (Ubuntu 24.04, in session), Laptop-Amir (Windows 11, offline), Studio-Mini (macOS 14), Warehouse-Kiosk (Windows 10, offline). People: Sam T., Dana R., Amir K. These hint at studios, small offices, labs and kiosks as early use cases.

Visual language already in the prototype: deep teal primary #0F7B6C (hover #0A6457, darkest #074A40), mint accent #5FD9BF, pale teal tint #E3F1EE, near-black ink #14181B, green-grey neutrals (#4A524E, #8A938E, #DADDD8), page surface #F5F6F4, dark session chrome #0E1214. Fonts: Inter, Space Grotesk, JetBrains Mono. Corner radii 10-16 px with pill chips.

## 2. Market research findings

Prices and claims below come from third-party pages, some written by competitors, so treat them as indicative.

Competitor price anchors:

- Splashtop Solo from about $6 per month billed annually; TeamViewer from about $24.90 per month (roundup on dupple.com, prices checked Oct 2026).
- AnyDesk: Splashtop's own comparison page says Solo is $28.90 and Standard $49.90 per month in 2026, up from $14.90 and $22.90 in 2024. Capterra lists a lower starting price (22.90 EUR per month flat rate), so the AnyDesk figure is unconfirmed.
- RustDesk: free self-hostable core and server under the AGPL licence, with a paid Server Pro tier (RustDesk's own blog).
- Chrome Remote Desktop is free for personal use; Parsec is positioned for low-latency creative work (dupple.com).
- Pattern: cheap or free entry, then upsell on teams, unattended access, audit and management. Recent incumbent price rises may open a switching window.

The abuse problem (the sharpest differentiator):

- 2026 warnings from the Swiss NCSC (April), urlscan (March), a US bank (July) and Bermuda police (October) describe fraudsters phoning victims and pressing them to install legitimate tools such as AnyDesk or TeamViewer, then moving money.
- An earlier Which? analysis named TeamViewer as the brand most often reported as misused. The ACCC said Australians reported losing 15.5 million dollars to remote access scams in 2023, and Q1 2024 losses rose 52 percent on the prior quarter. Fortra reported AnyDesk restricting direct downloads after phishing abuse.
- Implication: the prototype's design (see who is asking, compare a code, grant only what is needed, banner, end anytime, full audit) answers this directly. Position it as the remote tool built so a stranger cannot easily talk someone into handing over their computer. Never claim scam-proof: social engineering can be reduced, not eliminated.

## 3. Functional features

### 3.1 Devices (screen 1)

- F-D1 \[M\] Device list: name, OS, 9-digit ID, access mode (Unattended or Asks first), status line.
- F-D2 \[M\] Live status: Online, In session (with who), Offline (last seen). Updates without refresh.
- F-D3 \[M\] Search by name, ID or OS. Filter chips with live counts: All, Online, In session, Offline.
- F-D4 \[M\] Add device: install agent, pair with a single-use code, QR or link, name it, choose mode, record its key. (No screen for this in the prototype.)
- F-D5 \[M\] State-specific actions: Connect (online), Ask to join (in session), Notify me when online (offline). Define each flow end to end.
- F-D6 \[M\] Screen unavailable state with the reason (locked, asleep, permission missing, no display) and a fix.
- F-D7 \[S\] Groups and tags, rename, transfer ownership, remove device (revokes keys, ends sessions).
- F-D8 \[S\] Wake sleeping devices, agent version and health, update status.
- F-D9 \[C\] Share a device with named people, favourites, recents.

### 3.2 Connecting (screen 2)

- F-C1 \[M\] Three-step progress (device verified, owner approval, secure channel) with plain-language status per step.
- F-C2 \[M\] Device verification against the key saved on first connection. If the key changed, stop with a clear warning and a recovery path.
- F-C3 \[M\] Owner approval with 2-minute expiry, countdown, cancel, and defined outcomes for deny, expiry and owner offline.
- F-C4 \[M\] 6-digit code on both screens; mismatch means cancel.
- F-C5 \[M\] Show requested permissions before sending (screen, mouse and keyboard, clipboard, files); the requester can ask for less.
- F-C6 \[S\] Skip approval only for admin-marked unattended devices, with step-up MFA for the viewer.
- F-C7 \[S\] Ask for more permissions mid-session through a new host prompt.
- F-C8 \[C\] Scheduled sessions pre-approved by the owner.

### 3.3 Live session (screen 3)

- F-L1 \[M\] Remote screen, full screen, scaling, multi-monitor switcher, HiDPI.
- F-L2 \[M\] Mouse and keyboard with shortcut pass-through, a send-shortcuts menu (Ctrl+Alt+Del, Win and Cmd keys), correct keyboard layouts and IME input.
- F-L3 \[M\] Quality modes Auto (adaptive, up to 1080p), Sharp, Fast; latency readout; Encrypted badge.
- F-L4 \[M\] Clipboard sync (text first) with direction control.
- F-L5 \[M\] File transfer, off by default, granted per session; progress, cancel, size and type limits, optional malware-scan hook.
- F-L6 \[M\] Session panel: who connected and role, who approved, allowed, not allowed. End session always visible.
- F-L7 \[M\] Show remote cursor toggle.
- F-L8 \[S\] Blank remote screen, allowed only if the owner granted it or the device is unattended; otherwise disabled with a reason.
- F-L9 \[S\] View-only mode, remote audio, session chat, remote restart, auto-reconnect that keeps the grant.
- F-L10 \[C\] Opt-in session recording (host consent, encrypted), co-viewing with driver and observer roles, mobile and web viewers.

### 3.4 Host consent and control (screen 4)

- F-H1 \[M\] Prompt showing who is asking (name, verified organisation), which device, and the code to compare.
- F-H2 \[M\] Permission toggles: See screen (required), Mouse and keyboard, Clipboard, Files (not requested, can be ticked anyway).
- F-H3 \[M\] Duration: This session only (default), 1 hour, Until I turn it off. Deny and Allow.
- F-H4 \[M\] Persistent banner naming the viewer with an End session button the viewer cannot hide.
- F-H5 \[M\] Host can revoke any permission or end the session at any time, effective immediately.
- F-H6 \[S\] Block a requester, report as suspicious, extra warning for first-time requesters.
- F-H7 \[S\] Reminders while an Until I turn it off grant is active, with a policy cap on maximum duration.
- F-H8 \[S\] Tray or menu-bar app with status, active grants and the device's own history.

### 3.5 Access policies (screen 5)

- F-P1 \[M\] The five prototype rules: ask before each connection (skipped only for admin-marked unattended devices), require matching code, allow unattended access, allow file transfer (off by default), end idle sessions after 15 min, 30 min, 1 hour or never.
- F-P2 \[M\] Owners can only tighten. Effective policy is the most restrictive of all levels; show lock icons where an admin set a rule.
- F-P3 \[S\] More rules: allowed hours, IP or country allowlist, MFA to start, max session length, clipboard direction, allow blanking, require recording, max grant duration, approved requesters only.
- F-P4 \[S\] Scopes: organisation, group, device. Versioned, previewable, with history and rollback.
- F-P5 \[C\] Policy templates for regulated industries; policy as code through the API.

### 3.6 Audit log (screen 5)

- F-A1 \[M\] Table with When, Who, Device, What happened, Length. Include denied requests and sessions still open.
- F-A2 \[M\] Export CSV.
- F-A3 \[M\] Record events, not content: requests, approvals and denials with reasons, permissions granted or changed, file names and sizes (never contents), clipboard use (never contents), session end reason, policy changes, sign-ins, device enrol and remove.
- F-A4 \[S\] Filters, search, saved views, session detail page, retention setting.
- F-A5 \[S\] Tamper-evident log, SIEM export (syslog, CEF, webhook), alerts for denied requests, after-hours use, repeated code mismatches.
- F-A6 \[S\] Owners can see their own device's log.
- F-A7 \[C\] Live active-sessions board with a kill switch.

### 3.7 Accounts, teams, billing

- F-U1 \[M\] Sign-up and sign-in with passkeys or MFA; organisation workspace; roles Owner (of a device), Admin, Member (Member is an assumption).
- F-U2 \[M\] Invite users and assign devices.
- F-U3 \[S\] SSO (SAML, OIDC) and SCIM.
- F-U4 \[S\] Plans, device and seat counts, invoices, usage view.
- F-U5 \[S\] One-time support link for a customer with no account: starts only from a verified supporter, shows the supporter's organisation, expires in 15 minutes (assumption).
- F-U6 \[C\] Partner console for MSPs: multi-tenant, white-label.

### 3.8 Trust and safety (brainstorm)

- F-T1 \[S\] First-time requester interstitial: Did someone call or message you asking for this? (copy to be user-tested).
- F-T2 \[S\] Verified organisation badge and verified email domain on every consent prompt.
- F-T3 \[S\] Optional cooling-off: Allow button enabled after a short delay for first-time requesters.
- F-T4 \[S\] Rate limits per requester and per device, abuse reporting, takedown process, contact channel for banks and authorities.
- F-T5 \[C\] Guardian mode: a trusted person must co-approve unknown requests for at-risk users.
- F-T6 \[C\] Sensitive-window protection: pause control or mask when banking pages or password managers are in focus.

## 4. Non-functional requirements (numeric targets are assumptions to validate)

Performance

- N-1 Input-to-pixel latency p50 at or under 80 ms and p95 at or under 150 ms for same-region sessions on a 20 Mbps link (the prototype sample shows 38 ms).
- N-2 Auto holds 1080p at 30 fps or better; Sharp reaches 60 fps when bandwidth allows; Fast stays usable at 1.5 Mbps.
- N-3 Allow click to first frame in 3 s or less (p95); consent prompt appears on an online host within 5 s (p95).
- N-4 Host agent: under 1 percent CPU idle, under 15 percent of one core in session with hardware encoding, under 150 MB memory.

Reliability

- N-5 Control plane (sign-in, devices, consent, policy) 99.9 percent monthly at launch, 99.95 percent by V1. Running sessions survive a control-plane outage.
- N-6 Auto-reconnect within 10 s after a network change or sleep and wake, keeping the grant.
- N-7 Fail closed: if consent or policy services are unreachable, no new session starts. Admin-marked unattended devices may use cached signed policy for up to 24 h (configurable).

Security

- N-8 End-to-end encryption of screen, input, clipboard and files; servers and relays see ciphertext only.
- N-9 Permissions are enforced on the host, not only by the server.
- N-10 No standing vendor-staff access to customer sessions; any staff access to metadata is logged.
- N-11 Independent penetration test before launch and yearly; bug bounty by V1; SOC 2 Type II target 12-18 months after launch (assumption).
- N-12 Keys in OS secure storage (TPM or DPAPI, Secure Enclave or Keychain, Secret Service). Signed installers and updates with staged rollout and rollback.

Privacy and compliance

- N-13 Never store screen content, keystrokes, clipboard or file contents on servers. Recordings are opt-in, encrypted with customer-held keys.
- N-14 Audit log is metadata only: 90 days retention by default, longer on paid plans; export and delete on request.
- N-15 GDPR-ready (DPA, subprocessor list, data-subject requests); regional data residency by V1. Check local privacy law in each launch market.
- N-16 Acceptable-use terms forbid unauthorised access; abuse reports answered within 24 h (assumption).
- N-17 Regulated buyers (health, finance) will ask about HIPAA, PCI and SOX controls: prepare answers, not launch blockers (assumption).

Usability and accessibility

- N-18 A non-technical person understands the consent prompt in under 10 seconds; plain language, tested with real users including older adults.
- N-19 WCAG 2.2 AA on web and apps: full keyboard use, screen-reader labels, contrast, reduced motion, state never shown by colour alone.
- N-20 Internationalisation from day one (RTL-ready, locale formats); remote typing correct for non-Latin keyboards and IMEs.

Compatibility, scale and cost

- N-21 Hosts: Windows 10/11, macOS 13+ (prototype shows 14 and 15), Ubuntu 22.04/24.04 on X11 and Wayland. Viewers: Windows, macOS, Linux, modern browsers, iOS and Android (view-first, see 5.2).
- N-22 Multi-monitor, HiDPI, mixed DPI; works behind corporate NAT with UDP direct and TCP 443 fallback.
- N-23 Phase 1 capacity: 100k registered devices and 10k concurrent sessions per region, scaling horizontally.
- N-24 Direct peer-to-peer is the default path; relay only when needed. Track relay share, since bandwidth is the main variable cost.
- N-25 Infra and relay cost under 10-15 percent of revenue per paid device (assumption).

Operability and quality

- N-26 OpenTelemetry metrics and traces, session-quality telemetry without content, SLO dashboards, status page, on-call. RPO 5 min and RTO 1 h for the control plane.
- N-27 Automated tests for the consent state machine (including negative paths), protocol fuzzing, network chaos (loss, jitter), OS beta regression, accessibility checks in CI.
- N-28 Licence hygiene: inventory third-party licences, avoid copyleft contamination, check encryption export rules.

## 5. Technical requirements and architecture

### 5.1 Components

- T-1 Host agent: privileged service plus user-session helper. Handles capture, input injection, clipboard, file transfer, consent UI, banner, permission enforcement, signed event reporting.
- T-2 Viewer clients: desktop (Windows, macOS, Linux), web (WebRTC), mobile.
- T-3 Control plane: identity and auth, device registry, policy engine, consent broker, audit service, billing.
- T-4 Connectivity: signaling, STUN and TURN with ICE, regional encrypted relays.
- T-5 Data: relational database (orgs, users, devices, policies), append-only audit store, object storage (exports, recordings), cache for presence.

### 5.2 Platform specifics (verify per OS version)

- Windows: Desktop Duplication or Windows Graphics Capture, SendInput. Needs a Windows service plus a per-session helper; the lock screen and UAC prompts run on the secure desktop and need service-level handling. Ctrl+Alt+Del also needs service-level handling.
- macOS: ScreenCaptureKit and CGEvent. The owner must grant Screen Recording and Accessibility permission in System Settings, so build guided permission onboarding and the Screen unavailable state around this.
- Linux: X11 and Wayland. Wayland uses PipeWire portals and libei or uinput, and portals ask for consent per session, which complicates unattended access.
- Mobile (assumption to verify): iOS is typically view or share-your-screen only because of platform limits; Android control needs an accessibility service and passes app-store policy review.

### 5.3 Media pipeline

- T-6 Hardware encoders (NVENC, Intel QSV, AMD AMF, VideoToolbox, VAAPI) with software fallback. H.264 everywhere, HEVC or AV1 where supported.
- T-7 Screen-content tuning: dirty-region updates, 4:4:4 chroma for Sharp text, frame pacing, adaptive bitrate with congestion control, resolution and fps ladder (Auto up to 1080p, extend to 1440p and 4K later).
- T-8 Transport over WebRTC (DTLS-SRTP, data channels) or QUIC. Input on a low-latency channel; clipboard and files on a separate channel with backpressure. Optional Opus audio.

### 5.4 Cryptography and identity

- T-9 Per-device identity key pair generated on the device, private key in TPM, Secure Enclave or Keychain where available. Public key registered; viewers pin it on first use (this is the prototype's device key matches the saved one).
- T-10 Per-session authenticated key exchange (Noise or DTLS with identity binding), forward secrecy, keys never leave the endpoints.
- T-11 The 6-digit code must be a short authentication string derived from the session's key-agreement transcript on both ends (ZRTP-style, with commit before reveal), not a number issued by the server. Otherwise it cannot catch a man-in-the-middle. Limit attempts per request.
- T-12 User auth with passkeys (WebAuthn), OIDC or SAML, short-lived tokens, single-use enrollment tokens.
- T-13 Capability grants: the host issues a signed grant (viewer, device, permissions, expiry). The agent drops any input, clipboard or file frame outside the grant. Revocation takes effect locally at once.
- T-14 Policy engine merges levels monotonically (effective rule = most restrictive of organisation, group, owner). Agents cache signed policy bundles.

### 5.5 Consent UI integrity (critical)

- T-15 Consent prompt and banner are drawn by the privileged agent on a surface remote input cannot reach (secure desktop on Windows; protected top-level window with input filtering on macOS and Linux). Remote pointer or keyboard events are never delivered to them, so a viewer cannot click Allow for themselves.
- T-16 Detect and log synthetic input aimed at the consent window.
- T-17 Banner cannot be dismissed while a session is active, survives screen blanking, and End session works even on a degraded connection.
- T-18 Requester identity text is server-verified and rendered in a fixed format with length limits, so names like Your Bank (verified) cannot be spoofed.

### 5.6 Audit design

- T-19 Both host and server emit events; host events are signed with the device key. Server stores them append-only and hash-chained with periodic checkpoints; exports are signed; a WORM tier for compliance customers.
- T-20 Event fields: id, UTC time, actor, device, session id, action, permissions, outcome and reason, duration, coarse IP or country, client version.

### 5.7 Core data model

- T-21 Org, User, Role, Device (display ID, public key, OS, mode, owner, group, last seen), Policy (scope, rules, version, signature), ConsentRequest (requester, device, requested permissions, code state, expires\_at, status), Session (grant, start, end, end reason, quality stats), AuditEvent, Invitation, Subscription and Usage.
- T-22 9-digit IDs (about a billion values) are guessable. An ID alone must never be enough to request access: require an authenticated requester, rate limits, and organisation membership or an allowlist. Allow anyone-can-ask only if the owner turns it on.

### 5.8 APIs and deployment

- T-23 Public REST API and webhooks (devices, sessions, audit, policies), CLI; Terraform provider later.
- T-24 Silent install with enrollment token (MSI, PKG, DEB), MDM support (Intune, Jamf, GPO), signed auto-update.
- T-25 Self-hosted or on-prem relay option for regulated and cost-sensitive buyers (V1 or later).

### 5.9 Suggested stack (options, not requirements)

- Agent, relay and protocol core in Rust, with C++, Swift or Objective-C shims where OS APIs require them.
- Desktop viewer native or Tauri; web viewer TypeScript with WebRTC; mobile Swift and Kotlin.
- Backend in Go or TypeScript, PostgreSQL, Redis, NATS or Kafka, S3-compatible storage, Kubernetes, OpenTelemetry.
- Licensing: RustDesk's core is AGPL per its own site, so building on it would carry copyleft obligations for your product. Get legal advice before reusing any copyleft code. WebRTC has a permissive licence and is a common base.

### 5.10 Threat model must cover

1. A stranger talking a user through clicking Allow.
2. Man-in-the-middle or malicious relay.
3. Device ID enumeration and request spam.
4. Rogue or compromised admin.
5. Viewer account takeover.
6. Malicious file or clipboard payload.
7. Synthetic input to the consent window.
8. Installer or update supply-chain tampering.
9. Log forgery or deletion.
10. Vendor insider access.
11. Replay of old approvals.
12. Request flooding a host.

### 5.11 Testing and release

- T-26 Model-check or property-test the consent state machine: no path reaches connected without host approval, and revocation always wins.
- T-27 External pen test pre-launch, protocol fuzzing, chaos network tests, OS beta compatibility runs.
- T-28 Phased rollout: private beta, public beta, general availability.

## 6. Suggested phasing

- MVP: all \[M\] items. Goal: two people complete a consented, encrypted, logged session across Windows and macOS, with the policy rules and CSV export. Linux host and web viewer follow shortly after.
- V1: all \[S\] items: SSO, unattended access with step-up MFA, richer policies, SIEM export, trust-and-safety features, Wayland support, view-first mobile viewer, billing, data residency.
- Later: \[C\] items such as recording, co-viewing, guardian mode, sensitive-window protection, partner console.

## 7. Gaps and open questions in the prototype

- The navigation has a Sessions tab but there is no Sessions screen.
- Missing screens: Add device and onboarding, installer and permission setup (especially macOS), settings, users and roles, billing, mobile layouts.
- Missing states: owner denies, request expires after 2 minutes, code mismatch, device key changed, device goes offline mid-session, host permission missing, poor connection, session-ended summary.
- The host banner and tray menu are mentioned but not designed. Decide whether the consent prompt is an OS-native dialog or a custom window (the prototype shows a 520 by 660 custom card).
- Ask to join on a device already in a session is ambiguous: does it ask to start your own session or to watch the current one? Decide single versus multi-viewer.
- Notify me when online: which channel (push or email), how long it lasts, and what it reveals about the owner's presence.
- Blank the remote screen can be abused against a present owner; restrict it as in F-L8.
- Until I turn it off creates open-ended access; add a maximum and reminders (F-H7).
- Unattended mode: define who may mark a device, how it is enrolled, and what extra authentication the viewer needs.
- Simulate approval is a prototype-only button; remove it from the product.
- The role vocabulary (Owner, Admin, You, a named person) needs a glossary so labels stay consistent.

## 8. Business-model inputs

Customer segment hypotheses (from the sample data and research):

- Families and friends helping relatives with their computers (consumer helper): scam-resistance is the hook.
- Small IT shops and MSPs supporting many small clients: need audit, unattended access, white-label.
- Small and mid-size businesses managing their own fleet: reception PCs, kiosks, lab servers.
- Regulated teams (finance, health, public sector): audit trail, consent records, data residency.
- Creative studios and labs reaching powerful machines: latency and quality matter.
- Open question: which segment is first? Brand and pricing change a lot between consumer and MSP.

Value proposition candidates:

- Nothing starts without a yes, and every session is on the record.
- The remote tool a stranger cannot easily talk you into using.
- Admin control without removing the owner's veto (owners can only tighten).
- Plain-language consent anyone can follow.

Revenue model options to test:

- Free personal tier (asks-first only, few devices, no unattended access).
- Per device per month for fleets; per technician for support teams; per seat for small teams.
- Add-ons: unattended access, audit retention and SIEM export, SSO and SCIM, recording, white-label, self-hosted relay, data residency.
- Usage pricing only where bandwidth cost is real (relayed hours).
- Price anchors: see section 2 (entry plans roughly $6 to $50 per month across competitors; verify before use).

Cost drivers:

- Relay bandwidth: a relayed 1080p stream at 2-6 Mbps moves roughly 1-3 GB per hour (my estimate), so maximise direct connections.
- Security and compliance programme (pen tests, SOC 2, bug bounty), code signing and notarisation, abuse handling and support, payment fees, OS-compatibility upkeep.

Go-to-market ideas:

- Trust-first content: publish the threat model and a plain-language security page; invite audits.
- Switching guides against incumbents (price rises, scam reputation).
- Channels: MSPs, IT communities, schools and community help desks; banks, telcos and insurers as a longer partnership play.
- Consider open-sourcing the agent for auditability (trade-off: copying risk and licence choice).

Moat: consent UX plus policy and audit engine, trust reputation, certifications, partner integrations.

Key risks: abuse liability and reputational harm; OS permission changes (macOS, Wayland, mobile); incumbents cutting prices; relay cost; long enterprise sales cycles; over-claiming security.

Metrics: time to first consented session, session success rate, deny rate, p95 latency, paid devices, net revenue retention, abuse reports per 1,000 sessions.

## 9. Brand and logo brief

Essence: ask first, then help. Calm, human, transparent.

Personality: trustworthy, calm, precise, warm, plain-spoken. Not hacker-style, aggressive or surveillance-feeling.

Voice: short plain sentences that say what happens next, such as 'Waiting for the owner to approve' and 'The owner can end this session at any time' (both from the prototype).

Starting visual system: the prototype's teal and mint palette, Inter with Space Grotesk and JetBrains Mono (mono for IDs and codes), 10-16 px radii. Use the dark session chrome (#0E1214) as the reversed-logo background.

Logo concept directions to explore:

1. Open door or keyhole screen: a rounded-square screen with a narrow vertical gap, the gap being the ask.
2. Nod: two overlapping rounded squares whose overlap forms a checkmark.
3. Cursor at a threshold: an arrow cursor stopped at a doorway.
4. Six-dot code: six dots in a 3 by 2 grid with one in mint, echoing the verification code.
5. Brackets: two facing brackets forming a screen frame with a mint dot (the yes) in the centre.
6. Knock: a rounded square with three short marks.

Name ideas (placeholders; I have not checked trademarks, domains or app-store names): Nod, Knock, Threshold, Ajar, Handshake, Permit, Vouch, Doorstep.

Logo deliverables and constraints:

- Symbol, wordmark, horizontal and stacked lockups; one-colour and reversed versions.
- Legible at 16 px (favicon) and as a monochrome tray or menu-bar icon with idle, in-session and alert states.
- App icons for macOS, Windows, iOS and Android, plus the on-screen session banner badge.
- Distinct from incumbents: avoid the generic monitor-plus-arrow and padlock-only clichés.
- Status never conveyed by colour alone (shape or label too).
