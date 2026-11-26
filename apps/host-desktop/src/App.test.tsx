import { act, fireEvent, render, screen } from "@testing-library/react";
import axe from "axe-core";
import { afterEach, describe, expect, it, vi } from "vitest";
import App from "./App";
import { HostFacingBracketsIcon } from "./components/HostFacingBracketsIcon";
import type { HostBridge } from "./services/hostBridge";
import type { HostSnapshot } from "./types";

function expectNoMintOnLightSurfaces(container: HTMLElement) {
  const mintNodes = container.querySelectorAll('[class*="5FD9BF"]');
  for (const node of mintNodes) {
    expect(node.closest('[class*="bg-[#0E1214]"]')).not.toBeNull();
  }
}

async function expectZeroAxeViolations(container: HTMLElement) {
  expectNoMintOnLightSurfaces(container);
  const results = await axe.run(container, {
    rules: {
      "color-contrast": { enabled: false },
    },
  });
  const criticalOrSerious = results.violations.filter(
    (v) => v.impact === "critical" || v.impact === "serious"
  );
  expect(criticalOrSerious).toEqual([]);
}

function makePendingSnapshot(overrides?: Partial<HostSnapshot>): HostSnapshot {
  return {
    tray_state: "idle",
    consent_state_name: "COOLING_OFF_5S",
    pending_request: {
      session_id: "019363f2-0000-7000-8000-000000000001",
      requester_name: "Alex Rivera",
      requester_pubkey: "ed25519:abcd1234",
      server_domain: "support.example.org",
      server_enrolled: true,
      sas_code: "739 204",
      first_time_requester: true,
      cooling_off_remaining_secs: 5,
      ttl_remaining_secs: 120,
      permissions: {
        view_screen: true,
        mouse_keyboard: false,
        clipboard_sync: false,
        file_transfer: false,
      },
      policy_ceilings: {
        allow_mouse_keyboard: true,
        allow_clipboard_sync: true,
        allow_file_transfer: false,
        idle_timeout_label: "30 minutes",
      },
      duration: "this_session",
    },
    active_banner: null,
    alert_message: null,
    audit_entry_count: 1,
    blocked_input_count: 0,
    dropped_frame_count: 0,
    recent_audit: [],
    ...overrides,
  };
}

function makeMemoryBridge(initial: HostSnapshot): {
  bridge: HostBridge;
  allowCalls: unknown[];
  endCalls: number;
} {
  let snap = initial;
  const allowCalls: unknown[] = [];
  let endCalls = 0;

  const bridge: HostBridge = {
    getSnapshot: async () => snap,
    tick: async () => snap,
    presentIncomingRequest: async () => snap,
    allowSession: async (permissions, duration) => {
      allowCalls.push({ permissions, duration });
      snap = {
        ...snap,
        tray_state: "in_session",
        consent_state_name: "GRANT_SIGNED_ACTIVE",
        pending_request: null,
        active_banner: {
          session_id: "019363f2-0000-7000-8000-000000000001",
          requester_name: "Alex Rivera",
          server_domain: "support.example.org",
          server_enrolled: true,
          status_label: "LIVE",
          permissions: {
            view_screen: true,
            mouse_keyboard: permissions.mouse_keyboard,
            clipboard_sync: permissions.clipboard_sync,
            file_transfer: false,
            blank_screen: false,
          },
          permission_summary: permissions.mouse_keyboard ? "Screen + Input" : "Screen only",
          duration,
          expires_at_utc: "2026-11-25T16:00:00Z",
        },
      };
      return snap;
    },
    denySession: async () => {
      snap = {
        ...snap,
        tray_state: "idle",
        consent_state_name: "DENIED_LOGGED",
        pending_request: null,
      };
      return snap;
    },
    blockAndReportPending: async (_category, _summary) => {
      snap = {
        ...snap,
        tray_state: "alert",
        consent_state_name: "DENIED_LOGGED",
        pending_request: null,
        alert_message: "Requester blocked on this device.",
      };
      return snap;
    },
    updateActivePermissions: async (permissions) => {
      if (snap.active_banner) {
        snap = {
          ...snap,
          active_banner: {
            ...snap.active_banner,
            permissions: {
              ...snap.active_banner.permissions,
              mouse_keyboard: permissions.mouse_keyboard,
            },
            permission_summary: permissions.mouse_keyboard ? "Screen + Input" : "Screen only",
          },
        };
      }
      return snap;
    },
    endActiveSessionLocal: async () => {
      endCalls += 1;
      snap = {
        ...snap,
        tray_state: "idle",
        consent_state_name: "REVOKED_LOGGED",
        active_banner: null,
      };
      return snap;
    },
    subscribe: async () => () => {},
  };

  return {
    bridge,
    allowCalls,
    get endCalls() {
      return endCalls;
    },
  };
}

