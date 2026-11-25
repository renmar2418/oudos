import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { fireEvent, render, screen } from "@testing-library/react";
import axe from "axe-core";
import { describe, expect, it } from "vitest";
import App from "./App";
import { SAMPLE_AUDIT_ENTRIES } from "./data/sampleData";
import { buildAuditCsv } from "./screens/AuditLogScreen";
import type { CoreTab } from "./types";

async function expectZeroAxeViolations(container: HTMLElement) {
  const results = await axe.run(container, {
    rules: {
      // jsdom does not compute CSS layout/colors for color-contrast; all other
      // WCAG 2.2 AA structural, ARIA, label, landmark, and heading rules run.
      "color-contrast": { enabled: false },
    },
  });
  const criticalOrSerious = results.violations.filter(
    (v) => v.impact === "critical" || v.impact === "serious"
  );
  expect(criticalOrSerious).toEqual([]);
}

describe("Oudos web-pwa core 5 screens & accessibility", () => {
  const tabs: CoreTab[] = ["devices", "requesting", "session", "policies", "audit"];

  it.each(tabs)("renders %s screen with zero critical/serious axe-core violations", async (tab) => {
    const { container } = render(<App initialTab={tab} />);
    await expectZeroAxeViolations(container);
  });

  it("navigates from Devices -> Requesting (6-digit SAS) -> Active Session -> Devices", () => {
    render(<App />);
    expect(screen.getByRole("heading", { name: "Devices" })).toBeInTheDocument();

    const connectButtons = screen.getAllByRole("button", { name: "Request access" });
    fireEvent.click(connectButtons[0]!);

    expect(screen.getByText(/739 204/)).toBeInTheDocument();
    expect(screen.getByText(/Waiting for Dana to allow access/i)).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Preview active session" }));
    expect(screen.getByText(/E2E Encrypted/i)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Ctrl+Alt+Del" })).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "End" }));
    expect(screen.getByRole("heading", { name: "Devices" })).toBeInTheDocument();
  });

  it("enforces monotonic policy UI locks and prevents relaxing idle timeout", () => {
    render(<App initialTab="policies" />);
    expect(screen.getByText("Locked OFF in v0.1")).toBeInTheDocument();

    const select = screen.getByLabelText("Idle disconnect timeout") as HTMLSelectElement;
    expect(select.value).toBe("30m");

    // Relaxing to 1h is disabled/ignored.
    const option1h = screen.getByRole("option", { name: "1 hour" }) as HTMLOptionElement;
    expect(option1h.disabled).toBe(true);

    // Tightening to 15m succeeds.
    fireEvent.change(select, { target: { value: "15m" } });
    expect(select.value).toBe("15m");
  });

  it("builds verifiable CSV export for audit log", () => {
    const csv = buildAuditCsv(SAMPLE_AUDIT_ENTRIES);
    const lines = csv.split("\n");
    expect(lines.length).toBe(4);
    expect(lines[0]).toContain("prev_hash,entry_hash,host_signature");
  });

  it("includes valid PWA manifest and service worker assets", () => {
    const manifestRaw = readFileSync(
      resolve(__dirname, "../public/manifest.webmanifest"),
      "utf-8"
    );
    const manifest = JSON.parse(manifestRaw) as {
      name: string;
      display: string;
      theme_color: string;
      icons: { src: string }[];
    };
    expect(manifest.name).toBe("Oudos Remote Desktop");
    expect(manifest.display).toBe("standalone");
    expect(manifest.theme_color).toBe("#0F7B6C");
    expect(manifest.icons.length).toBeGreaterThan(0);

    const sw = readFileSync(resolve(__dirname, "../public/sw.js"), "utf-8");
    expect(sw).toContain("oudos-pwa-v1");
  });
});
