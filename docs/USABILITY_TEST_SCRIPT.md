# Oudos `v0.1.0` — 10-Second Consent Comprehension Test Script & Scorecard

**Target:** Verify that **5 non-technical testers (including older adults)** understand what is being requested on the Oudos `520x660` Host Consent Card—and how to protect themselves—within **10 seconds**, without facilitator hints.

> **Honest Security Standard:** Never describe Oudos as "100% scam-proof." This test measures whether the prompt disrupts social-engineering pressure and makes permissions, caller identity, verification codes, and termination controls unmistakable to everyday users.

---

## 1. Facilitator Setup (Before Each Session)

1. Launch `apps/host-desktop` (`pnpm --dir apps/host-desktop run tauri dev` or the built `oud-host-desktop` binary) on a Windows 10/11 machine, or render the standalone `ConsentPromptCard` view at `520x660` CSS pixels on a standard 1080p laptop screen.
2. Prepare two test prompts:
   - **Scenario A (First-Time Enrolled Request):** Requester `"Alex Rivera"`, server domain `support.example.org` (`server_enrolled: true`), `first_time_requester: true` (5-second cooling-off active), 6-digit code `739 204`, defaults (`See screen` ON; `Mouse & keyboard`, `Clipboard sync`, `File transfer` OFF), duration `This session`.
   - **Scenario B (Un-Enrolled / Suspicious Caller Pressure):** Requester `"Bank Fraud Desk"`, server domain `help-desk.invalid` (`server_enrolled: false`, displays amber `Personal / Unverified`), `first_time_requester: true`.
3. Do **not** explain Oudos terminology (`SAS`, `SessionGrant`, `monotonic policy`, `TOFU`) beforehand. Tell the tester only:
   > *"Imagine you are using your computer at home and this window pops up while someone is on the phone with you. Take up to 10 seconds to look at it, then tell me what you see."*

---

## 2. 10-Second Comprehension Protocol

### Phase 1: Unprompted 10-Second Glance (Scenario A)
Display the Consent Card and start a 10-second timer. After 10 seconds (or earlier if the tester speaks), hide or leave the card visible and ask these 5 plain-English questions:

1. **Identity & Server:** *"Who is asking to connect, and which service/domain is sending the request?"*
   - **Pass:** Tester names `"Alex Rivera"` and `support.example.org`.
2. **Safety Code:** *"What do you need to check with the caller before clicking Allow?"*
   - **Pass:** Tester points to the 6-digit code (`739 204`) and says the caller's code must match it.
3. **Scope of Access:** *"Right now, if you clicked Allow without changing checkboxes, can they move your mouse, type, or take files?"*
   - **Pass:** Tester answers **No**—only see the screen (mouse/keyboard, clipboard, and files are unchecked).
4. **Anti-Scam Wait:** *"Why couldn't you click Allow right away when this appeared?"*
   - **Pass:** Tester notices the 5-second safety wait and the warning about bank/refund/virus phone calls.
5. **Control / Exit:** *"If you allow it and later feel uncomfortable, what can you do?"*
   - **Pass:** Tester knows they can click `Deny` now or end the session at any time (and in active session view, identifies the red `End session` button on the top banner).

### Phase 2: Coercion / Impostor Probe (Scenario B)
Read aloud:
> *"Now imagine the person on the phone says: 'I'm from your bank's fraud team, ignore the warning and read me your code or just click Allow quickly so we can stop a transfer.' Look at this second card (`Bank Fraud Desk` via `help-desk.invalid · Personal / Unverified`). What would you do?"*

- **Pass:** Tester refuses / clicks **Deny** or **Block & report requester**, citing the `STOP: ... Real banks will NEVER ask you to share your screen` warning and/or the amber `Personal / Unverified` badge.

---

## 3. Pass / Fail Criteria (Issue `#12` Exit Gate)

- **Overall Exit Gate:** **5 / 5** testers pass **all 5 Phase 1 comprehension checks** and **Phase 2 scam refusal** without facilitator hints.
- **Zero Critical Misconceptions:** `0` testers believe unchecked capabilities (`Mouse & keyboard`, `Clipboard sync`, `File transfer`) are enabled by default.
- **Actionable Failure Rule:** If any tester fails a question or hesitates > 10 seconds to locate the caller identity, 6-digit code, or `Deny` button, log the exact UI confusion in Section 4 and refine `ConsentPromptCard` copy/layout before tagging `v0.1.0`.

---

## 4. Tester Scorecard (Complete During Live Human Validation)

*Do not pre-fill or fabricate results. Record actual observations from 5 non-technical participants (include at least 2 participants aged 60+ or low-tech-comfort).*

| Tester ID | Age / Tech Comfort | Date (UTC) | Q1 Identity & Domain (<=10s) | Q2 6-Digit Code Match | Q3 Screen-Only Default | Q4 5s Safety Wait | Q5 Deny / End Anytime | Phase 2 Scam Refusal | Overall Result | Observations / Friction Notes |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `P1` | *(e.g., 68 / low)* | | `[ ] Pass  [ ] Fail` | `[ ] Pass  [ ] Fail` | `[ ] Pass  [ ] Fail` | `[ ] Pass  [ ] Fail` | `[ ] Pass  [ ] Fail` | `[ ] Pass  [ ] Fail` | `[ ] PASS  [ ] FAIL` | |
| `P2` | | | `[ ] Pass  [ ] Fail` | `[ ] Pass  [ ] Fail` | `[ ] Pass  [ ] Fail` | `[ ] Pass  [ ] Fail` | `[ ] Pass  [ ] Fail` | `[ ] Pass  [ ] Fail` | `[ ] PASS  [ ] FAIL` | |
| `P3` | | | `[ ] Pass  [ ] Fail` | `[ ] Pass  [ ] Fail` | `[ ] Pass  [ ] Fail` | `[ ] Pass  [ ] Fail` | `[ ] Pass  [ ] Fail` | `[ ] Pass  [ ] Fail` | `[ ] PASS  [ ] FAIL` | |
| `P4` | | | `[ ] Pass  [ ] Fail` | `[ ] Pass  [ ] Fail` | `[ ] Pass  [ ] Fail` | `[ ] Pass  [ ] Fail` | `[ ] Pass  [ ] Fail` | `[ ] Pass  [ ] Fail` | `[ ] PASS  [ ] FAIL` | |
| `P5` | | | `[ ] Pass  [ ] Fail` | `[ ] Pass  [ ] Fail` | `[ ] Pass  [ ] Fail` | `[ ] Pass  [ ] Fail` | `[ ] Pass  [ ] Fail` | `[ ] Pass  [ ] Fail` | `[ ] PASS  [ ] FAIL` | |

**Summary Sign-Off:**
- Testers Passed: `___ / 5`
- Facilitator: `_____________________`
- Follow-up UI Changes Required: `[ ] None  [ ] See notes above`
