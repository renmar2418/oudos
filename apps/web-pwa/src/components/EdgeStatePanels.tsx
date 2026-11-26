import React, { useState } from "react";
import {
  AlertTriangle,
  CheckCircle2,
  Clock,
  KeyRound,
  MonitorOff,
  RefreshCw,
  ShieldAlert,
  WifiOff,
  XCircle,
} from "lucide-react";
import type {
  DeviceItem,
  ScreenUnavailableReason,
  SessionRecord,
} from "../types";
import { FacingBracketsIcon } from "./FacingBracketsIcon";

const SCREEN_UNAVAILABLE_DETAILS: Record<
  ScreenUnavailableReason,
  { title: string; summary: string; steps: string[] }
> = {
  locked: {
    title: "Screen locked on host PC",
    summary:
      "The Windows lock screen is currently active. In Asks-First mode, screen capture pauses at the lock screen.",
    steps: [
      "Ask the device owner to sign in on their Windows PC.",
      "Once unlocked, click Check availability again to start the request.",
    ],
  },
  asleep: {
    title: "Device display is asleep",
    summary:
      "The host PC or monitor entered sleep mode, so no desktop frames are available.",
    steps: [
      "Ask the owner to move their mouse or press a key to wake the display.",
      "Or use Notify me when online from the Devices list.",
    ],
  },
  permission_missing: {
    title: "Screen capture permission missing",
    summary:
      "The host agent cannot access Windows Graphics Capture / Desktop Duplication on the active display session.",
    steps: [
      "Ask the owner to click the Oudos tray icon and choose Check Screen Capture Permission.",
      "Ensure the owner is signed into an interactive desktop session (not RDP-disconnected).",
      "Click Check availability again once enabled.",
    ],
  },
  no_display: {
    title: "No active monitor detected",
    summary:
      "The host computer does not report an active display output to capture.",
    steps: [
      "Ask the owner to power on or reconnect an external monitor.",
      "Click Check availability again once a display is active.",
    ],
  },
};

export const ScreenUnavailableCard: React.FC<{
  device: DeviceItem;
  reason: ScreenUnavailableReason;
  onChangeReason?: (reason: ScreenUnavailableReason) => void;
  onRetry: () => void;
  onBack: () => void;
}> = ({ device, reason, onChangeReason, onRetry, onBack }) => {
  const details = SCREEN_UNAVAILABLE_DETAILS[reason];

  return (
    <section
      aria-labelledby="screen-unavailable-heading"
      className="mx-auto max-w-xl border border-[#DADDD8] bg-white divide-y divide-[#DADDD8]"
    >
      <div className="flex items-center gap-3 bg-[#FEF0C7]/60 px-6 py-4">
        <MonitorOff className="h-5 w-5 shrink-0 text-[#B54708]" aria-hidden="true" />
        <h1
          id="screen-unavailable-heading"
          className="text-lg font-semibold text-[#111618]"
        >
          {details.title}
        </h1>
      </div>

      <div className="space-y-4 px-6 py-5">
        <p className="text-sm text-[#525D63]">
          <strong className="text-[#111618]">{device.alias}</strong> (
          <span className="font-mono-code">{device.deviceId}</span>) cannot share its screen right now.{" "}
          {details.summary}
        </p>

        <div className="border border-[#DADDD8] bg-[#F5F6F4] p-4">
          <h2 className="text-xs font-semibold text-[#111618]">How to fix it</h2>
          <ol className="mt-2 list-decimal space-y-1.5 pl-5 text-sm text-[#525D63]">
            {details.steps.map((stepText) => (
              <li key={stepText}>{stepText}</li>
            ))}
          </ol>
        </div>

        {onChangeReason && (
          <div className="flex flex-wrap items-center gap-2 text-xs">
            <span className="font-medium text-[#525D63]">Cause reported by host:</span>
            {(["permission_missing", "locked", "asleep", "no_display"] as const).map(
              (r) => (
                <button
                  key={r}
                  type="button"
                  aria-pressed={reason === r}
                  onClick={() => onChangeReason(r)}
                  className={`rounded-md border px-2.5 py-1 font-medium ${
                    reason === r
                      ? "border-[#B54708] bg-[#FEF0C7] text-[#7A2E0E]"
                      : "border-[#DADDD8] bg-white text-[#525D63] hover:bg-[#F5F6F4]"
                  }`}
                >
                  {r.replace("_", " ")}
                </button>
              )
            )}
          </div>
        )}
      </div>

      <div className="flex flex-col gap-3 bg-[#F5F6F4] px-6 py-4 sm:flex-row">
        <button
          type="button"
          onClick={onBack}
          className="flex-1 rounded-md border border-[#DADDD8] bg-white px-4 py-2.5 text-sm font-medium text-[#111618] hover:bg-[#F5F6F4] focus:outline-none focus-visible:ring-2 focus-visible:ring-[#0F5C52]"
        >
          Back to Devices
        </button>
        <button
          type="button"
          onClick={onRetry}
          className="inline-flex flex-1 items-center justify-center gap-2 rounded-md bg-[#0F5C52] px-4 py-2.5 text-sm font-semibold text-white hover:bg-[#0B473F] focus:outline-none focus-visible:ring-2 focus-visible:ring-[#0F5C52]"
        >
          <RefreshCw className="h-4 w-4" aria-hidden="true" />
          Check availability again
        </button>
      </div>
    </section>
  );
};

