import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { act, fireEvent, render, screen } from "@testing-library/react";
import axe from "axe-core";
import { describe, expect, it } from "vitest";
import App from "./App";
import { DEFAULT_VIEW_ONLY_PERMISSIONS, SAMPLE_AUDIT_ENTRIES } from "./data/sampleData";
import { buildAuditCsv } from "./screens/AuditLogScreen";
import type { IncomingSignalEvent, SessionTransport } from "./services/sessionTransport";
import type { CoreTab } from "./types";

async function expectZeroAxeViolations(container: HTMLElement) {
  const results = await axe.run(container, {
    rules: {
      // jsdom does not compute CSS layout/colors for color-contrast; Playwright Chromium
      // verifies computed color-contrast in e2e/edge-states.spec.ts.
      "color-contrast": { enabled: false },
    },
  });
  const criticalOrSerious = results.violations.filter(
    (v) => v.impact === "critical" || v.impact === "serious"
  );
  expect(criticalOrSerious).toEqual([]);
}

function createMemoryTransport() {
  const listeners = new Set<(event: IncomingSignalEvent) => void>();
  const sent: unknown[] = [];
  const transport: SessionTransport = {
    subscribe(listener) {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    send(command) {
      sent.push(command);
    },
  };
  const emit = (event: IncomingSignalEvent) => {
    act(() => {
      for (const listener of listeners) {
        listener(event);
      }
    });
  };
  return { transport, emit, sent };
}

describe("Oudos web-pwa screens, edge states & accessibility", () => {
  const tabs: CoreTab[] = [
    "devices",
    "sessions",
    "requesting",
    "session",
    "policies",
    "audit",
  ];

  it.each(tabs)("renders %s screen with zero critical/serious axe-core violations", async (tab) => {
    const { transport } = createMemoryTransport();
    const { container } = render(<App initialTab={tab} transport={transport} />);
    await expectZeroAxeViolations(container);
  });

  it("never renders prototype-only Simulate approval or Preview active session buttons", () => {
    const { transport } = createMemoryTransport();
    render(<App initialTab="requesting" transport={transport} />);
    expect(screen.queryByText(/Simulate approval/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/Preview active session/i)).not.toBeInTheDocument();
  });

  it("navigates Devices -> Requesting -> Host GrantActivated -> Active Session -> Session Ended Summary", () => {
    const { transport, emit, sent } = createMemoryTransport();
    render(<App transport={transport} />);

    fireEvent.click(screen.getAllByRole("button", { name: "Request access" })[0]!);
    expect(screen.getByText(/739 204/)).toBeInTheDocument();
    expect(screen.getByText(/Waiting for Dana to allow access/i)).toBeInTheDocument();
    expect(sent.length).toBeGreaterThan(0);

    emit({
      type: "grant_activated",
      sessionId: "sess-1",
      approver: "Dana",
      permissions: { ...DEFAULT_VIEW_ONLY_PERMISSIONS },
      transportMode: "direct_p2p",
      latencyMs: 36,
    });

    expect(screen.getByText(/E2E Encrypted/i)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Ctrl+Alt+Del" })).toBeDisabled();

    fireEvent.click(screen.getByRole("button", { name: "End" }));
    expect(screen.getByRole("heading", { name: /Session with Dana's ThinkPad closed/i })).toBeInTheDocument();
  });

  it("enforces monotonic policy UI locks and prevents relaxing idle timeout", () => {
    const { transport } = createMemoryTransport();
    render(<App initialTab="policies" transport={transport} />);
    expect(screen.getByText("Locked OFF in v0.1")).toBeInTheDocument();

    const select = screen.getByLabelText("Idle disconnect timeout") as HTMLSelectElement;
    expect(select.value).toBe("30m");

    const option1h = screen.getByRole("option", { name: "1 hour" }) as HTMLOptionElement;
    expect(option1h.disabled).toBe(true);

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