describe("Oudos host-desktop Consent UI, Banner & Tray", () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  it("renders consent card guardrails: raw domain, 5s cooling-off, Screen-only defaults, v0.1 durations", async () => {
    vi.useFakeTimers();
    const initial = makePendingSnapshot();
    const { bridge, allowCalls } = makeMemoryBridge(initial);

    const { container } = render(<App bridge={bridge} initialSnapshot={initial} />);

    // Raw server domain (never a fake Verified badge)
    const domainChip = screen.getByTestId("server-domain-chip");
    expect(domainChip).toHaveTextContent("support.example.org");
    expect(screen.queryByText(/Verified/i)).not.toBeInTheDocument();

    // 6-digit SAS code & plain anti-scam warning
    expect(screen.getByTestId("sas-code-display")).toHaveTextContent("739 204");
    expect(screen.getByTestId("first-time-cooling-warning")).toHaveTextContent(
      /Real banks will NEVER ask you to share your screen/i
    );

    // Permission defaults: See your screen checked+disabled; Mouse/Keyboard, Clipboard, File transfer OFF
    expect(screen.getByRole("switch", { name: /See your screen/i })).toBeChecked();
    expect(screen.getByRole("switch", { name: /See your screen/i })).toBeDisabled();
    expect(screen.getByRole("switch", { name: /Mouse & keyboard/i })).not.toBeChecked();
    expect(screen.getByRole("switch", { name: /Clipboard/i })).not.toBeChecked();
    expect(screen.getByRole("switch", { name: /File transfer/i })).not.toBeChecked();

    // v0.1 duration options only (no "Until I turn it off")
    expect(screen.getByLabelText("This session")).toBeChecked();
    expect(screen.getByLabelText("1 hour")).not.toBeChecked();
    expect(screen.queryByText(/Until I turn it off/i)).not.toBeInTheDocument();

    // Allow is disabled during 5s cooling-off
    const allowBtn = screen.getByRole("button", { name: /Allow \(5s\)/i });
    expect(allowBtn).toBeDisabled();

    // Advance 5 seconds -> Allow unlocks
    await act(async () => {
      vi.advanceTimersByTime(5000);
    });
    const unlockedAllow = screen.getByRole("button", { name: /^Allow$/i });
    expect(unlockedAllow).toBeEnabled();

    await act(async () => {
      fireEvent.click(unlockedAllow);
    });
    expect(allowCalls.length).toBe(1);

    vi.useRealTimers();
    await expectZeroAxeViolations(container);
  });

  it("renders persistent session banner with LIVE status, permission summary, and End session button", async () => {
    const activeSnap = makePendingSnapshot({
      tray_state: "in_session",
      consent_state_name: "GRANT_SIGNED_ACTIVE",
      pending_request: null,
      active_banner: {
        session_id: "019363f2-0000-7000-8000-000000000001",
        requester_name: "Alex Rivera",
        server_domain: "support.example.org",
        server_enrolled: true,
        status_label: "LIVE",
        permissions: {
          view_screen: true,
          mouse_keyboard: true,
          clipboard_sync: false,
          file_transfer: false,
          blank_screen: false,
        },
        permission_summary: "Screen + Input",
        duration: "this_session",
        expires_at_utc: "2026-11-25T16:00:00Z",
      },
    });
    const mem = makeMemoryBridge(activeSnap);

    const { container } = render(
      <App bridge={mem.bridge} viewOverride="banner" initialSnapshot={activeSnap} />
    );

    const banner = screen.getByTestId("persistent-session-banner");
    expect(banner).toHaveTextContent("LIVE ·");
    expect(screen.getByTestId("banner-requester-name")).toHaveTextContent("Alex Rivera");
    expect(screen.getByTestId("banner-server-domain")).toHaveTextContent("support.example.org");
    expect(banner).toHaveTextContent("Screen + Input");

    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "End session" }));
    });
    expect(mem.endCalls).toBe(1);
    await expectZeroAxeViolations(container);
  });

  it("warns explicitly for un-enrolled servers, supports Deny, and revokes input mid-session", async () => {
    const unenrolled = makePendingSnapshot({
      pending_request: {
        ...makePendingSnapshot().pending_request!,
        server_domain: "rogue-relay.example",
        server_enrolled: false,
        first_time_requester: false,
        cooling_off_remaining_secs: 0,
      },
    });
    const mem = makeMemoryBridge(unenrolled);
    const { container } = render(
      <App bridge={mem.bridge} initialSnapshot={unenrolled} />
    );

    expect(screen.getByTestId("server-domain-chip")).toHaveTextContent(
      "rogue-relay.example · Personal / Unverified"
    );
    expect(screen.queryByText(/\bVerified\b/i)).not.toBeInTheDocument();

    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Deny" }));
    });
    expect(screen.getByRole("heading", { name: /Waiting for requests/i })).toBeInTheDocument();

    const activeInputSnap = makePendingSnapshot({
      tray_state: "in_session",
      pending_request: null,
      active_banner: {
        session_id: "019363f2-0000-7000-8000-000000000001",
        requester_name: "Alex Rivera",
        server_domain: "support.example.org",
        server_enrolled: true,
        status_label: "LIVE",
        permissions: {
          view_screen: true,
          mouse_keyboard: true,
          clipboard_sync: false,
          file_transfer: false,
          blank_screen: false,
        },
        permission_summary: "Screen + Input",
        duration: "this_session",
        expires_at_utc: "2026-11-25T16:00:00Z",
      },
    });
    const mem2 = makeMemoryBridge(activeInputSnap);
    const bannerView = render(
      <App bridge={mem2.bridge} viewOverride="banner" initialSnapshot={activeInputSnap} />
    );
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Stop input" }));
    });
    expect(screen.getByTestId("persistent-session-banner")).toHaveTextContent("Screen only");
    await expectZeroAxeViolations(bannerView.container);
    await expectZeroAxeViolations(container);
  });

  it("keeps Idle screen free of spec jargon and tucks telemetry into collapsed Diagnostics", async () => {
    const idleSnap = makePendingSnapshot({
      pending_request: null,
      tray_state: "idle",
      audit_entry_count: 4,
      blocked_input_count: 2,
      dropped_frame_count: 1,
    });
    const { bridge } = makeMemoryBridge(idleSnap);
    const { container } = render(<App bridge={bridge} initialSnapshot={idleSnap} />);

    expect(screen.getByText("Ready · No active session")).toBeInTheDocument();
    expect(screen.queryByText(/Asks-First v0\.1/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/Monochrome tray indicators/i)).not.toBeInTheDocument();

    const details = container.querySelector("details");
    expect(details).not.toBeNull();
    expect(details?.hasAttribute("open")).toBe(false);

    expect(screen.getByTestId("diag-audit-count")).toHaveTextContent("4");
    expect(screen.getByTestId("diag-blocked-input")).toHaveTextContent("2");
    expect(screen.getByTestId("diag-dropped-frames")).toHaveTextContent("1");

    await expectZeroAxeViolations(container);
  });

  it("renders all 3 colour-independent Facing Brackets tray states cleanly", async () => {
    const { container } = render(
      <div>
        <HostFacingBracketsIcon state="idle" label="Idle" />
        <HostFacingBracketsIcon state="in_session" label="In session" />
        <HostFacingBracketsIcon state="alert" label="Alert" />
      </div>
    );

    const svgs = container.querySelectorAll("svg[data-tray-state]");
    const states = Array.from(svgs).map((el) => el.getAttribute("data-tray-state"));
    expect(states).toEqual(["idle", "in_session", "alert"]);
    expect(container.querySelector('[data-testid="tray-center-dot"]')).toBeInTheDocument();
    expect(container.querySelector('[data-testid="tray-alert-mark"]')).toBeInTheDocument();

    await expectZeroAxeViolations(container);
  });

  it("supports host-side Block & report requester on the consent card", async () => {
    const initial = makePendingSnapshot();
    const { bridge } = makeMemoryBridge(initial);
    const { container } = render(<App bridge={bridge} initialSnapshot={initial} />);

    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: /Block & report requester/i }));
    });
    expect(screen.getByText(/Requester blocked on this device/i)).toBeInTheDocument();
    await expectZeroAxeViolations(container);
  });
});
