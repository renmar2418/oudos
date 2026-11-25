# Consent-First Remote Access: Business Model, Brand & Next Decisions

* [A. Business Model](#a-business-model)
  * [1. Lean Canvas](#1-lean-canvas)
  * [2. Recommended First Segment](#2-recommended-first-segment)
  * [3. Pricing Structures & Test Hypotheses](#3-pricing-structures--test-hypotheses)
  * [4. Free-Tier Abuse Controls](#4-free-tier-abuse-controls)
  * [5. Unit Economics](#5-unit-economics)
  * [6. First 90 Days GTM, Risks & Metrics](#6-first-90-days-gtm-risks--metrics)
* [B. Brand and Logo](#b-brand-and-logo)
  * [1. Name Candidates](#1-name-candidates)
  * [2. Brand Essence, Personality & Voice](#2-brand-essence-personality--voice)
  * [3. Four Logo Concepts](#3-four-logo-concepts)
  * [4. Palette & Typography System](#4-palette--typography-system)
  * [5. Tray Icon States & On-Screen Session Banner Badge](#5-tray-icon-states--on-screen-session-banner-badge)
* [C. 10 Decisions You Must Make Next](#c-10-decisions-you-must-make-next)

---

# A. Business Model

## 1. Lean Canvas

* **Customer Segments**
  * *Consumer helpers:* Tech-savvy relatives supporting family members; motivated by scam resistance and plain-language prompts.
  * *MSPs:* Small IT service providers managing multi-client fleets; need unattended access, audit trails, and white-label partner consoles.
  * *Small businesses (SMBs):* 15–100 person firms with lean internal IT/ops supporting mixed devices (reception PCs, lab servers, kiosks, shared Macs) plus occasional external clients.
  * *Regulated teams:* Finance, healthcare, and public-sector teams requiring tamper-evident consent logs, MFA, and data residency.
  * *Creative studios:* Post-production, CAD, and design teams needing low-latency, high-color-accuracy (4:4:4) access to workstations.
* **Problem**
  * Legitimate incumbents (AnyDesk, TeamViewer) are widely exploited in phone-based remote-access scams—Australia's ACCC reported $15.5M in remote-access scam losses in 2023 with Q1 2024 rising 52% quarter-on-quarter, and a Which? analysis named TeamViewer as the brand most frequently reported as misused (sources cited in brief; needs re-checking).
  * Incumbent entry pricing has risen steeply (e.g., AnyDesk Solo reportedly rose from $14.90 in 2024 to $28.90/mo in 2026 on Splashtop's comparison page, though Capterra lists €22.90/mo—unverified, needs re-checking).
  * IT admins and device owners lack an auditable middle ground where central policies exist without stripping owners of informed, per-permission veto power.
* **Unique Value Proposition**
  * *Nothing starts without a yes, and every session is on the record*. End-to-end encrypted remote access engineered so a stranger cannot easily talk someone into handing over their machine, with monotonic policies (owners can only make rules stricter, never looser) and a signed audit trail.
* **Channels**
  * Trust-first technical content (public threat model, plain-language security architecture, third-party pen-test summary).
  * Competitor switching guides targeting incumbent price hikes and scam concerns.
  * Viral "person being helped" loop: every 15-minute one-time support link exposes a verified organisation's client to the consent UX (assumption).
  * IT/sysadmin communities (Reddit r/sysadmin, Hacker News, Spiceworks, MacAdmins Slack) and later bank/telco/insurer fraud-prevention partnerships.
* **Revenue Streams**
  * Paid supporter seats or per-managed-device SaaS subscriptions; host/recipient side remains free (assumption).
  * V1/Enterprise add-ons: extended WORM audit retention, SIEM export, SSO/SCIM, self-hosted relay, regional data residency, and MSP multi-tenant branding.
* **Cost Structure**
  * *Variable:* Regional TURN relay bandwidth (1–3 GB/hr estimate for 1080p relayed streams), control-plane compute, metadata audit storage, payment processing (assumption).
  * *Fixed:* Rust/cross-platform engineering, OS compatibility maintenance (macOS permissions, Wayland portals, Windows secure desktop), annual pen tests, SOC 2 Type II (target 12–18 months), code signing/notarisation, and 24-hour abuse triage.
* **Unfair Advantage (Moat)**
  * Host-enforced cryptographic consent architecture (handshake-derived 6-digit SAS code, non-clickable privileged consent UI, monotonic policy engine, hash-chained logs) paired with an earned trust reputation that incumbents cannot retrofit without breaking legacy workflows.

---

## 2. Recommended First Segment

**Target Beachhead: Small Businesses (15–100 employees) with lean internal IT/ops and sensitive or mixed fleets**—specifically boutique accounting/bookkeeping firms, legal offices, private clinics, engineering labs, and design/media shops (assumption).

* **Direct MVP fit:** Your prototype's sample inventory (*Reception-PC, Design-Mac-02, Lab-Server-01, Studio-Mini, Warehouse-Kiosk*) already reflects this exact environment. These buyers need your MVP today (Windows/macOS agents, asks-first + unattended modes, monotonic policies, CSV audit export) without waiting for V1/Later features like multi-tenant MSP consoles, RMM/PSA integrations, or formal HIPAA BAAs.
* **Immediate switching trigger:** Incumbent price hikes—with Splashtop Solo starting near $6/mo billed annually, TeamViewer near $24.90/mo (dupple.com, Oct 2026; needs re-checking), and AnyDesk Standard reportedly up to $49.90/mo (Splashtop comparison page; unconfirmed, needs re-checking)—push cost-sensitive SMBs to look for alternatives.
* **Consent as a revenue-side differentiator:** Small firms routinely troubleshoot both internal laptops and external client machines. Showing a **verified organisation badge** and plain-language consent card via a 15-minute support link makes their IT/support team look more trustworthy to clients (assumption).
* **Fastest learning and cash cycle:** Consumer helpers have near-zero willingness to pay and disproportionate scam-liability risk; regulated enterprises require SOC 2 Type II before procurement (12–18 months away); MSPs require heavy partner tooling (assumption). SMBs close via self-serve or 20-minute founder calls in under 30 days (assumption).

---

## 3. Pricing Structures & Test Hypotheses

**Who pays:** **Free for the person being helped; paid for the supporter/organisation** (assumption). Charging the host creates fatal friction during support moments and undermines adoption of the 15-minute one-time support link.

| Structure | Free Tier (Personal / Host) | Paid Tier 1 (Starter) | Paid Tier 2 (Business) | Primary Test Hypothesis (assumptions) |
| :--- | :--- | :--- | :--- | :--- |
| **1. Per-Supporter Seat** *(Recommended to test first)* | **$0** — Unlimited incoming help (receiving); 1 supporter seat; up to 3 asks-first devices; 7-day log; 5 relayed hrs/mo cap (assumption). | **Pro: $15/supporter/mo** — Up to 25 devices; verified org badge; 15-min one-time links; 90-day audit log + CSV export (assumption). | **Team: $29/supporter/mo** — Up to 100 devices/seat; unattended mode + MFA; org policies; 1-yr audit log; SSO/SIEM add-on in V1 (assumption). | SMBs with 1–3 IT staff prefer predictable per-tech pricing anchored below TeamViewer (~$24.90/mo, dupple.com; needs re-checking) and convert faster when adding devices has zero marginal friction. |
| **2. Per-Managed-Device** | **$0** — Unlimited incoming help; up to 3 asks-first devices; 1 user; no unattended mode (assumption). | **Fleet: $2.50/device/mo** *(5-device min = $12.50/mo)* — Unlimited named supporters; asks-first + unattended; 90-day log + CSV (assumption). | **Compliance: $5/device/mo** — 3-yr tamper-evident WORM log; granular policies; step-up MFA; regional residency add-on (assumption). | Fleet-heavy SMBs (kiosks, reception PCs, labs) with several occasional helpers prefer paying per endpoint so multiple staff can help without seat-sharing workarounds. |
| **3. Org Tier + Relay Pack** | **$0** — Up to 3 asks-first devices; P2P unlimited; 5 relayed hrs/mo hard cap (assumption). | **Small Org: $19/org/mo** — 3 supporters, 15 devices, 50 relayed hrs/mo; $0.25/hr overage (assumption). | **Growth Org: $49/org/mo** — 10 supporters, 50 devices, 200 relayed hrs/mo; self-hosted relay option in V1 (assumption). | Bundled org pricing simplifies SMB budgeting while explicitly capping worst-case relay margin exposure on strict corporate NATs. |

---

## 4. Free-Tier Abuse Controls

Scammers abuse free tiers first. Every control below assumes the free tier is strictly **asks-first only** (no unattended access):

* **Identity & verification gates:** Require passkey/MFA plus a verified phone number or unique payment-method authorization ($0 hold) before a free account can *initiate* outbound sessions (receiving help requires no friction) (assumption). Display "Unverified Personal Account" prominently on host prompts so free users can never impersonate a bank or IT vendor.
* **Request & rate limits:** Cap free requesters at 3 pending requests per hour, 10 unique target devices per month, and an automatic 24-hour lockout after 3 host denials or 2 code mismatches (assumption).
* **Outside-organisation caps:** Block free accounts from generating 15-minute one-time support links (reserve those for domain- and business-verified paid organisations) (assumption). Require first-time personal requesters to pass a mandatory 5-second cooling-off timer and the *"Did someone call or message you asking for this?"* warning.
* **Bandwidth & session caps:** Default to direct P2P; enforce a 5 relayed hours/month cap and 60-minute max session length on free accounts, with file transfer disabled on free tiers by default (assumption).

---

## 5. Unit Economics

All figures below are `(assumption)` except the 1–3 GB/hr relayed bandwidth estimate and the 10–15% infra/relay target taken from the brief. Egress pricing varies heavily by provider (e.g., hyperscalers vs. bare-metal VPS relays) and must be verified against live vendor quotes before fundraising.

### Core Assumptions
* **ARPD (Average Revenue Per Paid Device):** **$3.00/device/mo** (e.g., $15/mo Pro seat averaging 5 active devices, or $3/device/mo fleet pricing) (assumption).
* **Usage:** 6 session-hours/device/mo (assumption).
* **Relay fallback share:** 25% of hours relayed; 75% direct P2P (assumption).
* **Relayed bandwidth:** 2.0 GB/hr midpoint (from 1–3 GB/hr estimate) = 3.0 GB relayed/device/mo.
* **Blended relay cost:** **$0.05/GB** across regional TURN nodes (assumption — verify current AWS/Cloudflare/Hetzner/OVH quotes).

### Unit Economics & Break-Even Table

| Metric | Formula / Basis | Base Case (assumption) | High-Relay Stress Case (60% relay, 3 GB/hr @ $0.08/GB) |
| :--- | :--- | :--- | :--- |
| **Relay cost per relayed session-hour** | $2.0\text{ GB} \times \$0.05/\text{GB}$ | **$0.10 / relayed hr** | $0.24 / relayed hr |
| **Blended relay cost per total session-hour** | $\text{Relayed hr cost} \times \text{Relay \%}$ | **$0.025 / session-hr** | $0.144 / session-hr |
| **Monthly relay cost per paid device** | $6\text{ hrs} \times \text{Blended hr cost}$ | **$0.15 / device / mo** | $0.86 / device / mo |
| **Control plane + metadata audit storage** | Signaling, DB, hash-chained logs | **$0.10 / device / mo** | $0.10 / device / mo |
| **Infra + relay % of revenue** *(Target: <10–15%)* | $(\$0.15 + \$0.10) / \$3.00$ | **8.3%** *(Passes target)* | 32.0% *(Breaches target)* |
| **Payment processing + abuse/support COGS** | ~3.3% fees ($0.10) + support ($0.10) | **$0.20 / device / mo** | $0.20 / device / mo |
| **Total Variable COGS per paid device** | Sum of variable costs | **$0.45 / device / mo** | $1.16 / device / mo |
| **Gross Profit per paid device** | $\$3.00 - \text{Variable COGS}$ | **$2.55 / device / mo** | $1.84 / device / mo |
| **Gross Margin** *(Target: 80–85%)* | $\text{Gross Profit} / \$3.00$ | **85.0%** | 61.3% |
| **Break-Even Devices — Bootstrapped** *($12,000/mo fixed burn)* | $\$12,000 / \text{Gross Profit}$ | **4,706 paid devices** (~470 SMBs @ 10 devices) | 6,522 paid devices |
| **Break-Even Devices — Seed Team** *($40,000/mo fixed burn: 4 staff + pen test/SOC 2 amortized)* | $\$40,000 / \text{Gross Profit}$ | **15,687 paid devices** (~1,570 SMBs @ 10 devices) | 21,739 paid devices |

The stress case shows that P2P hole-punching success and low-cost bare-metal TURN relays are business-critical: letting relay share climb above ~40% on hyperscaler egress will break the 15% infra budget (assumption).

---

## 6. First 90 Days GTM, Risks & Metrics

### 90-Day Go-to-Market Plan (assumptions)
* **Days 1–30 (Validation & Trust Foundation):**
  * Recruit 15–20 design-partner SMBs (accounting, clinics, design labs) from founder network and r/sysadmin/MacAdmins.
  * Run 10-second comprehension tests on the consent prompt with non-technical staff and older adults.
  * Publish the public Threat Model, plain-language Security Architecture page, and "Why Owners Can Only Tighten" explainer.
  * Complete pre-launch external penetration test and fix findings.
* **Days 31–60 (Private Beta & Pricing Experiments):**
  * Ship signed Windows/macOS installers with guided macOS Screen Recording/Accessibility onboarding.
  * A/B test Structure 1 (Per-Supporter) vs. Structure 2 (Per-Device) on landing pages and with design partners.
  * Publish 2 high-intent switching guides (*"Switching from AnyDesk after the price hikes"*, *"Why your remote support tool shouldn't look like a scammer's favorite app"*).
* **Days 61–90 (Public Beta & Conversion):**
  * Launch on Hacker News ("Show HN: Consent-first E2EE remote desktop with handshake-derived 6-digit codes") and Product Hunt.
  * Activate the 15-minute verified-org support link to trigger organic B2B2C discovery.
  * Convert at least 10 design partners to paid annual/monthly plans and instrument relay-ratio telemetry per NAT type.

### Top 5 Risks & Mitigations
1. **Abuse & reputational damage (scammers adopt your app):** Never claim "scam-proof". Enforce verified org badges, personal-tier friction/cooling-off timers, abuse reporting with <24 h SLA, and proactive takedown contacts for banks and CERTs.
2. **OS permission friction (macOS resets, Linux Wayland portals, Windows UAC):** Build a guided permission wizard, clear *Screen unavailable* remediation cards, and automated OS-beta CI regression tests.
3. **Relay bandwidth margin squeeze:** Invest early in ICE/STUN/QUIC NAT traversal to keep P2P >75%, run regional TURN relays on bandwidth-efficient bare-metal providers, and cap free-tier relayed hours (assumption).
4. **Over-claiming security or a consent-UI bypass:** Model-check the consent state machine, run synthetic-input detection tests on protected surfaces, and publish independent pen-test results rather than marketing guarantees.
5. **Incumbent price cuts or feature copying:** Anchor differentiation in *architectural* trust (host-enforced signed grants, zero-knowledge relays, monotonic policy engine) and clean plain-language UX rather than competing solely on price.

### 6 Core North-Star & Health Metrics
1. **Time to First Consented Session (TTFCS):** Target <5 minutes from install to completed handshake (assumption).
2. **Session Success Rate:** % of approved requests reaching first frame within 3 s (p95) without dropping.
3. **Host Deny & Code-Mismatch Rate:** Separates normal UX friction from potential abuse or social-engineering spikes.
4. **Input-to-Pixel Latency (p95) & P2P Share:** Target p50 ≤80 ms, p95 ≤150 ms, and ≥75% direct P2P (assumption).
5. **Paid Devices & Net Revenue Retention (NRR):** Measures SMB fleet expansion from initial supporter seat to company-wide deployment.
6. **Abuse Reports per 1,000 Sessions:** Primary trust-and-safety guardrail (target <0.5/1,000; assumption).

---

# B. Brand and Logo

## 1. Name Candidates

*Note: Trademark clearances (USPTO/EUIPO/WIPO Class 9 & 42), exact-match domain availability, and Apple/Google/Microsoft app-store name checks have NOT been run and are required before committing to any name.*

| Candidate | Origin | Rationale & Positioning Fit |
| :--- | :--- | :--- |
| **Knock** | Brief | Instantly understood etiquette ("knock before coming in"). Warm, human, verb-ready (*"Knock on Dana's Mac"*), zero surveillance vibe. |
| **Vouch** | Brief | Highlights verified organisation identity and auditable accountability. Strongest enterprise/SMB compliance resonance. |
| **Nod** | Brief | Crisp, approachable shorthand for informed consent ("get the nod first"). Works equally well for SMBs and families. |
| **Ajar** | Brief | Evokes opening access *only as much as needed* (screen yes, files no) and closing it in one click. Distinctive, memorable. |
| **Doorstep** | Brief | Concrete, non-technical physical metaphor for pausing at the threshold before letting anyone inside. High consumer clarity. |
| **Latch** | New | A safeguard operated exclusively from the *inside* of the door—directly mirrors host-enforced permissions and the owner's absolute veto. |
| **Sayso** | New | Plain-spoken English for authority (*"Nothing starts without your say-so"*). Warm, unmistakable, hard to confuse with legacy remote tools. |
| **Porch** | New | The well-lit, visible entry space where you verify who is visiting before inviting them in. Calm, human, anti-hacker aesthetic. |

**Shortlist recommendation (assumption):** Test **Knock** (most intuitive consumer/SMB verb), **Latch** (sharpest security-without-fear metaphor), and **Vouch** (strongest B2B/audit authority).

---

## 2. Brand Essence, Personality & Voice

* **Brand Essence:** *Ask first, then help.* Calm, human, transparent.
* **Brand Personality:** Trustworthy, composed, plain-spoken, precise, warm. Feels like a well-lit clinic or a conscientious architect—never a dark-mode hacker utility, cyber-warfare shield, or silent IT surveillance agent.
* **Voice Principles:**
  * **State what happens next in plain English:** Say *"Waiting for Dana to allow access (1:42 left)"* rather than *"Awaiting RPC handshake acknowledgment"*.
  * **Put the owner in the subject position:** Say *"You can end this session at any time"* rather than *"Session termination is enabled"*.
  * **Be candid about limits:** Say *"Built so a stranger can't rush you into sharing your screen"*—never claim *"100% scam-proof"*.
  * **Keep technical proof accessible, not intimidating:** Show *"Encrypted end to end"* and *"Device key matches saved key"* cleanly; put cryptographic details (Noise/DTLS, hash-chaining) one click away for IT buyers.

---

## 3. Four Logo Concepts

All concepts are built on a `64×64` coordinate grid (multiples of 4 align cleanly to whole pixels at `16×16`), avoiding monitor+arrow and padlock-only clichés.

### Concept 1: Threshold Arch (Keyhole Screen)
A rounded-square screen frame opened at the base by a calm doorway arch, with a mint consent pulse at the threshold—representing *the pause before entry*.

**Primary Colour SVG**
```xml
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64" width="64" height="64" role="img" aria-label="Threshold Arch Logo">
  <path fill="#0F7B6C" d="M20 8H44C50.627 8 56 13.373 56 20V44C56 50.627 50.627 56 44 56H42V36C42 30.477 37.523 26 32 26C26.477 26 22 30.477 22 36V56H20C13.373 56 8 50.627 8 44V20C8 13.373 13.373 8 20 8Z"/>
  <circle cx="32" cy="40" r="5" fill="#5FD9BF"/>
</svg>
```

**One-Colour SVG (`currentColor`)**
```xml
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64" width="64" height="64" role="img" aria-label="Threshold Arch Logo Monochrome">
  <path fill="currentColor" d="M20 8H44C50.627 8 56 13.373 56 20V44C56 50.627 50.627 56 44 56H42V36C42 30.477 37.523 26 32 26C26.477 26 22 30.477 22 36V56H20C13.373 56 8 50.627 8 44V20C8 13.373 13.373 8 20 8Z"/>
  <circle cx="32" cy="40" r="5" fill="currentColor"/>
</svg>
```

* **16 px Legibility Check:** Outer silhouette scales to `12×12 px` with `3.5 px` side pillars and a `5 px`-wide arch opening. The center dot scales to `2.5 px` diameter with `1.25 px` clearance on both sides—clean and readable, though for the `16 px` favicon the dot can be nudged to `r="4.5"` for extra breathing room.

---

### Concept 2: Nod (Overlapping Squares Forming a Checkmark)
Two overlapping rounded screens (requester + host) uniting around a single affirmative checkmark—representing *mutual agreement before connection*.

**Primary Colour SVG**
```xml
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64" width="64" height="64" role="img" aria-label="Nod Overlap Logo">
  <rect x="20" y="8" width="36" height="36" rx="10" fill="#074A40"/>
  <rect x="8" y="20" width="36" height="36" rx="10" fill="#0F7B6C"/>
  <path d="M19 37L27 45L45 25" fill="none" stroke="#5FD9BF" stroke-width="6" stroke-linecap="round" stroke-linejoin="round"/>
</svg>
```

**One-Colour SVG (`currentColor`)**
```xml
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64" width="64" height="64" role="img" aria-label="Nod Overlap Logo Monochrome">
  <path fill="none" stroke="currentColor" stroke-width="5" stroke-linecap="round" stroke-linejoin="round" d="M22 18V16C22 11.582 25.582 8 30 8H46C50.418 8 54 11.582 54 16V32C54 36.418 50.418 40 46 40H44"/>
  <rect x="10" y="22" width="34" height="34" rx="9" fill="none" stroke="currentColor" stroke-width="5"/>
  <path d="M19 39L25 45L36 32" fill="none" stroke="currentColor" stroke-width="5" stroke-linecap="round" stroke-linejoin="round"/>
</svg>
```

* **16 px Legibility Check:** In full colour, the mint checkmark (`1.5 px` stroke at `16 px`) pops clearly against the two teal planes. In `16 px` monochrome, two nested strokes plus a checkmark risk visual crowding—simplify the `16 px` monochrome favicon by removing the back-square outline and keeping only the single rounded square + bold checkmark.

---

### Concept 3: Six-Dot Code
Six dots in a 3×2 matrix directly echoing the handshake-derived 6-digit verification code. One dot is differentiated in mint (and rendered as a hollow ring in monochrome so meaning never relies on colour alone).

**Primary Colour SVG**
```xml
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64" width="64" height="64" role="img" aria-label="Six-Dot Code Logo">
  <circle cx="12" cy="22" r="6" fill="#0F7B6C"/>
  <circle cx="32" cy="22" r="6" fill="#0F7B6C"/>
  <circle cx="52" cy="22" r="6" fill="#0F7B6C"/>
  <circle cx="12" cy="42" r="6" fill="#0F7B6C"/>
  <circle cx="32" cy="42" r="6" fill="#0F7B6C"/>
  <circle cx="52" cy="42" r="6" fill="#5FD9BF"/>
</svg>
```

**One-Colour SVG (`currentColor`)**
```xml
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64" width="64" height="64" role="img" aria-label="Six-Dot Code Logo Monochrome">
  <circle cx="12" cy="22" r="6" fill="currentColor"/>
  <circle cx="32" cy="22" r="6" fill="currentColor"/>
  <circle cx="52" cy="22" r="6" fill="currentColor"/>
  <circle cx="12" cy="42" r="6" fill="currentColor"/>
  <circle cx="32" cy="42" r="6" fill="currentColor"/>
  <circle cx="52" cy="42" r="4.5" fill="none" stroke="currentColor" stroke-width="3"/>
</svg>
```

* **16 px Legibility Check:** Each dot scales to `3×3 px` with a `2 px` gap between dots—near-ideal geometric clarity at `16 px`. Note: in `16 px` monochrome, the hollow ring (`0.75 px` stroke) may soften on 1x displays; either use a `4 px` solid diamond for the 6th position at `16 px` or enclose the 6 dots inside a soft dark app-icon tile on OS docks.

---

### Concept 4: Facing Brackets with Dot *(Recommended for Tray + App Icon Versatility)*
Two balanced rounded brackets forming an open screen boundary around a central mint dot (the owner's active yes). Scales effortlessly into menu-bar state icons.

**Primary Colour SVG**
```xml
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64" width="64" height="64" role="img" aria-label="Facing Brackets Logo">
  <path d="M22 12H16C11.582 12 8 15.582 8 20V44C8 48.418 11.582 52 16 52H22" fill="none" stroke="#0F7B6C" stroke-width="6" stroke-linecap="round"/>
  <path d="M42 12H48C52.418 12 56 15.582 56 20V44C56 48.418 52.418 52 48 52H42" fill="none" stroke="#0F7B6C" stroke-width="6" stroke-linecap="round"/>
  <circle cx="32" cy="32" r="6.5" fill="#5FD9BF"/>
</svg>
```

**One-Colour SVG (`currentColor`)**
```xml
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64" width="64" height="64" role="img" aria-label="Facing Brackets Logo Monochrome">
  <path d="M22 12H16C11.582 12 8 15.582 8 20V44C8 48.418 11.582 52 16 52H22" fill="none" stroke="currentColor" stroke-width="6" stroke-linecap="round"/>
  <path d="M42 12H48C52.418 12 56 15.582 56 20V44C56 48.418 52.418 52 48 52H42" fill="none" stroke="currentColor" stroke-width="6" stroke-linecap="round"/>
  <circle cx="32" cy="32" r="6.5" fill="currentColor"/>
</svg>
```

* **16 px Legibility Check:** Strongest `16 px` performer of the four. Bracket strokes render at `1.5 px`, center dot at `3.25 px` diameter, with generous negative space (`>1.5 px` clear air around the dot). Works identically on light surfaces, `#0E1214` reversed dark chrome, and monochrome OS trays.

**Lockup Rule (applies to all concepts):** Set wordmark in **Space Grotesk SemiBold (-0.02em tracking)** in `#14181B` on `#F5F6F4` (or `#F5F6F4` on `#0E1214` dark session chrome), with clearspace equal to `0.5×` mark height.

---

## 4. Palette & Typography System

### Colour Tokens & Accessibility Guardrails
* **Primary Teal `#0F7B6C`** (Hover `#0A6457`, Darkest `#074A40`): Primary actions, active toggles, logo base. Passes WCAG 2.2 AA contrast on `#F5F6F4` and `#FFFFFF`.
* **Mint Accent `#5FD9BF`**: Live/encrypted indicator on dark chrome (`#0E1214`) and logo highlight. *Accessibility rule:* Never use `#5FD9BF` for text or thin icons on light surfaces (`#F5F6F4`); pair it exclusively with `#0E1214` or `#074A40`.
* **Pale Teal Tint `#E3F1EE`**: Verified org chip background, selected filter pills, info callouts.
* **Ink & Neutrals:** Primary ink `#14181B`, secondary text `#4A524E`, muted labels `#8A938E` (use only for large/decorative elements or darken to `#5C6560` for AA small text; assumption), borders `#DADDD8`, page surface `#F5F6F4`, dark session chrome `#0E1214`.
* **Semantic Status (additions, assumptions):** Deny / End Session crimson `#B42318` (hover `#912018`, tint `#FEE4E2`); Warning amber `#B54708` (tint `#FEF0C7`) for first-time requester interstitials and key-changed alerts.

### Typography Hierarchy
* **Space Grotesk (500–600):** Wordmark, marketing headlines, modal titles (*"Sam T. wants to connect"*).
* **Inter (400–600):** All UI controls, consent checkboxes, policy descriptions, audit table body. Minimum `14 px` on consent cards for 10-second readability.
* **JetBrains Mono (500–600):** 9-digit device IDs (`482 190 734`), 6-digit handshake codes (`739 204`), latency readouts (`38 ms`), UTC audit timestamps.

---

## 5. Tray Icon States & On-Screen Session Banner Badge

### Monochrome Tray / Menu-Bar States (Colour-Independent)
Built on **Concept 4 (Facing Brackets)** so each state changes *silhouette*, passing WCAG 2.2 AA and macOS/Windows monochrome template-icon rules:

1. **Idle (Ready, no active session):** Open facing brackets with an **empty center**—clearly signals nobody is connected.
```xml
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="24" height="24" role="img" aria-label="Tray Idle">
  <path d="M8.5 5H6.5C5.119 5 4 6.119 4 7.5V16.5C4 17.881 5.119 19 6.5 19H8.5" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round"/>
  <path d="M15.5 5H17.5C18.881 5 20 6.119 20 7.5V16.5C20 17.881 18.881 19 17.5 19H15.5" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round"/>
</svg>
```

2. **In Session (Active remote connection):** Facing brackets enclosing a **solid filled center dot**—unmistakably "occupied" even in pure black/white.
```xml
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="24" height="24" role="img" aria-label="Tray In Session">
  <path d="M8.5 5H6.5C5.119 5 4 6.119 4 7.5V16.5C4 17.881 5.119 19 6.5 19H8.5" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round"/>
  <path d="M15.5 5H17.5C18.881 5 20 6.119 20 7.5V16.5C20 17.881 18.881 19 17.5 19H15.5" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round"/>
  <circle cx="12" cy="12" r="3.4" fill="currentColor"/>
</svg>
```

3. **Alert (Pending consent prompt / key mismatch / warning):** Facing brackets with a **bold exclamation mark (`!`) in the center** and a top-right corner diamond—distinct silhouette from both Idle and In Session.
```xml
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="24" height="24" role="img" aria-label="Tray Alert">
  <path d="M8.5 5H6.5C5.119 5 4 6.119 4 7.5V16.5C4 17.881 5.119 19 6.5 19H8.5" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round"/>
  <path d="M15.5 19H17.5C18.881 19 20 17.881 20 16.5V11" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round"/>
  <line x1="12" y1="8" x2="12" y2="13.5" stroke="currentColor" stroke-width="2.2" stroke-linecap="round"/>
  <circle cx="12" cy="16.6" r="1.2" fill="currentColor"/>
  <polygon points="18,2 22,6 18,10 14,6" fill="currentColor"/>
</svg>
```

### Persistent On-Screen Session Banner Badge
Rendered by the privileged host agent on a protected top-most surface that remote input cannot click, hide, or blank:

```xml
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 460 44" width="460" height="44" role="img" aria-label="Active Session Banner">
  <rect x="1" y="1" width="458" height="42" rx="21" fill="#0E1214" stroke="#5FD9BF" stroke-width="1.5"/>
  <!-- Brand mark + LIVE pill -->
  <path d="M23 14H20C18.343 14 17 15.343 17 17V27C17 28.657 18.343 30 20 30H23" fill="none" stroke="#5FD9BF" stroke-width="2.2" stroke-linecap="round"/>
  <path d="M31 14H34C35.657 14 37 15.343 37 17V27C37 28.657 35.657 30 34 30H31" fill="none" stroke="#5FD9BF" stroke-width="2.2" stroke-linecap="round"/>
  <circle cx="27" cy="22" r="3" fill="#5FD9BF"/>
  <!-- Text: Verified identity + active scope -->
  <text x="48" y="19" fill="#F5F6F4" font-family="Inter, sans-serif" font-size="12" font-weight="600">LIVE · Sam T. (Acme IT · Verified)</text>
  <text x="48" y="33" fill="#5FD9BF" font-family="Inter, sans-serif" font-size="10.5" font-weight="500">Screen + Input · Clipboard off · Files off</text>
  <!-- High-contrast End Session button -->
  <rect x="352" y="8" width="98" height="28" rx="14" fill="#B42318"/>
  <text x="401" y="26" text-anchor="middle" fill="#FFFFFF" font-family="Inter, sans-serif" font-size="12" font-weight="600">End session</text>
</svg>
```

* **Behavioural spec:** Always displays explicit text (`LIVE`), server-verified viewer name + organisation, active permission summary (never colour-only), and a local-input-only **End session** button that disconnects immediately even on degraded links.

---

# C. 10 Decisions You Must Make Next

*(Ordered by impact)*

1. **Lock the first customer segment (SMB internal IT vs. Consumer helpers vs. MSPs):** This single choice dictates your onboarding flow, pricing model, compliance timeline, and whether your brand voice leans warm-everyday (*Knock*) or audit-ready B2B (*Vouch*).
2. **Choose the agent licensing & open-source strategy:** Decide now whether the host agent is closed-source (protecting commercial moat) or source-available/open-source (maximising security auditability without AGPL copyleft contamination) before writing production code.
3. **Finalise the product name after legal clearance:** Run trademark searches (Class 9 & 42), domain checks, and app-store checks on your top 3 candidates (*Knock, Latch, Vouch*) before registering code-signing certificates and installer bundle IDs.
4. **Decide free-tier boundaries and anti-abuse friction:** Confirm whether free users can initiate outbound sessions with a verified phone/card check or whether the free tier is strictly *receive-help-only* plus 3 personal asks-first devices (assumption).
5. **Validate pricing metric (Per-Supporter vs. Per-Device) with 15 target buyers:** Run willingness-to-pay interviews with SMB IT leads to verify whether $15/supporter/mo or $2.50/device/mo maximizes ACV and minimizes seat-sharing friction.
6. **Commit to the protected consent-UI architecture per OS:** Decide whether the host consent prompt is a custom privileged card (matching the 520×660 prototype) or OS-native secure surfaces on Windows (Secure Desktop), macOS, and Linux, and validate synthetic-input isolation early.
7. **Define "Unattended mode" and "Until I turn it off" guardrails:** Set hard limits on who can mark a device unattended, require step-up MFA for unattended entry, and define maximum grant durations + reminder cadence so open-ended grants don't undermine your consent-first promise.
8. **Specify the missing MVP states and screens:** Close the prototype gaps before engineering sprint 1—specifically *Add device / macOS permission onboarding*, *Sessions tab*, *Ask to join* (single vs. multi-viewer), *Key-changed recovery*, and *Deny/Expiry/Offline* error flows.
9. **Select TURN relay hosting strategy and region footprint:** Benchmark P2P hole-punching rates and get binding bandwidth quotes from non-hyperscaler TURN providers to keep infra + relay costs under your 10–15% revenue ceiling.
10. **Finalize logo direction and 16 px tray-state iconography:** Pick between **Concept 4 (Facing Brackets)** and **Concept 1 (Threshold Arch)**, then test the three monochrome tray states and the persistent session banner with non-technical users alongside the 10-second consent-prompt comprehension test.