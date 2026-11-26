import React, { useState } from "react";
import {
  AlertTriangle,
  Clipboard,
  Command,
  EyeOff,
  FileUp,
  Lock,
  Monitor,
  MousePointer,
  PhoneOff,
  RefreshCw,
  WifiOff,
  X,
} from "lucide-react";
import type {
  ClipboardDirection,
  DeviceItem,
  FileTransferItem,
  GrantedPermissions,
  QualityPreset,
  SessionRecord,
} from "../types";

const MAX_FILE_SIZE_BYTES = 100 * 1024 * 1024;
const BLOCKED_EXTENSIONS = new Set([
  "exe",
  "bat",
  "cmd",
  "com",
  "msi",
  "ps1",
  "psm1",
  "vbs",
  "vbe",
  "js",
  "jse",
  "hta",
  "scr",
  "pif",
  "dll",
  "sys",
  "reg",
]);

function validateClientFileSpec(filename: string, sizeBytes: number): string | null {
  const trimmed = filename.trim();
  if (!trimmed || trimmed.includes("..") || trimmed.includes("/") || trimmed.includes("\\")) {
    return "Invalid file name.";
  }
  if (sizeBytes <= 0) {
    return "File size must be greater than 0 bytes.";
  }
  if (sizeBytes > MAX_FILE_SIZE_BYTES) {
    return "File exceeds the 100 MiB policy size limit.";
  }
  const dot = trimmed.lastIndexOf(".");
  if (dot >= 0) {
    const ext = trimmed.slice(dot + 1).toLowerCase();
    if (BLOCKED_EXTENSIONS.has(ext)) {
      return `Files of type .${ext} are blocked for safety.`;
    }
  }
  return null;
}

interface ActiveSessionScreenProps {
  device: DeviceItem;
  session: SessionRecord;
  controlPlaneOffline?: boolean;
  onRecheckHost?: () => void;
  onClipboardSync?: (direction: ClipboardDirection, byteLen: number) => void;
  onFileTransferComplete?: (filename: string, sizeBytes: number) => void;
  onEndSession: (reason?: "You ended the session" | "Host went offline") => void;
}

