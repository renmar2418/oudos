# Oudos Design System (`DESIGN.md`)

Oudos (*Ancient Greek: οὐδός — "threshold / doorstep"*) uses a calm, clinical, high-contrast security UI built around one rule: **Ask first, then help.** Every screen—from the `520x660` Windows host consent card to the self-hosted web console—must be readable by a non-technical device owner under time pressure in **10 seconds or less**.

---

## 1. Brand Personality & Copy Rules

- **Tone:** Restrained, transparent, and plain-English. Never mimic dark "hacker" tools, noisy consumer dashboards, or silent surveillance agents.
- **Zero Jargon on Host Surfaces:** Never write *"Awaiting RPC handshake acknowledgment"* or *"Asks-First v0.1"* on user-facing host cards. State what happens next plainly:
  - `"Nothing is shared until you click Allow. You can end this session at any time."`
  - `"Waiting for Dana to allow access (1:42 left)"`
- **Honest Security Language:** Never claim Oudos is `"100% scam-proof"` or `"unhackable"`. Describe the concrete mechanism instead (6-digit code match, 5-second safety wait, screen-only default, host-enforced permissions).

---

## 2. Brand Mark: Facing Brackets (`32x32`)

The Oudos mark is a monochrome geometric threshold rendered on a canonical `32x32` viewBox (`stroke="currentColor"`, `strokeWidth="2.5"`, `strokeLinecap="round"`, `strokeLinejoin="round"`).

All host tray icons and header marks MUST distinguish state by **shape and text, never colour alone** (WCAG 2.2 AA):

| State | Geometry (`32x32` grid) | Colour Usage |
| :--- | :--- | :--- |
| **Idle** | Open facing brackets (`M11 7H7.5...` / `M21 7H24.5...`) with an **empty center** | Light header: `#0F5C52` · Windows tray: monochrome `#F5F6F4` |
| **In Session / Live** | Facing brackets enclosing a **solid filled center dot** (`cx="16" cy="16" r="3"`) | Light header: `#0F5C52` · Obsidian live banner & dark chrome: `#5FD9BF` · Windows tray: monochrome `#F5F6F4` |
| **Alert** | Facing brackets with **center `!`** (`M16 11V17`, dot `cy="20.5"`) + **top-right diamond** (`M27 2L30 5L27 8L24 5L27 2Z`) | Host header: `#B42318` · Web PWA header: `#B42318` brackets + `#B54708` diamond · Windows tray: monochrome `#F5F6F4` |

- **PWA App Tile (`apps/web-pwa/public/icons/icon.svg`):** `64x64` rounded tile (`rx="14"`), white facing brackets (`strokeWidth="4.5"`), and `#5FD9BF` center dot on dark teal background.
- **Wordmark Pairing:** `FacingBracketsIcon` beside `"Oudos"` set in `Space Grotesk` SemiBold (`600`) with `-0.02em` letter-spacing.

---

## 3. Exact Color Tokens

Tokens are defined in [`apps/web-pwa/src/index.css`](apps/web-pwa/src/index.css) and [`apps/host-desktop/src/index.css`](apps/host-desktop/src/index.css).

### Light Surfaces, Ink & Brand Slate-Teal

| Token / Role | CSS Variable | Hex | Usage |
| :--- | :--- | :--- | :--- |
| **Canvas** | `--color-oud-surface` | `#F5F6F4` | Page background, SAS verification well, segmented track, card footers |
| **Panels** | — | `#FFFFFF` | Elevated content cards, headers, table rows, consent body, modals |
| **Primary Ink** | `--color-oud-ink` | `#111618` | Headings, body copy, 6-digit SAS digits, active tab labels |
| **Secondary / Muted Ink** | `--color-oud-secondary` / `--color-oud-muted` | `#525D63` | Subtitles, metadata labels, helper copy, inactive nav tabs |
| **1px Hairline Border** | `--color-oud-border` | `#DADDD8` | Panel outlines, section dividers, table gridlines, input borders |
| **Primary Slate-Teal** | `--color-oud-teal` | `#0F5C52` | Primary CTAs (`Allow`, `Request access`, `Add device`), active tab underline, enrolled server domain text, brand mark |
| **Slate-Teal Hover** | `--color-oud-teal-hover` | `#0B473F` | Hover state for primary slate-teal buttons |
| **Slate-Teal Deep** | `--color-oud-teal-deep` | `#083630` | Focus rings on primary actions |
| **Pale Teal Tint** | `--color-oud-tint` | `#E6F2F0` | Active filter pills, subtle status chips (`::selection` uses `#D7ECE8` / `#111618`) |