export const OwnerDeniedCard: React.FC<{
  device: DeviceItem;
  streakCount: number;
  onRetry: () => void;
  onBack: () => void;
}> = ({ device, streakCount, onRetry, onBack }) => (
  <section
    aria-labelledby="owner-denied-heading"
    className="mx-auto max-w-lg border border-[#DADDD8] bg-white divide-y divide-[#DADDD8]"
  >
    <div className="flex items-center gap-3 bg-[#FEE4E2]/70 px-6 py-4">
      <XCircle className="h-5 w-5 shrink-0 text-[#B42318]" aria-hidden="true" />
      <h1 id="owner-denied-heading" className="text-lg font-semibold text-[#111618]">
        {device.ownerName} denied the connection request
      </h1>
    </div>

    <div className="space-y-4 px-6 py-5">
      <p className="text-sm text-[#525D63]">
        Your request to <strong className="text-[#111618]">{device.alias}</strong> (
        <span className="font-mono-code">{device.deviceId}</span>) was declined by the owner. No screen frames, input, or clipboard data were shared, and the denial was recorded in the audit log.
      </p>

      <div className="border border-[#DADDD8] bg-[#F5F6F4] p-3.5 text-xs text-[#525D63]">
        Anti-abuse protection: 3 consecutive denials or mismatches lock further requests to this device for 24 hours (current streak: {streakCount} of 3).
      </div>
    </div>

    <div className="flex flex-col gap-3 bg-[#F5F6F4] px-6 py-4 sm:flex-row">
      <button
        type="button"
        onClick={onBack}
        className="flex-1 rounded-md border border-[#DADDD8] bg-white px-4 py-2.5 text-sm font-medium text-[#111618] hover:bg-[#F5F6F4]"
      >
        Back to Devices
      </button>
      <button
        type="button"
        disabled={streakCount >= 3}
        onClick={onRetry}
        className="flex-1 rounded-md bg-[#0F5C52] px-4 py-2.5 text-sm font-semibold text-white hover:bg-[#0B473F] disabled:cursor-not-allowed disabled:bg-[#DCE0DC] disabled:text-[#384146]"
      >
        {streakCount >= 3 ? "Locked out (24h)" : "Request again"}
      </button>
    </div>
  </section>
);

