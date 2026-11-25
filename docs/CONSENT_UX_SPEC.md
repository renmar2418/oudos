# Oudos Consent & UI Specification (`v0.1`)

## 1. Brand Voice & Comprehension Target
- **Essence:** *Ask first, then help.*
- Plain-English copy with zero jargon (e.g., *"Waiting for Dana to allow access (1:42 left)"*, *"You can end this session at any time."*).
- Target: non-technical users understand the `520x660` host consent card in under 10 seconds.

## 2. Design Tokens
- **Primary Teal:** `#0F7B6C` (Hover `#0A6457`, Deep `#074A40`)
- **Mint Accent:** `#5FD9BF` (dark chrome `#0E1214` only; never on light `#F5F6F4`/`#FFFFFF`)
- **Pale Teal Tint:** `#E3F1EE`
- **Ink & Surfaces:** Ink `#14181B`, Secondary `#4A524E`, Muted `#5C6560`, Borders `#DADDD8`, Page Surface `#F5F6F4`, Session Chrome `#0E1214`
- **Semantic Status:** Deny/End Crimson `#B42318` (tint `#FEE4E2`); Warning Amber `#B54708` (tint `#FEF0C7`)
- **Fonts:** `Space Grotesk` (headings), `Inter` (min `14px` body on consent cards), `JetBrains Mono` (device IDs, 6-digit SAS, latency, UTC timestamps)

## 3. Host Consent Card (`520x660`) & Persistent Banner
1. **Identity Header:** Requester display name + raw enrolled server domain chip (or explicit amber `Personal / Unverified`).
2. **First-Time Anti-Scam Interstitial:** `Allow` disabled for **5 seconds** beneath amber warning: *"STOP: Did someone call or message you about your bank, a refund, or a virus? Real banks will NEVER ask you to share your screen."*
3. **6-Digit SAS Match:** Large `JetBrains Mono` code + *"Only allow if the caller's code matches this exact number."*
4. **Permissions:** Screen view (required ON); Mouse & Keyboard (OFF); Clipboard (OFF); File transfer (OFF).
5. **Duration (`v0.1`):** `This session` (default) or `1 hour`.
6. **Balanced Actions:** `Deny` beside `Allow` + 2-minute countdown.
7. **Persistent Banner:** Top-most dark pill (`#0E1214`, `#5FD9BF` border) showing `LIVE · <Requester> (<Server Domain>)`, permission summary, and local-input-only crimson `End session` button.

## 4. Colour-Independent Tray Icons (Facing Brackets)
- **Idle:** Open facing brackets, empty center.
- **In Session:** Facing brackets enclosing solid center dot.
- **Alert:** Facing brackets with center `!` and top-right diamond.
