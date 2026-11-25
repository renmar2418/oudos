import React, { useCallback, useEffect, useMemo, useState } from "react";
import { ShieldCheck } from "lucide-react";
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
        <main className="flex h-[56px] items-center justify-center bg-[#0E1214] px-4 text-sm text-white">
          No active session
        </main>
      );
    }
    return (
      <main className="bg-transparent p-0">
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

  return (
    <main className="mx-auto flex min-h-[660px] w-[520px] max-w-full flex-col justify-between border border-[#DADDD8] bg-[#F5F6F4] p-6 text-[#14181B]">
      <div className="space-y-4">
        <header className="flex items-center justify-between">
          <div className="flex items-center gap-2.5">
            <HostFacingBracketsIcon
              state={snapshot.tray_state}
              className="h-7 w-7 text-[#14181B]"
              label={`Host status: ${snapshot.tray_state}`}
            />
            <h1 className="font-display text-xl font-semibold tracking-[-0.02em] text-[#14181B]">
              Oudos Host Agent
            </h1>
          </div>
          <span className="inline-flex items-center gap-1.5 rounded-full bg-[#E3F1EE] px-3 py-1 text-sm font-medium text-[#074A40]">
            <ShieldCheck className="h-4 w-4" aria-hidden="true" />
            Asks-First v0.1
          </span>
        </header>

        {snapshot.active_banner ? (
          <PersistentSessionBanner
            banner={snapshot.active_banner}
            onEndSession={handleEndSession}
            onRevokeInput={handleRevokeInput}
          />
        ) : (
          <section className="rounded-xl border border-[#DADDD8] bg-white p-4">
            <h2 className="font-display text-base font-semibold text-[#14181B]">
              {snapshot.tray_state === "alert" ? "Connection blocked" : "Waiting for requests"}
            </h2>
            <p className="mt-1 text-sm text-[#4A524E]">
              {snapshot.alert_message ??
                "Nobody can view or control your computer without your explicit yes on this screen."}
            </p>
          </section>
        )}

        <section
          aria-label="Tray icon states"
          className="rounded-xl border border-[#DADDD8] bg-white p-4"
        >
          <h2 className="font-display text-sm font-semibold text-[#14181B]">
            Monochrome tray indicators
          </h2>
          <div className="mt-2.5 grid grid-cols-3 gap-2 text-sm text-[#4A524E]">
            <div className="flex items-center gap-2 rounded-lg border border-[#DADDD8] px-2.5 py-2">
              <HostFacingBracketsIcon state="idle" className="h-5 w-5 text-[#14181B]" />
              <span>Idle</span>
            </div>
            <div className="flex items-center gap-2 rounded-lg border border-[#DADDD8] px-2.5 py-2">
              <HostFacingBracketsIcon state="in_session" className="h-5 w-5 text-[#14181B]" />
              <span>In Session</span>
            </div>
            <div className="flex items-center gap-2 rounded-lg border border-[#DADDD8] px-2.5 py-2">
              <HostFacingBracketsIcon state="alert" className="h-5 w-5 text-[#14181B]" />
              <span>Alert</span>
            </div>
          </div>
        </section>

        <section
          aria-label="Local host protection counters"
          className="rounded-xl border border-[#DADDD8] bg-white p-4 text-sm"
        >
          <div className="grid grid-cols-3 gap-2">
            <div>
              <p className="text-[#4A524E]">Audit entries</p>
              <p className="font-mono-code text-base font-semibold text-[#14181B]">
                {snapshot.audit_entry_count}
              </p>
            </div>
            <div>
              <p className="text-[#4A524E]">Blocked input</p>
              <p className="font-mono-code text-base font-semibold text-[#14181B]">
                {snapshot.blocked_input_count}
              </p>
            </div>
            <div>
              <p className="text-[#4A524E]">Dropped frames</p>
              <p className="font-mono-code text-base font-semibold text-[#14181B]">
                {snapshot.dropped_frame_count}
              </p>
            </div>
          </div>
        </section>
      </div>

      <footer className="border-t border-[#DADDD8] pt-3 text-sm text-[#4A524E]">
        Protected local consent card · Unattended access locked OFF in v0.1
      </footer>
    </main>
  );
};

export default App;