export const RequestExpiredCard: React.FC<{
  device: DeviceItem;
  onRetry: () => void;
  onBack: () => void;
}> = ({ device, onRetry, onBack }) => (
  <section
    aria-labelledby="request-expired-heading"
    className="mx-auto max-w-lg border border-[#DADDD8] bg-white divide-y divide-[#DADDD8]"
  >
    <div className="flex items-center gap-3 bg-[#FEF0C7]/60 px-6 py-4">
      <Clock className="h-5 w-5 shrink-0 text-[#B54708]" aria-hidden="true" />
      <h1 id="request-expired-heading" className="text-lg font-semibold text-[#111618]">
        Request expired after 2 minutes
      </h1>
    </div>

    <div className="px-6 py-5">
      <p className="text-sm text-[#525D63]">
        {device.ownerName} did not approve within the 2-minute window, so the request closed automatically and nothing was shared.
      </p>
    </div>

    <div className="flex flex-col gap-3 bg-[#F5F6F4] px-6 py-4 sm:flex-row">
      <button
        type="button"
        onClick={onBack}
        className="flex-1 rounded-md border border-[#DADDD8] bg-white px-4 py-2.5 text-sm font-medium text-[#111618] hover:bg-[#F5F6F4]"
      >
        Back to Devices
      </button>
      <button
        type="button"
        onClick={onRetry}
        className="flex-1 rounded-md bg-[#0F5C52] px-4 py-2.5 text-sm font-semibold text-white hover:bg-[#0B473F]"
      >
        Send new request
      </button>
    </div>
  </section>
);

export const CodeMismatchCard: React.FC<{
  device: DeviceItem;
  streakCount: number;
  onBack: () => void;
}> = ({ device, streakCount, onBack }) => (
  <section
    aria-labelledby="code-mismatch-heading"
    role="alert"
    className="mx-auto max-w-lg border border-[#B42318] bg-white divide-y divide-[#DADDD8]"
  >
    <div className="flex items-center gap-3 bg-[#FEE4E2] px-6 py-4">
      <ShieldAlert className="h-5 w-5 shrink-0 text-[#B42318]" aria-hidden="true" />
      <h1 id="code-mismatch-heading" className="text-lg font-semibold text-[#111618]">
        6-digit verification codes did not match
      </h1>
    </div>

    <div className="space-y-4 px-6 py-5">
      <p className="text-sm text-[#525D63]">
        Connection to <strong className="text-[#111618]">{device.alias}</strong> was aborted immediately because the 6-digit verification codes did not match.
      </p>

      <div className="border border-[#B54708]/40 bg-[#FEF0C7] p-3.5 text-xs text-[#7A2E0E]">
        <strong>Why this matters:</strong> A mismatched code can mean a relay or attacker is interfering with the handshake. Do not continue if anyone asks you to ignore a mismatched code. Repeated mismatches lock requests for 24 hours (streak: {streakCount}/3).
      </div>
    </div>

    <div className="bg-[#F5F6F4] px-6 py-4">
      <button
        type="button"
        onClick={onBack}
        className="w-full rounded-md bg-[#0F5C52] px-4 py-2.5 text-sm font-semibold text-white hover:bg-[#0B473F]"
      >
        Return safely to Devices
      </button>
    </div>
  </section>
);

