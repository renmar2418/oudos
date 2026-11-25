import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { expect, test, type Page, type WebSocketRoute } from "@playwright/test";

const require = createRequire(import.meta.url);
const axeSource = readFileSync(require.resolve("axe-core/axe.min.js"), "utf-8");

async function assertWcagAndColorRules(page: Page) {
  await page.evaluate(axeSource);
  const axeResult = await page.evaluate(async () => {
    // @ts-expect-error injected global axe
    return window.axe.run(document.body, {
      runOnly: {
        type: "tag",
        values: ["wcag2a", "wcag2aa", "wcag22aa"],
      },
    });
  });

  const criticalOrSerious = (
    axeResult.violations as { id: string; impact: string; description: string }[]
  ).filter((v) => v.impact === "critical" || v.impact === "serious");
  expect(criticalOrSerious, JSON.stringify(criticalOrSerious)).toEqual([]);

  // Guardrail: #5FD9BF (rgb(95, 217, 191)) must never be rendered on light surfaces (#F5F6F4 / #FFFFFF).
  const lightSurfaceMintViolations = await page.evaluate(() => {
    const MINT = "rgb(95, 217, 191)";
    const violations: string[] = [];
    for (const el of document.querySelectorAll("body *")) {
      const style = window.getComputedStyle(el);
      const inDarkChrome = Boolean(el.closest('[class*="bg-[#0E1214]"], [class*="bg-[#090C0D]"], [class*="bg-[#13181B]"], [class*="bg-[#141D20]"]'));
      if (inDarkChrome) continue;
      if (style.color === MINT || style.fill === MINT || style.stroke === MINT) {
        violations.push(`${el.tagName}:${el.textContent?.slice(0, 30)}`);
      }
    }
    return violations;
  });
  expect(lightSurfaceMintViolations).toEqual([]);

  // Guardrail: no prototype-only Simulate approval / Preview active session buttons.
  await expect(page.getByRole("button", { name: /simulate approval/i })).toHaveCount(0);
  await expect(page.getByRole("button", { name: /preview active session/i })).toHaveCount(0);
}

async function setupSignalingMock(page: Page): Promise<{
  getRoute: () => WebSocketRoute | null;
}> {
  let activeRoute: WebSocketRoute | null = null;
  await page.routeWebSocket("**/ws/signal*", (ws) => {
    activeRoute = ws;
  });
  return {
    getRoute: () => activeRoute,
  };
}