### Obsidian Dark Session Surfaces (`ActiveSessionScreen` & `PersistentSessionBanner`)

Dark surfaces isolate active remote viewing from configuration UI so users immediately recognize when a live session is active.

| Role | CSS Variable / Token | Hex | Usage |
| :--- | :--- | :--- | :--- |
| **Obsidian Chrome** | `--color-oud-chrome` | `#0E1214` | Active Session page/header, Host `PersistentSessionBanner` background |
| **Stream Viewport Well** | — | `#090C0D` | Remote desktop stream frame container |
| **Secondary Toolbar** | — | `#141D20` | Shortcut bar (`Ctrl+Alt+Del`, `Win+L`, `Alt+Tab`), quality select background |
| **Control Keycap** | — | `#1C2327` | Shortcut buttons, remote cursor switch |
| **Dark Hairlines** | — | `#252F34`, `#2A3236`, `#2E383D` | Section dividers (`#252F34`), banner divider (`#2A3236`), button borders (`#2E383D`) |
| **Live Mint Accent** | `--color-oud-mint` | `#5FD9BF` | `LIVE` / `E2E Encrypted` indicator, latency (`36 ms`), banner border, active focus rings |
| **Dark Secondary Text** | — | `#D2DAD7`, `#A8B4B0`, `#9FB0AA` | Permission summaries, server domain, shortcut labels on `#0E1214` / `#141D20` |

> [!IMPORTANT]
> **Strict Mint Contrast Rule:** `#5FD9BF` is permitted **only** on obsidian dark surfaces (`#0E1214`, `#090C0D`, `#13181B`, `#141D20`). Never render `#5FD9BF` text, icons, or borders on `#F5F6F4` or `#FFFFFF` light surfaces. Automated Vitest and Playwright tests enforce this invariant.

### Semantic Warning & Destructive Status

| State | Token | Hex | Paired Background / Text |
| :--- | :--- | :--- | :--- |
| **Unverified / Anti-Scam Amber** | `--color-oud-amber` | `#B54708` | Background `#FEF0C7` (`--color-oud-amber-tint`), body text `#7A2E0E`, border `#B54708/35`; dark chrome warning uses `#FDB022` on `#2A1B0E` |
| **Deny / End Destructive Crimson** | `--color-oud-crimson` | `#B42318` | Solid CTA `#B42318` (hover `#912018` `--color-oud-crimson-hover`) with `#FFFFFF` text; `Deny` outline uses `#FEE4E2` (`--color-oud-crimson-tint`, hover `#FCD5D2`) + `#912018` text; dark banner `Stop input` uses `#2C1211` + `#FDA29B` text |

---

## 4. Typography

| Role | CSS Stack | Weights | Usage & Formatting |
| :--- | :--- | :--- | :--- |
| **Display & Headings** | `"Space Grotesk", "Inter", "Segoe UI Variable Display", ui-sans-serif, system-ui, sans-serif` | `500`, `600` | Wordmark (`-0.02em` tracking), consent heading (`19px–20px`, `-0.015em` tracking), page titles |
| **Body & Controls** | `"Inter", "Segoe UI Variable Text", "Segoe UI", ui-sans-serif, system-ui, sans-serif` | `400`–`600` | UI labels, switches, buttons, plain-English explanations (`13px–14px` minimum on consent card) |
| **Codes & Telemetry** | `"JetBrains Mono", "Cascadia Mono", ui-monospace, SFMono-Regular, monospace` | `500`, `600` | `.font-mono-code` with `font-variant-numeric: tabular-nums` for 6-digit SAS (`739 204`, `26px–30px`, `0.14em–0.16em` tracking), 9-digit Device IDs (`482 190 734`), server domains, latency (`36 ms`), and UTC timestamps |

---

## 5. Layout Discipline & Surfaces