export const DeviceKeyChangedAlert: React.FC<{
  device: DeviceItem;
  expectedPubkey: string;
  presentedPubkey: string;
  onAcceptNewKey: (newPubkey: string) => void;
  onBack: () => void;
}> = ({ device, expectedPubkey, presentedPubkey, onAcceptNewKey, onBack }) => {
  const [showRecovery, setShowRecovery] = useState(false);
  const [confirmDigits, setConfirmDigits] = useState("");
  const expectedDigits = device.deviceId.replace(/\D/g, "");
  const enteredDigits = confirmDigits.replace(/\D/g, "");
  const canRepin = enteredDigits === expectedDigits;

  return (
    <section
      aria-labelledby="key-changed-heading"
      role="alertdialog"
      aria-modal="true"
      className="mx-auto max-w-xl border-2 border-[#B42318] bg-white divide-y divide-[#DADDD8]"
    >
      <div className="flex items-center gap-3 bg-[#FEE4E2] px-6 py-4">
        <span className="text-[#B42318]">
          <FacingBracketsIcon state="alert" className="h-7 w-7" label="Security alert" />
        </span>
        <h1 id="key-changed-heading" className="text-lg font-semibold text-[#111618]">
          Device key changed for {device.alias}
        </h1>
      </div>

      <div className="space-y-4 px-6 py-5">
        <p className="text-sm text-[#525D63]">
          The cryptographic identity key presented by{" "}
          <strong className="text-[#111618]">{device.alias}</strong> does not match the Ed25519 key saved when you first paired this device. Connection is halted until you verify what changed.
        </p>

        <div className="divide-y divide-[#DADDD8] border border-[#DADDD8] bg-[#F5F6F4] text-xs">
          <div className="p-3.5">
            <p className="font-semibold text-[#111618]">Saved (pinned) key:</p>
            <p className="mt-0.5 break-all font-mono-code text-[#525D63]">
              {expectedPubkey}
            </p>
          </div>
          <div className="p-3.5">
            <p className="font-semibold text-[#B42318]">New presented key:</p>
            <p className="mt-0.5 break-all font-mono-code text-[#B42318]">
              {presentedPubkey}
            </p>
          </div>
        </div>

        <div className="border border-[#B54708]/40 bg-[#FEF0C7] p-3.5 text-xs text-[#7A2E0E]">
          <span className="inline-flex items-center gap-1.5 font-semibold">
            <AlertTriangle className="h-4 w-4 text-[#B54708]" aria-hidden="true" />
            Check with {device.ownerName} out-of-band before continuing
          </span>
          <p className="mt-1">
            A key change happens if Windows or the Oudos agent was reinstalled, OR if an attacker is impersonating the device.
          </p>
        </div>
      </div>

      <div className="space-y-3 bg-[#F5F6F4] px-6 py-4">
        <button
          type="button"
          onClick={onBack}
          className="w-full rounded-md bg-[#B42318] px-4 py-2.5 text-sm font-semibold text-white hover:bg-[#912018] focus:outline-none focus-visible:ring-2 focus-visible:ring-[#B42318]"
        >
          Keep blocked &amp; return to Devices
        </button>

        {!showRecovery ? (
          <button
            type="button"
            onClick={() => setShowRecovery(true)}
            className="w-full rounded-md border border-[#DADDD8] bg-white px-4 py-2 text-xs font-medium text-[#525D63] hover:bg-[#F5F6F4] hover:text-[#111618]"
          >
            I verified this key change directly with {device.ownerName}
          </button>
        ) : (
          <div className="space-y-3 border border-[#DADDD8] bg-white p-4">
            <label
              htmlFor="repin-confirm-id"
              className="block text-xs font-semibold text-[#111618]"
            >
              Type the 9-digit Device ID ({device.deviceId}) to accept &amp; re-pin the new key
            </label>
            <input
              id="repin-confirm-id"
              type="text"
              value={confirmDigits}
              onChange={(e) => setConfirmDigits(e.target.value)}
              placeholder={device.deviceId}
              className="w-full rounded-md border border-[#DADDD8] bg-white px-3 py-2 font-mono-code text-sm text-[#111618] focus:border-[#0F5C52] focus:outline-none focus:ring-2 focus:ring-[#0F5C52]"
            />
            <button
              type="button"
              disabled={!canRepin}
              onClick={() => onAcceptNewKey(presentedPubkey)}
              className="inline-flex w-full items-center justify-center gap-2 rounded-md bg-[#0F5C52] px-4 py-2 text-sm font-semibold text-white hover:bg-[#0B473F] disabled:cursor-not-allowed disabled:bg-[#DCE0DC] disabled:text-[#384146]"
            >
              <KeyRound className="h-4 w-4" aria-hidden="true" />
              Accept new key &amp; re-pin device
            </button>
          </div>
        )}
      </div>
    </section>
  );
};

export const ControlPlaneOfflineCard: React.FC<{
  reason: string;
  onRetry: () => void;
  onBack: () => void;
}> = ({ reason, onRetry, onBack }) => (
  <section
    aria-labelledby="cp-offline-heading"
    role="alert"
    className="mx-auto max-w-lg border border-[#DADDD8] bg-white divide-y divide-[#DADDD8]"
  >
    <div className="flex items-center gap-3 bg-[#FEF0C7]/60 px-6 py-4">
      <WifiOff className="h-5 w-5 shrink-0 text-[#B54708]" aria-hidden="true" />
      <h1 id="cp-offline-heading" className="text-lg font-semibold text-[#111618]">
        Control plane unreachable
      </h1>
    </div>

    <div className="px-6 py-5">
      <p className="text-sm text-[#525D63]">
        Zero new sessions can start while the signaling server is offline ({reason}). Any already-active peer-to-peer sessions remain running until ended.
      </p>
    </div>

    <div className="flex flex-col gap-3 bg-[#F5F6F4] px-6 py-4 sm:flex-row">
      <button
        type="button"
        onClick={onBack}
        className="flex-1 rounded-md border border-[#DADDD8] bg-white px-4 py-2.5 text-sm font-medium text-[#111618] hover:bg-[#F5F6F4]"
      >
        Back to Devices
      </button>
      <button
        type="button"
        onClick={onRetry}
        className="flex-1 rounded-md bg-[#0F5C52] px-4 py-2.5 text-sm font-semibold text-white hover:bg-[#0B473F]"
      >
        Retry connection
      </button>
    </div>
  </section>
);