test.describe("Oudos web-pwa MVP flows and 10 edge states", () => {
  test("1. Add Device pairing wizard adds device and logs enrollment", async ({ page }) => {
    await setupSignalingMock(page);
    await page.goto("/");

    await page.getByRole("button", { name: "Add device" }).click();
    await expect(page.getByRole("dialog", { name: /Pair a new host device/i })).toBeVisible();
    await expect(page.getByText("OUD-8492-K7M3", { exact: true })).toBeVisible();

    await page.getByRole("button", { name: "Copy pairing code" }).click();
    await expect(page.getByText("Copied pairing code")).toBeVisible();
    await page.getByRole("button", { name: "Continue to device details" }).click();

    await page.getByLabel("9-digit Device ID").fill("619 402 851");
    await page.getByLabel("Device name").fill("Field Tablet 01");
    await page.getByLabel("Device owner").fill("Morgan");
    await expect(page.getByText(/Unattended mode locked OFF in v0.1/i)).toBeVisible();
    await page.getByRole("button", { name: "Review & pin key" }).click();

    await page.getByRole("button", { name: "Pin key & add device" }).click();
    await expect(page.getByRole("heading", { name: "Field Tablet 01" })).toBeVisible();
    await assertWcagAndColorRules(page);
  });

  test("2. Sessions tab shows active/completed counts and session controls", async ({ page }) => {
    await setupSignalingMock(page);
    await page.goto("/");

    await page.getByRole("button", { name: "Sessions", exact: true }).click();
    await expect(page.getByRole("heading", { name: "Sessions", exact: true })).toBeVisible();
    await expect(page.getByText("Active now")).toBeVisible();
    await expect(page.getByText("Studio Desktop")).toBeVisible();
    await assertWcagAndColorRules(page);
  });

  test("3. Screen unavailable remediation card shows cause, steps, and retry", async ({ page }) => {
    await setupSignalingMock(page);
    await page.goto("/");

    await page.getByRole("button", { name: "View fix & retry" }).click();
    await expect(page.getByRole("heading", { name: /Screen capture permission missing/i })).toBeVisible();
    await expect(page.getByText(/Check Screen Capture Permission/i)).toBeVisible();

    await page.getByRole("button", { name: "locked" }).click();
    await expect(page.getByRole("heading", { name: /Screen locked on host PC/i })).toBeVisible();

    await page.getByRole("button", { name: "asleep" }).click();
    await expect(page.getByRole("heading", { name: /Device display is asleep/i })).toBeVisible();

    await page.getByRole("button", { name: "no display" }).click();
    await expect(page.getByRole("heading", { name: /No active monitor detected/i })).toBeVisible();
    await assertWcagAndColorRules(page);

    await page.getByRole("button", { name: "Check availability again" }).click();
    await expect(page.getByRole("heading", { name: /Waiting for Maya to allow access/i })).toBeVisible();
  });

  test("4. Owner Denied state renders decline summary and cooldown count", async ({ page }) => {
    const ws = await setupSignalingMock(page);
    await page.goto("/");

    await page.getByRole("button", { name: "Request access" }).first().click();
    await expect(page.getByRole("heading", { name: /Waiting for Dana to allow access/i })).toBeVisible();

    ws.getRoute()?.send(
      JSON.stringify({
        type: "owner_denied",
        session_id: "sess-denied",
        streakCount: 1,
      })
    );

    await expect(page.getByRole("heading", { name: /Dana denied the connection request/i })).toBeVisible();
    await expect(page.getByText(/current streak: 1 of 3/i)).toBeVisible();
    await assertWcagAndColorRules(page);
  });

  test("5. Request Expired state triggers automatically after 2-minute TTL", async ({ page }) => {
    await page.clock.install();
    await setupSignalingMock(page);
    await page.goto("/");

    await page.getByRole("button", { name: "Request access" }).first().click();
    await expect(page.getByRole("heading", { name: /Waiting for Dana to allow access/i })).toBeVisible();

    await page.clock.fastForward("02:01");

    await expect(page.getByRole("heading", { name: /Request expired after 2 minutes/i })).toBeVisible();
    await assertWcagAndColorRules(page);
  });

  test("6. Code Mismatch state aborts immediately and warns against coercion", async ({ page }) => {
    await setupSignalingMock(page);
    await page.goto("/");

    await page.getByRole("button", { name: "Request access" }).first().click();
    await page.getByRole("button", { name: /Codes don't match/i }).click();

    await expect(page.getByRole("heading", { name: /6-digit verification codes did not match/i })).toBeVisible();
    await expect(page.getByText(/Do not continue if anyone asks you to ignore a mismatched code/i)).toBeVisible();
    await assertWcagAndColorRules(page);
  });

  test("7. Device Key Changed blocks connection and requires 9-digit confirmation to re-pin", async ({ page }) => {
    const ws = await setupSignalingMock(page);
    await page.goto("/");

    await page.getByRole("button", { name: "Request access" }).first().click();

    ws.getRoute()?.send(
      JSON.stringify({
        type: "host_key_changed",
        session_id: "sess-key-changed",
        expectedPubkey: "ed25519:6a7f90c2e14b88d3a5c1f0987234b612a98e41c7d91e",
        presentedPubkey: "ed25519:ff112233445566778899aabbccddeeff001122334455",
      })
    );

    const dialog = page.getByRole("alertdialog");
    await expect(dialog).toBeVisible();
    await expect(page.getByRole("heading", { name: /Device key changed for Dana's ThinkPad/i })).toBeVisible();
    await assertWcagAndColorRules(page);

    await page.getByRole("button", { name: /I verified this key change directly with Dana/i }).click();
    const repinBtn = page.getByRole("button", { name: /Accept new key & re-pin device/i });
    await expect(repinBtn).toBeDisabled();

    await page.getByLabel(/Type the 9-digit Device ID/i).fill("482 190 734");
    await expect(repinBtn).toBeEnabled();
    await repinBtn.click();

    await expect(page.getByRole("heading", { name: /Waiting for Dana to allow access/i })).toBeVisible();
  });

  test("8 & 9. Host Offline mid-session and Session Ended summary", async ({ page }) => {
    const ws = await setupSignalingMock(page);
    await page.goto("/");

    await page.getByRole("button", { name: "Request access" }).first().click();

    ws.getRoute()?.send(
      JSON.stringify({
        type: "grant_activated",
        session_id: "sess-live-1",
        approver: "Dana",
        permissions: {
          view_screen: true,
          mouse_keyboard: false,
          clipboard_sync: false,
          file_transfer: false,
          blank_screen: false,
        },
        transportMode: "direct_p2p",
        latencyMs: 38,
      })
    );

    await expect(page.getByText(/E2E Encrypted · Live/i)).toBeVisible();
    await assertWcagAndColorRules(page);

    ws.getRoute()?.send(
      JSON.stringify({
        type: "host_offline",
        session_id: "sess-live-1",
      })
    );

    await expect(page.getByRole("heading", { name: /Host went offline mid-session/i })).toBeVisible();
    await page.getByRole("button", { name: /End session & view summary/i }).click();

    await expect(page.getByRole("heading", { name: /Session with Dana's ThinkPad closed/i })).toBeVisible();
    await expect(page.getByText("Host went offline")).toBeVisible();
    await assertWcagAndColorRules(page);
  });

  test("10. Control-plane offline fail-closed + Ask to join + Notify me when online", async ({ page }) => {
    const ws = await setupSignalingMock(page);
    await page.goto("/");

    await page.getByRole("button", { name: "Notify me when online" }).click();
    await expect(page.getByRole("button", { name: "Notifying when online" })).toBeVisible();

    await page.getByRole("button", { name: "Ask to join" }).click();
    await expect(page.getByText(/Asking to join the live session on/i)).toBeVisible();

    ws.getRoute()?.send(
      JSON.stringify({
        type: "control_plane_offline",
        reason: "Signaling relay unreachable",
      })
    );

    await expect(page.getByRole("heading", { name: /Control plane unreachable/i })).toBeVisible();
    await expect(page.getByText(/Zero new sessions can start/i)).toBeVisible();
    await assertWcagAndColorRules(page);
  });
});