export const ActiveSessionScreen: React.FC<ActiveSessionScreenProps> = ({
  device,
  session,
  controlPlaneOffline = false,
  onRecheckHost,
  onClipboardSync,
  onFileTransferComplete,
  onEndSession,
}) => {
  const [quality, setQuality] = useState<QualityPreset>("Auto");
  const [showRemoteCursor, setShowRemoteCursor] = useState(true);
  const [lastShortcut, setLastShortcut] = useState<string | null>(null);
  const [clipboardDirection, setClipboardDirection] =
    useState<ClipboardDirection>("host_to_viewer");
  const [clipboardDraft, setClipboardDraft] = useState("");
  const [clipboardStatus, setClipboardStatus] = useState<string | null>(null);
  const [transfers, setTransfers] = useState<FileTransferItem[]>([]);
  const [transferError, setTransferError] = useState<string | null>(null);

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

  const handleClipboardSend = () => {
    if (!perms.clipboardSync || session.hostOfflineMidSession) return;
    if (clipboardDirection === "disabled" || clipboardDirection === "host_to_viewer") {
      setClipboardStatus("Set direction to Viewer -> Host or Bidirectional to send text.");
      return;
    }
    if (!clipboardDraft.trim()) {
      setClipboardStatus("Enter text to sync.");
      return;
    }
    const byteLen = new TextEncoder().encode(clipboardDraft).length;
    onClipboardSync?.(clipboardDirection, byteLen);
    setClipboardDraft("");
    setClipboardStatus(`Clipboard synced (${byteLen} B metadata logged)`);
  };

  const handleQueueFile = (file: File) => {
    setTransferError(null);
    if (!perms.fileTransfer || session.hostOfflineMidSession) {
      setTransferError("File transfer is disabled by the host.");
      return;
    }
    const err = validateClientFileSpec(file.name, file.size);
    if (err) {
      setTransferError(err);
      return;
    }
    const id = `tx-${Date.now()}-${Math.floor(Math.random() * 1000)}`;
    const initialChunk = Math.min(file.size, Math.max(1, Math.floor(file.size / 2)));
    const item: FileTransferItem = {
      transferId: id,
      filename: file.name,
      sizeBytes: file.size,
      transferredBytes: initialChunk,
      status: "in_progress",
      statusMessage: "Chunking over E2E data channel",
    };
    setTransfers((prev) => [item, ...prev.slice(0, 3)]);
  };

  const handleAdvanceOrCompleteTransfer = (transferId: string) => {
    const target = transfers.find((t) => t.transferId === transferId);
    if (!target || target.status !== "in_progress") return;
    setTransfers((prev) =>
      prev.map((t) =>
        t.transferId === transferId
          ? {
              ...t,
              transferredBytes: t.sizeBytes,
              status: "completed",
              statusMessage: "Completed (metadata logged)",
            }
          : t
      )
    );
    onFileTransferComplete?.(target.filename, target.sizeBytes);
  };

  const handleCancelTransfer = (transferId: string) => {
    setTransfers((prev) =>
      prev.map((t) =>
        t.transferId === transferId && t.status === "in_progress"
          ? {
              ...t,
              status: "cancelled",
              statusMessage: "Cancelled mid-transfer",
            }
          : t
      )
    );
  };

  return (
    <section
      aria-labelledby="active-session-heading"
      className="flex min-w-0 flex-1 flex-col bg-[#0E1214] text-white"
    >
      <header className="flex flex-wrap items-center justify-between gap-3 border-b border-[#252F34] bg-[#0E1214] px-4 py-2.5 sm:px-6">
        <div className="flex w-full min-w-0 flex-col gap-1 sm:w-auto sm:flex-1 sm:flex-row sm:flex-wrap sm:items-center sm:gap-3">
          <span className="inline-flex shrink-0 items-center gap-1.5 font-mono-code text-xs font-semibold text-[#5FD9BF]">
            <span className="h-2 w-2 rounded-full bg-[#5FD9BF]" aria-hidden="true" />
            <Lock className="h-3.5 w-3.5" aria-hidden="true" />
            E2E Encrypted · {session.hostOfflineMidSession ? "Paused" : "Live"}
          </span>
          <span className="hidden h-4 w-px bg-[#252F34] lg:inline-block" aria-hidden="true" />
          <div className="min-w-0">
            <h1
              id="active-session-heading"
              className="truncate text-sm font-semibold text-white"
            >
              {device.alias}
            </h1>
            <p className="truncate font-mono-code text-xs text-[#9FB0AA]">
              {device.serverDomain} · {permissionSummary} ·{" "}
              {session.transportMode === "direct_p2p" ? "Direct P2P" : "TURN Relay"}
            </p>
          </div>
        </div>

        <div className="flex shrink-0 flex-wrap items-center gap-2.5">
          <span
            className="font-mono-code text-xs font-semibold text-[#5FD9BF]"
            aria-label="Connection latency"
          >
            {latencyMs} ms
          </span>

          <label htmlFor="quality-select" className="sr-only">
            Stream quality preset
          </label>
          <select
            id="quality-select"
            value={quality}
            onChange={(e) => setQuality(e.target.value as QualityPreset)}
            className="min-h-[40px] rounded-md border border-[#2E383D] bg-[#141D20] px-2.5 py-1.5 text-xs font-medium text-white focus:border-[#5FD9BF] focus:outline-none focus:ring-2 focus:ring-[#5FD9BF]/40 sm:min-h-[34px]"
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
            className="inline-flex min-h-[40px] items-center gap-1.5 rounded-md bg-[#B42318] px-3.5 py-1.5 text-xs font-semibold text-white transition-colors hover:bg-[#912018] focus:outline-none focus:ring-2 focus:ring-white sm:min-h-[34px]"
          >
            <PhoneOff className="h-3.5 w-3.5" aria-hidden="true" />
            End
          </button>
        </div>
      </header>

      {controlPlaneOffline && (
        <div
          role="status"
          className="flex items-center gap-2 border-b border-[#B54708]/50 bg-[#2A1B0E] px-4 py-2 text-xs text-[#FEF0C7] sm:px-6"
        >
          <AlertTriangle className="h-4 w-4 shrink-0 text-[#FDB022]" aria-hidden="true" />
          Control plane offline — this active P2P session remains protected until ended or expired; zero new sessions can start.
        </div>
      )}

      <div className="flex flex-wrap items-center justify-between gap-2 border-b border-[#252F34] bg-[#141D20] px-4 py-2 sm:px-6 sm:py-1.5">
        <div className="flex min-w-0 flex-wrap items-center gap-1.5">
          <span className="mr-1 inline-flex items-center gap-1 text-xs font-medium text-[#A8B4B0]">
            <Command className="h-3.5 w-3.5" aria-hidden="true" />
            Send Shortcuts:
          </span>
          {(["Ctrl+Alt+Del", "Win+L", "Alt+Tab"] as const).map((shortcut) => (
            <button
              key={shortcut}
              type="button"
              disabled={!perms.mouseKeyboard || Boolean(session.hostOfflineMidSession)}
              onClick={() => sendShortcut(shortcut)}
              className="min-h-[40px] rounded border border-[#2E383D] bg-[#1C2327] px-2.5 py-1 font-mono-code text-xs text-white transition-colors hover:border-[#5FD9BF] focus:outline-none focus:ring-2 focus:ring-[#5FD9BF] disabled:cursor-not-allowed disabled:opacity-45 sm:min-h-[30px]"
            >
              {shortcut}
            </button>
          ))}
          {lastShortcut && (
            <span className="ml-2 font-mono-code text-xs text-[#5FD9BF]" role="status">
              {lastShortcut}
            </span>
          )}
        </div>

        <button
          type="button"
          role="switch"
          aria-checked={showRemoteCursor}
          onClick={() => setShowRemoteCursor((prev) => !prev)}
          className="inline-flex min-h-[40px] items-center gap-2 rounded border border-[#2E383D] bg-[#1C2327] px-2.5 py-1 text-xs font-medium text-white transition-colors hover:border-[#5FD9BF] focus:outline-none focus:ring-2 focus:ring-[#5FD9BF] sm:min-h-[30px]"
        >
          <MousePointer className="h-3.5 w-3.5 text-[#5FD9BF]" aria-hidden="true" />
          <span>Remote cursor</span>
          <span className="font-mono-code text-[#5FD9BF]">
            {showRemoteCursor ? "On" : "Off"}
          </span>
        </button>
      </div>

      <div className="grid min-w-0 flex-1 lg:grid-cols-[minmax(0,1fr)_300px]">
        {session.hostOfflineMidSession ? (
          <div
            role="alert"
            className="flex min-h-[260px] min-w-0 flex-col items-center justify-center bg-[#090C0D] p-6 text-center sm:min-h-[360px] sm:p-8 lg:min-h-[460px]"
          >
            <div className="flex h-12 w-12 items-center justify-center rounded-md border border-[#FDB022]/40 bg-[#2A1B0E] text-[#FDB022]">
              <WifiOff className="h-6 w-6" aria-hidden="true" />
            </div>
            <h2 className="mt-4 text-lg font-semibold text-white">
              Host went offline mid-session
            </h2>
            <p className="mt-2 max-w-md text-xs leading-relaxed text-[#C4CCC8]">
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
                  className="inline-flex min-h-[40px] items-center gap-1.5 rounded-md border border-[#2E383D] bg-[#141D20] px-4 py-2 text-xs font-medium text-white hover:border-[#5FD9BF] sm:min-h-[34px]"
                >
                  <RefreshCw className="h-3.5 w-3.5" aria-hidden="true" />
                  Check reconnection
                </button>
              )}
              <button
                type="button"
                onClick={() => onEndSession("Host went offline")}
                className="min-h-[40px] rounded-md bg-[#B42318] px-4 py-2 text-xs font-semibold text-white hover:bg-[#912018] sm:min-h-[34px]"
              >
                End session &amp; view summary
              </button>
            </div>
          </div>
        ) : (
          <div className="flex min-h-[260px] min-w-0 flex-col items-center justify-center bg-[#090C0D] p-6 text-center sm:min-h-[360px] sm:p-8 lg:min-h-[460px]">
            <Monitor className="h-11 w-11 text-[#5FD9BF]" aria-hidden="true" />
            <p className="mt-4 text-sm font-medium text-white">
              Remote desktop stream viewport ({quality} mode · 1080p@30fps)
            </p>
            <p className="mt-1.5 max-w-md text-xs leading-relaxed text-[#A8B4B0]">
              {device.ownerName} can see the persistent top-most session banner and end this session at any time.
            </p>
          </div>
        )}

        <aside
          aria-label="Session details and local controls"
          className="grid min-w-0 divide-y divide-[#252F34] border-t border-[#252F34] bg-[#13181B] text-xs md:grid-cols-2 md:divide-x md:divide-y-0 lg:block lg:divide-x-0 lg:divide-y lg:border-l lg:border-t-0"
        >
          <div className="p-4 md:row-span-2 lg:row-span-1">
            <h2 className="text-xs font-semibold text-[#A8B4B0]">Session details</h2>
            <dl className="mt-3 space-y-2">
              <div className="flex items-baseline justify-between gap-2">
                <dt className="text-[#9AA5A0]">Connected by</dt>
                <dd className="truncate font-mono-code font-medium text-white">
                  {session.requester}
                </dd>
              </div>
              <div className="flex items-baseline justify-between gap-2">
                <dt className="text-[#9AA5A0]">Approved by</dt>
                <dd className="font-medium text-white">{session.approver}</dd>
              </div>
            </dl>

            <div className="mt-3.5 border-t border-[#252F34] pt-3">
              <p className="text-[11px] font-medium text-[#9AA5A0]">
                Host-granted permissions
              </p>
              <ul className="mt-2 space-y-1.5 text-xs">
                <li className="flex items-center justify-between">
                  <span className="text-[#D2DAD7]">Screen view</span>
                  <span className="font-mono-code text-[#5FD9BF]">Allowed</span>
                </li>
                <li className="flex items-center justify-between">
                  <span className="text-[#D2DAD7]">Mouse &amp; keyboard</span>
                  <span
                    className={`font-mono-code ${
                      perms.mouseKeyboard ? "text-[#5FD9BF]" : "text-[#9AA5A0]"
                    }`}
                  >
                    {perms.mouseKeyboard ? "Allowed" : "Off"}
                  </span>
                </li>
                <li className="flex items-center justify-between">
                  <span className="text-[#D2DAD7]">Clipboard sync</span>
                  <span
                    className={`font-mono-code ${
                      perms.clipboardSync ? "text-[#5FD9BF]" : "text-[#9AA5A0]"
                    }`}
                  >
                    {perms.clipboardSync ? "Allowed" : "Off"}
                  </span>
                </li>
                <li className="flex items-center justify-between">
                  <span className="text-[#D2DAD7]">File transfer</span>
                  <span
                    className={`font-mono-code ${
                      perms.fileTransfer ? "text-[#5FD9BF]" : "text-[#9AA5A0]"
                    }`}
                  >
                    {perms.fileTransfer ? "Allowed" : "Off"}
                  </span>
                </li>
              </ul>
            </div>

            <div className="mt-3.5 border-t border-[#252F34] pt-3 text-[#A8B4B0]">
              <div className="flex items-center justify-between">
                <span className="inline-flex items-center gap-1.5 font-medium text-white">
                  <EyeOff className="h-3.5 w-3.5 text-[#A8B4B0]" aria-hidden="true" />
                  Blank remote screen
                </span>
                <span className="font-mono-code text-[11px] text-[#9AA5A0]">Disabled</span>
              </div>
              <p className="mt-1 text-[11px] text-[#9AA5A0]">
                Disabled so the device owner always sees their screen.
              </p>
            </div>
          </div>

          <div className="p-4 text-[#C4CCC8] md:border-t-0 lg:border-t">
            <div className="flex items-center justify-between gap-2">
              <span className="inline-flex items-center gap-1.5 font-semibold text-white">
                <Clipboard className="h-3.5 w-3.5 text-[#5FD9BF]" aria-hidden="true" />
                Clipboard sync
              </span>
              <span
                className={`font-mono-code text-[11px] font-semibold ${
                  perms.clipboardSync ? "text-[#5FD9BF]" : "text-[#9AA5A0]"
                }`}
              >
                {perms.clipboardSync ? "Granted" : "Off"}
              </span>
            </div>

            {perms.clipboardSync ? (
              <div className="mt-3 space-y-2">
                <label htmlFor="clipboard-direction" className="block text-[11px] text-[#A8B4B0]">
                  Direction
                </label>
                <select
                  id="clipboard-direction"
                  value={clipboardDirection}
                  onChange={(e) =>
                    setClipboardDirection(e.target.value as ClipboardDirection)
                  }
                  className="min-h-[40px] w-full rounded border border-[#2E383D] bg-[#0E1214] px-2.5 py-1.5 text-xs text-white focus:border-[#5FD9BF] focus:outline-none lg:min-h-[32px]"
                >
                  <option value="host_to_viewer">Host to viewer only</option>
                  <option value="viewer_to_host">Viewer to host only</option>
                  <option value="bidirectional">Bidirectional</option>
                  <option value="disabled">Paused</option>
                </select>
                <label htmlFor="clipboard-text-input" className="sr-only">
                  Clipboard text to send
                </label>
                <input
                  id="clipboard-text-input"
                  type="text"
                  value={clipboardDraft}
                  onChange={(e) => setClipboardDraft(e.target.value)}
                  placeholder="Plain text only (64 KiB max)"
                  disabled={
                    clipboardDirection === "disabled" ||
                    clipboardDirection === "host_to_viewer" ||
                    Boolean(session.hostOfflineMidSession)
                  }
                  className="min-h-[40px] w-full rounded border border-[#2E383D] bg-[#0E1214] px-2.5 py-1.5 text-xs text-white placeholder-[#8A938E] focus:border-[#5FD9BF] focus:outline-none disabled:opacity-50 lg:min-h-[32px]"
                />
                <button
                  type="button"
                  onClick={handleClipboardSend}
                  disabled={
                    clipboardDirection === "disabled" ||
                    clipboardDirection === "host_to_viewer" ||
                    Boolean(session.hostOfflineMidSession)
                  }
                  className="min-h-[40px] w-full rounded bg-[#0F5C52] px-2.5 py-1.5 text-xs font-semibold text-white hover:bg-[#0B473F] disabled:cursor-not-allowed disabled:opacity-50 lg:min-h-[32px]"
                >
                  Send clipboard text
                </button>
                {clipboardStatus && (
                  <p role="status" className="text-[11px] text-[#5FD9BF]">
                    {clipboardStatus}
                  </p>
                )}
              </div>
            ) : (
              <p className="mt-1.5 text-[11px] text-[#9AA5A0]">
                Clipboard sync is off by default. The host owner must allow it on their consent card.
              </p>
            )}
          </div>

          <div className="p-4 text-[#C4CCC8] md:border-t lg:border-t">
            <div className="flex items-center justify-between gap-2">
              <span className="inline-flex items-center gap-1.5 font-semibold text-white">
                <FileUp className="h-3.5 w-3.5 text-[#5FD9BF]" aria-hidden="true" />
                File transfer
              </span>
              <span
                className={`font-mono-code text-[11px] font-semibold ${
                  perms.fileTransfer ? "text-[#5FD9BF]" : "text-[#9AA5A0]"
                }`}
              >
                {perms.fileTransfer ? "Granted" : "Off"}
              </span>
            </div>

            {perms.fileTransfer ? (
              <div className="mt-3 space-y-2">
                <label
                  htmlFor="session-file-input"
                  className="block text-[11px] text-[#A8B4B0]"
                >
                  Select file (100 MiB max, executables blocked)
                </label>
                <input
                  id="session-file-input"
                  type="file"
                  disabled={Boolean(session.hostOfflineMidSession)}
                  onChange={(e) => {
                    const file = e.target.files?.[0];
                    if (file) handleQueueFile(file);
                    e.target.value = "";
                  }}
                  className="block w-full text-[11px] text-[#C4CCC8] file:mr-2 file:rounded file:border-0 file:bg-[#0F5C52] file:px-2.5 file:py-1 file:text-xs file:font-semibold file:text-white hover:file:bg-[#0B473F]"
                />
                {transferError && (
                  <p role="alert" className="text-[11px] text-[#FDA29B]">
                    {transferError}
                  </p>
                )}
                {transfers.map((item) => {
                  const pct = Math.min(
                    100,
                    Math.round((item.transferredBytes / Math.max(1, item.sizeBytes)) * 100)
                  );
                  return (
                    <div
                      key={item.transferId}
                      className="border border-[#2E383D] bg-[#0E1214] p-2.5"
                    >
                      <div className="flex items-center justify-between gap-2">
                        <span className="truncate font-medium text-white">{item.filename}</span>
                        <span className="font-mono-code text-[11px] text-[#5FD9BF]">{pct}%</span>
                      </div>
                      <div className="mt-1.5 h-1.5 w-full overflow-hidden bg-[#252F34]">
                        <div
                          className="h-full bg-[#5FD9BF]"
                          style={{ width: `${pct}%` }}
                        />
                      </div>
                      <div className="mt-2 flex items-center justify-between gap-2">
                        <span className="text-[11px] text-[#A8B4B0]">
                          {item.statusMessage ?? item.status}
                        </span>
                        {item.status === "in_progress" && (
                          <div className="flex items-center gap-1.5">
                            <button
                              type="button"
                              onClick={() => handleAdvanceOrCompleteTransfer(item.transferId)}
                              className="rounded bg-[#0F5C52] px-2 py-0.5 text-[11px] font-semibold text-white hover:bg-[#0B473F]"
                            >
                              Complete
                            </button>
                            <button
                              type="button"
                              onClick={() => handleCancelTransfer(item.transferId)}
                              aria-label={`Cancel ${item.filename}`}
                              className="inline-flex items-center gap-0.5 rounded bg-[#B42318] px-2 py-0.5 text-[11px] font-semibold text-white hover:bg-[#912018]"
                            >
                              <X className="h-3 w-3" aria-hidden="true" />
                              Cancel
                            </button>
                          </div>
                        )}
                      </div>
                    </div>
                  );
                })}
              </div>
            ) : (
              <p className="mt-1.5 text-[11px] text-[#9AA5A0]">
                File transfer is off by default. Executable files are blocked even when enabled.
              </p>
            )}
          </div>
        </aside>
      </div>
    </section>
  );
};