export const SessionEndedSummary: React.FC<{
  session: SessionRecord;
  onViewAudit: () => void;
  onBackToDevices: () => void;
}> = ({ session, onViewAudit, onBackToDevices }) => {
  const grantedList = [
    "Screen view",
    session.permissions.mouseKeyboard ? "Mouse & keyboard" : null,
    session.permissions.clipboardSync ? "Clipboard sync" : null,
    session.permissions.fileTransfer ? "File transfer" : null,
  ].filter(Boolean);

  return (
    <section
      aria-labelledby="session-ended-heading"
      className="mx-auto max-w-xl border border-[#DADDD8] bg-white divide-y divide-[#DADDD8]"
    >
      <div className="flex items-center gap-3 bg-[#E6F2F0]/60 px-6 py-4">
        <CheckCircle2 className="h-5 w-5 shrink-0 text-[#0F5C52]" aria-hidden="true" />
        <h1 id="session-ended-heading" className="text-lg font-semibold text-[#111618]">
          Session with {session.deviceAlias} closed
        </h1>
      </div>

      <div className="space-y-4 px-6 py-5">
        <p className="text-sm text-[#525D63]">
          The cryptographic grant was revoked immediately and recorded in the tamper-evident hash-chained audit log.
        </p>

        <dl className="grid gap-3 border border-[#DADDD8] bg-[#F5F6F4] p-4 text-sm sm:grid-cols-2">
          <div>
            <dt className="text-xs text-[#525D63]">End reason</dt>
            <dd className="font-semibold text-[#111618]">
              {session.endReason ?? "Session ended"}
            </dd>
          </div>
          <div>
            <dt className="text-xs text-[#525D63]">Duration</dt>
            <dd className="font-mono-code font-semibold text-[#111618]">
              {session.durationLabel}
            </dd>
          </div>
          <div>
            <dt className="text-xs text-[#525D63]">Connected by</dt>
            <dd className="font-medium text-[#111618]">{session.requester}</dd>
          </div>
          <div>
            <dt className="text-xs text-[#525D63]">Approved by</dt>
            <dd className="font-medium text-[#111618]">{session.approver}</dd>
          </div>
          <div>
            <dt className="text-xs text-[#525D63]">Permissions granted</dt>
            <dd className="font-medium text-[#111618]">{grantedList.join(" · ")}</dd>
          </div>
          <div>
            <dt className="text-xs text-[#525D63]">Audit chain record</dt>
            <dd className="font-mono-code text-xs text-[#0F5C52]">
              #{session.auditSequence ?? 0} · {(session.auditEntryHash ?? "").slice(0, 14)}…
            </dd>
          </div>
        </dl>
      </div>

      <div className="flex flex-col gap-3 bg-[#F5F6F4] px-6 py-4 sm:flex-row">
        <button
          type="button"
          onClick={onViewAudit}
          className="flex-1 rounded-md border border-[#DADDD8] bg-white px-4 py-2.5 text-sm font-medium text-[#111618] hover:bg-[#F5F6F4]"
        >
          View Audit Log
        </button>
        <button
          type="button"
          onClick={onBackToDevices}
          className="flex-1 rounded-md bg-[#0F5C52] px-4 py-2.5 text-sm font-semibold text-white hover:bg-[#0B473F]"
        >
          Back to Devices
        </button>
      </div>
    </section>
  );
};
