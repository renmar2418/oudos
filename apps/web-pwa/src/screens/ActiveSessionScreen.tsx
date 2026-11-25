import React, { useState } from "react";
import {
  AlertTriangle,
  Command,
  EyeOff,
  Lock,
  Monitor,
  MousePointer,
  PhoneOff,
  RefreshCw,
  WifiOff,
} from "lucide-react";
import type {
  DeviceItem,
  GrantedPermissions,
  QualityPreset,
  SessionRecord,
} from "../types";

interface ActiveSessionScreenProps {
  device: DeviceItem;
  session: SessionRecord;
  controlPlaneOffline?: boolean;
  onRecheckHost?: () => void;
  onEndSession: (reason?: "You ended the session" | "Host went offline") => void;
}

export const ActiveSessionScreen: React.FC<ActiveSessionScreenProps> = ({
  device,
  session,
  controlPlaneOffline = false,
  onRecheckHost,
  onEndSession,
}) => {
  const [quality, setQuality] = useState<QualityPreset>("Auto");
  const [showRemoteCursor, setShowRemoteCursor] = useState(true);
  const [lastShortcut, setLastShortcut] = useState<string | null>(null);
  const latencyMs =
    quality === "Fast"
      ? Math.max(18, session.latencyMs - 9)
      : quality === "Auto"
        ? session.latencyMs
        : session.latencyMs + 16;

  const perms: GrantedPermissions = session.permissions;
  const permissionSummary = [
    "Screen",
    perms.mouseKeyboard ? "Input" : null,
    perms.clipboardSync ? "Clipboard" : null,
    perms.fileTransfer ? "Files" : null,
  ]
    .filter(Boolean)
    .join(" + ");

  const sendShortcut = (shortcut: string) => {
    setLastShortcut(`Sent ${shortcut}`);
  };

  return (
    <section
      aria-labelledby="active-session-heading"
      className="overflow-hidden rounded-2xl border border-[#2A3236] bg-[#0E1214] text-white shadow-lg"
    >
      <header className="flex flex-wrap items-center justify-between gap-3 border-b border-[#2A3236] px-4 py-3 sm:px-6">
        <div className="flex items-center gap-3">
          <span className="inline-flex items-center gap-2 rounded-full border border-[#5FD9BF]/40 bg-[#141D20] px-3 py-1 text-xs font-medium text-[#5FD9BF]">
            <Lock className="h-3.5 w-3.5" aria-hidden="true" />
            E2E Encrypted · {session.hostOfflineMidSession ? "Paused" : "Live"}
          </span>
          <div>
            <h1 id="active-session-heading" className="text-base font-semibold text-white">
              {device.alias}
            </h1>
            <p className="text-xs text-[#C4CCC8]">
              {device.serverDomain} · {permissionSummary} ·{" "}
              {session.transportMode === "direct_p2p" ? "Direct P2P" : "TURN Relay"}
            </p>
          </div>
        </div>

        <div className="flex flex-wrap items-center gap-3">
          <span className="font-mono-code text-xs text-[#5FD9BF]" aria-label="Connection latency">
            {latencyMs} ms
          </span>

          <label htmlFor="quality-select" className="sr-only">
            Stream quality preset
          </label>
          <select
            id="quality-select"
            value={quality}
            onChange={(e) => setQuality(e.target.value as QualityPreset)}
            className="rounded-lg border border-[#343E43] bg-[#161C1F] px-3 py-1.5 text-xs font-medium text-white focus:border-[#5FD9BF] focus:outline-none focus:ring-2 focus:ring-[#5FD9BF]/40"
          >
            <option value="Auto">Quality: Auto</option>
            <option value="Sharp">Quality: Sharp</option>
            <option value="Fast">Quality: Fast</option>
          </select>

          <button
            type="button"
            onClick={() =>
              onEndSession(
                session.hostOfflineMidSession ? "Host went offline" : "You ended the session"
              )
            }
            className="inline-flex items-center gap-1.5 rounded-lg bg-[#B42318] px-3.5 py-1.5 text-xs font-semibold text-white transition-colors hover:bg-[#912018] focus:outline-none focus:ring-2 focus:ring-white"
          >
            <PhoneOff className="h-3.5 w-3.5" aria-hidden="true" />
            End
          </button>
        </div>
      </header>

      {controlPlaneOffline && (
        <div
          role="status"
          className="flex items-center gap-2 border-b border-[#B54708]/40 bg-[#2A1B0E] px-4 py-2 text-xs text-[#FEF0C7] sm:px-6"
        >
          <AlertTriangle className="h-4 w-4 shrink-0 text-[#FDB022]" aria-hidden="true" />
          Control plane offline — this active P2P session remains protected until ended or expired; zero new sessions can start.
        </div>
      )}

      <div className="flex flex-wrap items-center justify-between gap-2 border-b border-[#2A3236] bg-[#13181B] px-4 py-2 sm:px-6">
        <div className="flex flex-wrap items-center gap-2">
          <span className="inline-flex items-center gap-1 text-xs font-medium text-[#C4CCC8]">
            <Command className="h-3.5 w-3.5" aria-hidden="true" />
            Send Shortcuts:
          </span>
          {(["Ctrl+Alt+Del", "Win+L", "Alt+Tab"] as const).map((shortcut) => (
            <button
              key={shortcut}
              type="button"
              disabled={!perms.mouseKeyboard || Boolean(session.hostOfflineMidSession)}
              onClick={() => sendShortcut(shortcut)}
              className="rounded-md border border-[#343E43] bg-[#1C2327] px-2.5 py-1 font-mono-code text-xs text-white hover:border-[#5FD9BF] focus:outline-none focus:ring-2 focus:ring-[#5FD9BF] disabled:cursor-not-allowed disabled:opacity-50"
            >
              {shortcut}
            </button>
          ))}
        </div>
        {lastShortcut && (
          <span className="text-xs text-[#5FD9BF]" role="status">
            {lastShortcut}
          </span>
        )}
      </div>

      <div className="grid lg:grid-cols-[1fr_280px]">
        {session.hostOfflineMidSession ? (
          <div
            role="alert"
            className="flex min-h-[360px] flex-col items-center justify-center bg-[#090C0D] p-8 text-center"
          >
            <div className="flex h-14 w-14 items-center justify-center rounded-2xl border border-[#FDB022]/40 bg-[#2A1B0E] text-[#FDB022]">
              <WifiOff className="h-7 w-7" aria-hidden="true" />
            </div>
            <h2 className="mt-4 text-lg font-semibold text-white">
              Host went offline mid-session
            </h2>
            <p className="mt-2 max-w-md text-xs text-[#C4CCC8]">
              {device.alias} disconnected at{" "}
              <span className="font-mono-code">
                {session.hostOfflineSinceUtc ?? "just now"}
              </span>
              . Screen frames and remote input are paused cleanly.
            </p>
            <div className="mt-5 flex flex-wrap justify-center gap-3">
              {onRecheckHost && (
                <button
                  type="button"
                  onClick={onRecheckHost}
                  className="inline-flex items-center gap-1.5 rounded-lg border border-[#343E43] bg-[#161C1F] px-4 py-2 text-xs font-medium text-white hover:border-[#5FD9BF]"
                >
                  <RefreshCw className="h-3.5 w-3.5" aria-hidden="true" />
                  Check reconnection
                </button>
              )}
              <button
                type="button"
                onClick={() => onEndSession("Host went offline")}
                className="rounded-lg bg-[#B42318] px-4 py-2 text-xs font-semibold text-white hover:bg-[#912018]"
              >
                End session &amp; view summary
              </button>
            </div>
          </div>
        ) : (
          <div className="flex min-h-[360px] flex-col items-center justify-center bg-[#090C0D] p-8 text-center">
            <Monitor className="h-12 w-12 text-[#5FD9BF]" aria-hidden="true" />
            <p className="mt-4 text-sm font-medium text-white">
              Remote desktop stream viewport ({quality} mode · 1080p@30fps)
            </p>
            <p className="mt-1 max-w-md text-xs text-[#C4CCC8]">
              {device.ownerName} can see the persistent top-most session banner and end this session at any time.
            </p>
          </div>
        )}

        <aside
          aria-label="Session details and local controls"
          className="border-t border-[#2A3236] bg-[#13181B] p-4 text-xs lg:border-l lg:border-t-0"
        >
          <h2 className="font-semibold uppercase tracking-wider text-[#C4CCC8]">
            Session Details
          </h2>
          <dl className="mt-3 space-y-2.5">
            <div>
              <dt className="text-[#9AA5A0]">Connected by</dt>
              <dd className="font-medium text-white">{session.requester}</dd>
            </div>
            <div>
              <dt className="text-[#9AA5A0]">Approved by</dt>
              <dd className="font-medium text-white">{session.approver}</dd>
            </div>
            <div>
              <dt className="text-[#9AA5A0]">Host-granted permissions</dt>
              <dd className="mt-1 space-y-1 text-[#C4CCC8]">
                <div>Screen view: Allowed</div>
                <div>
                  Mouse &amp; keyboard: {perms.mouseKeyboard ? "Allowed" : "Not allowed"}
                </div>
                <div>
                  Clipboard sync: {perms.clipboardSync ? "Allowed" : "Not allowed"}
                </div>
                <div>
                  File transfer: {perms.fileTransfer ? "Allowed" : "Not allowed"}
                </div>
              </dd>
            </div>
          </dl>

          <div className="mt-5 space-y-3 border-t border-[#2A3236] pt-4">
            <button
              type="button"
              role="switch"
              aria-checked={showRemoteCursor}
              onClick={() => setShowRemoteCursor((prev) => !prev)}
              className="flex w-full items-center justify-between rounded-lg border border-[#343E43] bg-[#1C2327] px-3 py-2 text-left text-xs font-medium text-white hover:border-[#5FD9BF]"
            >
              <span className="inline-flex items-center gap-1.5">
                <MousePointer className="h-3.5 w-3.5 text-[#5FD9BF]" aria-hidden="true" />
                Show remote cursor
              </span>
              <span className="text-[#5FD9BF]">{showRemoteCursor ? "On" : "Off"}</span>
            </button>

            <div className="rounded-lg border border-[#343E43] bg-[#161C1F] p-3 text-[#C4CCC8]">
              <div className="flex items-center justify-between">
                <span className="inline-flex items-center gap-1.5 font-medium text-white">
                  <EyeOff className="h-3.5 w-3.5 text-[#C4CCC8]" aria-hidden="true" />
                  Blank remote screen
                </span>
                <span className="rounded bg-[#2A3236] px-1.5 py-0.5 text-[10px] font-semibold">
                  Disabled
                </span>
              </div>
              <p className="mt-1 text-[11px] text-[#9AA5A0]">
                Blocked in v0.1 Asks-First mode so the device owner always sees their screen.
              </p>
            </div>
          </div>
        </aside>
      </div>
    </section>
  );
};