- **1px Hairline Architecture:** Panels, rows, and toolbars are separated by crisp `1px` hairlines (`#DADDD8` light, `#252F34` dark) rather than heavy drop shadows, blurry cards, or nested gradients.
- **Restrained Radii:** Controls use `4px` (`rounded`) or `6px` (`rounded-md`); `shadow-2xs` is restricted to active segmented pill indicators and switch thumbs.
- **Host Windows (`apps/host-desktop`):**
  - **Consent Card (`consent`):** `520x660` logical CSS px (`alwaysOnTop`, `center`, `resizable: false`). Uses a locked flex column (`header` -> optional unverified banner -> identity header -> SAS block -> anti-scam interstitial -> `flex-1 overflow-y-auto` permissions/duration -> pinned `Deny` / `Allow` footer).
  - **Persistent Session Banner (`banner`):** `640x56` logical CSS px, borderless (`decorations: false`), `alwaysOnTop`, `closable: false`, positioned top-center (`y + 12`), `1px` `#5FD9BF` outline on `#0E1214`.
- **Consent Card Hierarchy (10-Second Comprehension):**
  1. Requester name + raw server domain (`support.example.org` in `#0F5C52`, or `Personal / Unverified` in `#B54708` + top amber bar).
  2. Large centered 6-digit SAS code on `#F5F6F4`.
  3. First-time requester amber box disabling `Allow` for **5 seconds** (`Allow (5s)`).
  4. Permission switches (`See your screen` required ON; `Mouse & keyboard`, `Clipboard`, `File transfer` OFF by default).
  5. Duration toggle (`This session` default, `1 hour` max in `v0.1`).
  6. Balanced 50/50 `Deny` and `Allow` buttons + `Block & report requester`.

---

## 6. Anti-Slop Guardrails

Contributors modifying Oudos UI or docs MUST avoid these patterns:

1. **No AI-Slop Aesthetics:** No indigo/violet gradients, glowing neon cards, floating glassmorphism, decorative blobs, or emoji glyphs used as UI icons. Use `Lucide` vector icons and `HostFacingBracketsIcon` / `FacingBracketsIcon` exclusively.
2. **No Prototype Bypass Controls:** Never add `"Simulate approval"` or `"Preview active session"` buttons to production views.
3. **No Hidden / Asymmetric Actions:** `Deny` and `Allow` carry equal visual footprint; `End session` remains visible and un-obscured on the top-most host banner throughout every session.
4. **No Unrequested Motion:** Respect `@media (prefers-reduced-motion: reduce)` (`transition-duration: 0.01ms !important; animation-duration: 0.01ms !important`).

---

## 7. Windows DPI & WCAG 2.2 AA Accessibility Rules

- **Windows DPI Scaling (`100%`, `125%`, `150%`):**
  - `ConsentPromptCard` constrains height to `h-screen max-h-[660px]` and puts permissions/duration inside `min-h-0 flex-1 overflow-y-auto`, keeping the `Deny` / `Allow` action footer pinned and clickable at higher Windows display scales.
  - `PersistentSessionBanner` enforces `whitespace-nowrap` on `LIVE ·`, permission summary, `Stop input`, and `End session`, while truncating long requester/domain strings (`min-w-0 truncate`) so termination controls never wrap off-screen.
  - `apps/host-desktop/src-tauri/src/lib.rs` listens for `WindowEvent::Moved`, `Resized`, and `ScaleFactorChanged`, refreshing physical Win32 `GetWindowRect` bounds inside `WindowsInputGuard` (`WH_MOUSE_LL` / `WH_KEYBOARD_LL`) so remote pointer blocking over consent/banner surfaces stays accurate across DPI changes.
- **WCAG 2.2 AA Compliance:**
  - Body and control text maintain `>= 4.5:1` contrast in both light and dark modes; meaningful icons/borders maintain `>= 3:1`.
  - Keyboard navigation includes a `"Skip to main content"` link, visible `2px` focus rings (`#0F5C52` / `#083630` / `#B42318` / `#5FD9BF`), and native button/switch semantics (`role="switch"`, `role="alert"`, `aria-live="polite"`).
  - Mobile web navigation/controls enforce `min-h-[44px]` touch targets on `< 640px` viewports.
  - Automated verification runs on every PR via Vitest + `axe-core` and Playwright Chromium (`wcag2a`, `wcag2aa`, `wcag22aa`).
