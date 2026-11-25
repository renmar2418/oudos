---
trigger: model_decision
description: "Apply whenever building or styling React components in apps/web-pwa, Tauri host prompts/banners/tray icons in apps/host-desktop, Capacitor mobile views, copywriting, or accessibility tests."
---

# UI, Design System & Consent UX Rules

## 1. Brand Personality & Voice
- **Essence:** *Ask first, then help.* Calm, human, transparent—like a well-lit clinic or conscientious architect, never a dark hacker tool or surveillance agent.
- **Plain-English Copy Rules:**
  - Always state what happens next plainly: *"Waiting for Dana to allow access (1:42 left)"* — never *"Awaiting RPC handshake acknowledgment."*
  - Put the device owner in control: *"You can end this session at any time."*
  - Never claim *"100% scam-proof"*.

## 2. Exact Design Tokens
- **Primary Teal:** `#0F7B6C` (Hover `#0A6457`, Deep `#074A40`) — primary buttons, active switches, brand mark.
- **Mint Accent:** `#5FD9BF` — live/encrypted status on dark chrome (`#0E1214`) only. **WCAG Rule:** Never render `#5FD9BF` text or thin icons on light surfaces (`#F5F6F4` or `#FFFFFF`).
- **Pale Teal Tint:** `#E3F1EE` — enrolled server domain chips, selected filter pills, info callouts.
- **Ink & Surfaces:** Primary ink `#14181B`, secondary text `#4A524E`, muted text `#5C6560` (use `#8A938E` only for non-text/large decorative elements to preserve WCAG 2.2 AA contrast), borders `#DADDD8`, light page surface `#F5F6F4`, dark session chrome `#0E1214`.
- **Semantic Status:** Deny / End Session crimson `#B42318` (hover `#912018`, tint `#FEE4E2`); Warning amber `#B54708` (tint `#FEF0C7`) for first-time requester anti-scam warnings and key-changed alerts.

## 3. Typography
- **Headings & Modal Titles:** `Space Grotesk` (500–600), `-0.02em` tracking on wordmarks.
- **UI Body & Controls:** `Inter` (400–600). Minimum `14px` body size on host consent cards for 10-second comprehension.
- **Codes & Telemetry:** `JetBrains Mono` (500–600) for 9-digit device IDs (`482 190 734`), 6-digit SAS codes (`739 204`), latency (`38 ms`), and UTC timestamps.

## 4. Consent Prompt (`520x660`) & Anti-Scam UX
Every host consent prompt MUST pass a 10-second comprehension test for non-technical users:
1. **Identity Header:** Requester name + raw enrolled server domain chip (e.g., `support.example.org`). If un-enrolled/personal, display an explicit amber `Personal / Unverified` warning.
2. **First-Time Anti-Scam Interstitial:** Disable `Allow` for **5 seconds** behind a clear amber box: *"STOP: Did someone call or message you about your bank, a refund, or a virus? Real banks will NEVER ask you to share your screen."*
3. **6-Digit SAS Match:** Large `JetBrains Mono` 6-digit code with plain instruction: *"Only allow if the caller's code matches this exact number."*
4. **Explicit Permission Toggles:** Screen view (required ON); Mouse & Keyboard (OFF by default); Clipboard (OFF by default); File transfer (OFF by default).
5. **Bounded Duration (`v0.1`):** `This session` (default) or `1 hour`.
6. **Balanced Actions:** Prominent `Deny` button beside `Allow`, plus visible 2-minute countdown timer.

## 5. Required Screens & States (`apps/web-pwa`)
- **Core 5 Views:** (1) Devices list (`Online` / `In session` / `Offline` + search + filters), (2) Requesting / Waiting state with 6-digit SAS & 2-min countdown, (3) Active Session dark viewer (`#0E1214`) with quality (`Auto`/`Sharp`/`Fast`), latency, `Send Shortcuts` toolbar, and `End`, (4) Monotonic Policies panel with admin-lock icons, (5) Audit Log with filters and CSV export.
- **Mandatory Edge States:** Add Device / Pairing wizard, Sessions tab, `Screen unavailable` OS permission remediation card, Owner Denied, Request Expired (2m), Code Mismatch, Device Key Changed (blocking alert), Host Went Offline mid-session, Session Ended summary.

## 6. Colour-Independent Iconography (Concept 4: Facing Brackets)
All tray icons and banners MUST convey state through **shape and text**, never colour alone (WCAG 2.2 AA):
- **Tray — Idle:** Open facing brackets with an **empty center** (nobody connected).
- **Tray — In Session:** Facing brackets enclosing a **solid filled center dot**.
- **Tray — Alert:** Facing brackets with a **center exclamation mark (`!`)** and top-right corner diamond.
- **Persistent On-Screen Session Banner:** Top-most non-clickable dark pill (`#0E1214`, `#5FD9BF` border) displaying `LIVE · <Requester> (<Server Domain>)`, active permission summary text (`Screen only` / `Screen + Input`), and a crimson local-input-only `End session` button (`#B42318`).