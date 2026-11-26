import React, { useCallback, useEffect, useMemo, useState } from "react";
import { ChevronDown } from "lucide-react";
import { ConsentPromptCard } from "./components/ConsentPromptCard";
import { HostFacingBracketsIcon } from "./components/HostFacingBracketsIcon";
import { PersistentSessionBanner } from "./components/PersistentSessionBanner";
import { createTauriHostBridge, type HostBridge } from "./services/hostBridge";
import type { GrantDuration, HostPermissionSelection, HostSnapshot } from "./types";

interface AppProps {
  bridge?: HostBridge;
  viewOverride?: "consent" | "banner";
  initialSnapshot?: HostSnapshot;
}

const EMPTY_SNAPSHOT: HostSnapshot = {
  tray_state: "idle",
  consent_state_name: "IDLE",
  pending_request: null,
  active_banner: null,
  alert_message: null,
  audit_entry_count: 0,
  blocked_input_count: 0,
  dropped_frame_count: 0,
  recent_audit: [],
};

export const App: React.FC<AppProps> = ({
  bridge: injectedBridge,
  viewOverride,
  initialSnapshot,
}) => {
  const bridge = useMemo(() => injectedBridge ?? createTauriHostBridge(), [injectedBridge]);
  const viewMode = useMemo(() => {
    if (viewOverride) return viewOverride;
    if (typeof window !== "undefined") {
      const params = new URLSearchParams(window.location.search);
      if (params.get("view") === "banner") return "banner";
    }
    return "consent";
  }, [viewOverride]);

  const [snapshot, setSnapshot] = useState<HostSnapshot>(initialSnapshot ?? EMPTY_SNAPSHOT);
  const [submitting, setSubmitting] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  useEffect(() => {
    if (typeof document === "undefined") return;
    const isBanner = viewMode === "banner";
    document.body.style.backgroundColor = isBanner ? "#0E1214" : "#F5F6F4";
    document.body.style.color = isBanner ? "#FFFFFF" : "#111618";
  }, [viewMode]);

  useEffect(() => {
    let active = true;
    let unlisten: (() => void) | undefined;

    if (!initialSnapshot) {
      void bridge.getSnapshot().then((snap) => {
        if (active) setSnapshot(snap);
      });
    }
    void bridge.subscribe((snap) => {
      if (active) setSnapshot(snap);
    }).then((fn) => {
      unlisten = fn;
    });

    return () => {
      active = false;
      unlisten?.();
    };
  }, [bridge, initialSnapshot]);

  const handleAllow = useCallback(
    async (permissions: HostPermissionSelection, duration: GrantDuration) => {
      setSubmitting(true);
      setErrorMessage(null);
      try {
        const next = await bridge.allowSession(permissions, duration);
        setSnapshot(next);
      } catch (err) {
        setErrorMessage(err instanceof Error ? err.message : String(err));
      } finally {
        setSubmitting(false);
      }
    },
    [bridge]
  );

  const handleDeny = useCallback(async () => {
    setSubmitting(true);
    setErrorMessage(null);
    try {
      const next = await bridge.denySession();
      setSnapshot(next);
    } catch (err) {
      setErrorMessage(err instanceof Error ? err.message : String(err));
    } finally {
      setSubmitting(false);
    }
  }, [bridge]);

  const handleBlockAndReport = useCallback(
    async (category: string, summary: string) => {
      if (!bridge.blockAndReportPending) {
        await handleDeny();
        return;
      }
      setSubmitting(true);
      setErrorMessage(null);
      try {
        const next = await bridge.blockAndReportPending(category, summary);
        setSnapshot(next);
      } catch (err) {
        setErrorMessage(err instanceof Error ? err.message : String(err));
      } finally {
        setSubmitting(false);
      }
    },
    [bridge, handleDeny]
  );

  const handleEndSession = useCallback(async () => {
    try {
      const next = await bridge.endActiveSessionLocal();
      setSnapshot(next);
    } catch (err) {
      setErrorMessage(err instanceof Error ? err.message : String(err));
    }
  }, [bridge]);

  const handleRevokeInput = useCallback(async () => {
    const active = snapshot.active_banner;
    if (!active) return;
    try {
      const next = await bridge.updateActivePermissions({
        view_screen: true,
        mouse_keyboard: false,
        clipboard_sync: active.permissions.clipboard_sync,
        file_transfer: active.permissions.file_transfer,
      });
      setSnapshot(next);
    } catch (err) {
      setErrorMessage(err instanceof Error ? err.message : String(err));
    }
  }, [bridge, snapshot.active_banner]);

  if (viewMode === "banner") {
    if (!snapshot.active_banner) {
      return (
        <main className="flex h-[56px] w-full items-center justify-center border border-[#2A3236] bg-[#0E1214] px-4 text-sm text-white">
          No active session
        </main>
      );
    }
    return (
      <main className="w-full bg-[#0E1214] p-0">
        <PersistentSessionBanner
          banner={snapshot.active_banner}
          onEndSession={handleEndSession}
          onRevokeInput={handleRevokeInput}
        />
      </main>
    );
  }

  if (snapshot.pending_request) {
    return (
      <ConsentPromptCard
        request={snapshot.pending_request}
        submitting={submitting}
        errorMessage={errorMessage}
        onAllow={handleAllow}
        onDeny={handleDeny}
        onBlockAndReport={handleBlockAndReport}
      />
    );
  }

  const isAlert = snapshot.tray_state === "alert";
  const isInSession = snapshot.tray_state === "in_session" || Boolean(snapshot.active_banner);
  const statusText = isAlert
    ? "Alert · Connection blocked"
    : isInSession
      ? "In session · Active grant"
      : "Ready · No active session";

  return (
    <main className="mx-auto flex min-h-[660px] w-full max-w-[520px] flex-col bg-[#F5F6F4] text-[#111618]">
      <header className="flex items-center justify-between border-b border-[#DADDD8] bg-white px-5 py-3.5">
        <div className="flex items-center gap-2.5">
          <HostFacingBracketsIcon
            state={snapshot.tray_state}
            className={`h-6 w-6 ${isAlert ? "text-[#B42318]" : "text-[#0F5C52]"}`}
            label={`Host status: ${snapshot.tray_state}`}
          />
          <h1 className="font-display text-lg font-semibold tracking-[-0.02em] text-[#111618]">
            Oudos
          </h1>
        </div>

        <div className="flex items-center gap-2 text-xs font-medium text-[#525D63]">
          <span
            aria-hidden="true"
            className={`h-2 w-2 rounded-full ${
              isAlert ? "bg-[#B42318]" : "bg-[#0F5C52]"
            }`}
          />
          <span>{statusText}</span>
        </div>
      </header>

      {snapshot.active_banner ? (
        <section className="border-b border-[#DADDD8] bg-[#0E1214]">
          <PersistentSessionBanner
            banner={snapshot.active_banner}
            onEndSession={handleEndSession}
            onRevokeInput={handleRevokeInput}
          />
        </section>
      ) : (
        <section
          aria-labelledby="idle-state-heading"
          className="border-b border-[#DADDD8] bg-white px-5 py-5"
        >
          <h2
            id="idle-state-heading"
            className="font-display text-lg font-semibold text-[#111618]"
          >
            {isAlert ? "Connection blocked" : "Waiting for requests"}
          </h2>
          <p className="mt-1.5 text-sm leading-relaxed text-[#525D63]">
            {snapshot.alert_message ??
              "Nobody can view or control this computer until you review their request and click Allow."}
          </p>
        </section>
      )}

      <section
        aria-label="How every connection works"
        className="flex flex-1 flex-col border-b border-[#DADDD8] bg-white"
      >
        <div className="border-b border-[#DADDD8] bg-[#F5F6F4] px-5 py-2.5">
          <h2 className="text-xs font-semibold text-[#525D63]">
            How this computer stays protected
          </h2>
        </div>

        <ol className="flex flex-1 flex-col justify-between divide-y divide-[#DADDD8] text-sm">
          <li className="px-5 py-2.5">
            <p className="font-semibold text-[#111618]">Approval required</p>
            <p className="mt-0.5 text-xs text-[#525D63]">
              Every connection prompts on this screen before anything is shared.
            </p>
          </li>
          <li className="px-5 py-2.5">
            <p className="font-semibold text-[#111618]">6-digit code check</p>
            <p className="mt-0.5 text-xs text-[#525D63]">
              Match the 6-digit verification code with the caller before clicking Allow.
            </p>
          </li>
          <li className="px-5 py-2.5">
            <p className="font-semibold text-[#111618]">Screen view only by default</p>
            <p className="mt-0.5 text-xs text-[#525D63]">
              Mouse, keyboard, clipboard, and file transfer always start turned off.
            </p>
          </li>
          <li className="px-5 py-2.5">
            <p className="font-semibold text-[#111618]">Unattended access disabled</p>
            <p className="mt-0.5 text-xs text-[#525D63]">
              Background connections cannot run when you are away from this PC.
            </p>
          </li>
          <li className="px-5 py-2.5">
            <p className="font-semibold text-[#111618]">Instant stop control</p>
            <p className="mt-0.5 text-xs text-[#525D63]">
              A top-most bar with a red End session button stays visible during sessions.
            </p>
          </li>
        </ol>
      </section>

      <footer className="bg-[#F5F6F4] px-5 py-3 text-xs text-[#525D63]">
        <details className="group">
          <summary className="flex cursor-pointer list-none items-center justify-between py-1 font-medium text-[#525D63] hover:text-[#111618] focus:outline-none focus-visible:ring-2 focus-visible:ring-[#0F5C52]">
            <span>Diagnostics</span>
            <ChevronDown
              className="h-3.5 w-3.5 transition-transform group-open:rotate-180"
              aria-hidden="true"
            />
          </summary>

          <div
            aria-label="Local host protection counters"
            className="mt-2.5 grid grid-cols-3 gap-3 border-t border-[#DADDD8] pt-2.5"
          >
            <div>
              <p className="text-[11px] text-[#525D63]">Audit entries</p>
              <p
                data-testid="diag-audit-count"
                className="font-mono-code text-sm font-semibold text-[#111618]"
              >
                {snapshot.audit_entry_count}
              </p>
            </div>
            <div>
              <p className="text-[11px] text-[#525D63]">Blocked input</p>
              <p
                data-testid="diag-blocked-input"
                className="font-mono-code text-sm font-semibold text-[#111618]"
              >
                {snapshot.blocked_input_count}
              </p>
            </div>
            <div>
              <p className="text-[11px] text-[#525D63]">Dropped frames</p>
              <p
                data-testid="diag-dropped-frames"
                className="font-mono-code text-sm font-semibold text-[#111618]"
              >
                {snapshot.dropped_frame_count}
              </p>
            </div>
          </div>
        </details>

        <p className="mt-2 border-t border-[#DADDD8] pt-2 text-[11px] text-[#525D63]">
          End-to-end encrypted · Metadata-only local audit log
        </p>
      </footer>
    </main>
  );
};

export default App;
