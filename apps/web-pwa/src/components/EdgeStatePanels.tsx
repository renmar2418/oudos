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
      "The Windows lock screen is currently active. In v0.1 Asks-First mode, screen capture pauses at the lock screen.",
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
      className="mx-auto max-w-xl rounded-2xl border border-[#DADDD8] bg-white p-6 shadow-xs sm:p-8"
    >
      <div className="flex items-center gap-3">
        <div className="flex h-11 w-11 items-center justify-center rounded-xl bg-[#FEF0C7] text-[#B54708]">
          <MonitorOff className="h-6 w-6" aria-hidden="true" />
        </div>
        <div>
          <p className="text-xs font-semibold uppercase tracking-wider text-[#B54708]">
            Screen unavailable
          </p>
          <h1
            id="screen-unavailable-heading"
            className="text-xl font-semibold text-[#14181B]"
          >
            {details.title}
          </h1>
        </div>
      </div>

      <p className="mt-4 text-sm text-[#4A524E]">
        <strong className="text-[#14181B]">{device.alias}</strong> (
        <span className="font-mono-code">{device.deviceId}</span>) cannot share its screen right now. {details.summary}
      </p>

      <div className="mt-5 rounded-xl border border-[#DADDD8] bg-[#F5F6F4] p-4">
        <h2 className="text-xs font-semibold uppercase tracking-wider text-[#14181B]">
          How to fix it
        </h2>
        <ol className="mt-2 list-decimal space-y-1.5 pl-5 text-sm text-[#4A524E]">
          {details.steps.map((stepText) => (
            <li key={stepText}>{stepText}</li>
          ))}
        </ol>
      </div>

      {onChangeReason && (
        <div className="mt-4 flex flex-wrap items-center gap-2 text-xs">
          <span className="font-medium text-[#4A524E]">Cause reported by host:</span>
          {(["permission_missing", "locked", "asleep", "no_display"] as const).map(
            (r) => (
              <button
                key={r}
                type="button"
                aria-pressed={reason === r}
                onClick={() => onChangeReason(r)}
                className={`rounded-full border px-2.5 py-1 font-medium ${
                  reason === r
                    ? "border-[#B54708] bg-[#FEF0C7] text-[#B54708]"
                    : "border-[#DADDD8] bg-white text-[#4A524E] hover:bg-[#F5F6F4]"
                }`}
              >
                {r.replace("_", " ")}
              </button>
            )
          )}
        </div>
      )}

      <div className="mt-6 flex flex-col gap-3 sm:flex-row">
        <button
          type="button"
          onClick={onBack}
          className="flex-1 rounded-lg border border-[#DADDD8] bg-white px-4 py-2.5 text-sm font-medium text-[#14181B] hover:bg-[#F5F6F4] focus:outline-none focus:ring-2 focus:ring-[#0F7B6C]"
        >
          Back to Devices
        </button>
        <button
          type="button"
          onClick={onRetry}
          className="inline-flex flex-1 items-center justify-center gap-2 rounded-lg bg-[#0F7B6C] px-4 py-2.5 text-sm font-medium text-white hover:bg-[#0A6457] focus:outline-none focus:ring-2 focus:ring-[#0F7B6C]"
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
    className="mx-auto max-w-lg rounded-2xl border border-[#DADDD8] bg-white p-6 shadow-xs sm:p-8"
  >
    <div className="flex items-center gap-3">
      <div className="flex h-11 w-11 items-center justify-center rounded-xl bg-[#FEE4E2] text-[#B42318]">
        <XCircle className="h-6 w-6" aria-hidden="true" />
      </div>
      <div>
        <p className="text-xs font-semibold uppercase tracking-wider text-[#B42318]">
          Access declined
        </p>
        <h1 id="owner-denied-heading" className="text-xl font-semibold text-[#14181B]">
          {device.ownerName} denied the connection request
        </h1>
      </div>
    </div>

    <p className="mt-4 text-sm text-[#4A524E]">
      Your request to <strong className="text-[#14181B]">{device.alias}</strong> (
      <span className="font-mono-code">{device.deviceId}</span>) was declined by the owner. No screen frames, input, or clipboard data were shared, and the denial was recorded in the audit log.
    </p>

    <div className="mt-4 rounded-xl border border-[#DADDD8] bg-[#F5F6F4] p-3.5 text-xs text-[#4A524E]">
      Anti-abuse protection: 3 consecutive denials or mismatches lock further requests to this device for 24 hours (current streak: {streakCount} of 3).
    </div>

    <div className="mt-6 flex flex-col gap-3 sm:flex-row">
      <button
        type="button"
        onClick={onBack}
        className="flex-1 rounded-lg border border-[#DADDD8] bg-white px-4 py-2.5 text-sm font-medium text-[#14181B] hover:bg-[#F5F6F4]"
      >
        Back to Devices
      </button>
      <button
        type="button"
        disabled={streakCount >= 3}
        onClick={onRetry}
        className="flex-1 rounded-lg bg-[#0F7B6C] px-4 py-2.5 text-sm font-medium text-white hover:bg-[#0A6457] disabled:cursor-not-allowed disabled:bg-[#DADDD8] disabled:text-[#4A524E]"
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
    className="mx-auto max-w-lg rounded-2xl border border-[#DADDD8] bg-white p-6 shadow-xs sm:p-8"
  >
    <div className="flex items-center gap-3">
      <div className="flex h-11 w-11 items-center justify-center rounded-xl bg-[#FEF0C7] text-[#B54708]">
        <Clock className="h-6 w-6" aria-hidden="true" />
      </div>
      <div>
        <p className="text-xs font-semibold uppercase tracking-wider text-[#B54708]">
          2-minute timeout
        </p>
        <h1 id="request-expired-heading" className="text-xl font-semibold text-[#14181B]">
          Request expired after 2 minutes
        </h1>
      </div>
    </div>

    <p className="mt-4 text-sm text-[#4A524E]">
      {device.ownerName} did not approve within the 2-minute window, so the request closed automatically and nothing was shared.
    </p>

    <div className="mt-6 flex flex-col gap-3 sm:flex-row">
      <button
        type="button"
        onClick={onBack}
        className="flex-1 rounded-lg border border-[#DADDD8] bg-white px-4 py-2.5 text-sm font-medium text-[#14181B] hover:bg-[#F5F6F4]"
      >
        Back to Devices
      </button>
      <button
        type="button"
        onClick={onRetry}
        className="flex-1 rounded-lg bg-[#0F7B6C] px-4 py-2.5 text-sm font-medium text-white hover:bg-[#0A6457]"
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
    className="mx-auto max-w-lg rounded-2xl border border-[#B42318]/40 bg-white p-6 shadow-xs sm:p-8"
  >
    <div className="flex items-center gap-3">
      <div className="flex h-11 w-11 items-center justify-center rounded-xl bg-[#FEE4E2] text-[#B42318]">
        <ShieldAlert className="h-6 w-6" aria-hidden="true" />
      </div>
      <div>
        <p className="text-xs font-semibold uppercase tracking-wider text-[#B42318]">
          Security halt · SAS mismatch
        </p>
        <h1 id="code-mismatch-heading" className="text-xl font-semibold text-[#14181B]">
          6-digit verification codes did not match
        </h1>
      </div>
    </div>

    <p className="mt-4 text-sm text-[#4A524E]">
      Connection to <strong className="text-[#14181B]">{device.alias}</strong> was aborted immediately because the locally derived 6-digit verification codes did not match.
    </p>

    <div className="mt-4 rounded-xl border border-[#B54708]/30 bg-[#FEF0C7] p-4 text-xs text-[#B54708]">
      <strong>Why this matters:</strong> A mismatched code can mean a relay or attacker is interfering with the handshake. Do not continue if anyone asks you to ignore a mismatched code. Repeated mismatches lock requests for 24 hours (streak: {streakCount}/3).
    </div>

    <div className="mt-6">
      <button
        type="button"
        onClick={onBack}
        className="w-full rounded-lg bg-[#0F7B6C] px-4 py-2.5 text-sm font-medium text-white hover:bg-[#0A6457]"
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
      className="mx-auto max-w-xl rounded-2xl border-2 border-[#B42318] bg-white p-6 shadow-md sm:p-8"
    >
      <div className="flex items-start gap-3">
        <span className="mt-0.5 text-[#B42318]">
          <FacingBracketsIcon state="alert" className="h-8 w-8" label="Security alert" />
        </span>
        <div>
          <p className="text-xs font-semibold uppercase tracking-wider text-[#B42318]">
            Connection blocked · Key mismatch
          </p>
          <h1 id="key-changed-heading" className="mt-0.5 text-xl font-semibold text-[#14181B]">
            Device key changed for {device.alias}
          </h1>
        </div>
      </div>

      <p className="mt-4 text-sm text-[#4A524E]">
        The cryptographic identity key presented by <strong className="text-[#14181B]">{device.alias}</strong> does not match the Ed25519 key saved when you first paired this device. Connection is halted until you verify what changed.
      </p>

      <div className="mt-4 space-y-2.5 rounded-xl border border-[#DADDD8] bg-[#F5F6F4] p-4 text-xs">
        <div>
          <p className="font-semibold text-[#14181B]">Saved (pinned) key:</p>
          <p className="mt-0.5 break-all font-mono-code text-[#4A524E]">{expectedPubkey}</p>
        </div>
        <div>
          <p className="font-semibold text-[#B42318]">New presented key:</p>
          <p className="mt-0.5 break-all font-mono-code text-[#B42318]">{presentedPubkey}</p>
        </div>
      </div>

      <div className="mt-4 rounded-xl border border-[#B54708]/30 bg-[#FEF0C7] p-3.5 text-xs text-[#B54708]">
        <span className="inline-flex items-center gap-1.5 font-semibold">
          <AlertTriangle className="h-4 w-4" aria-hidden="true" />
          Check with {device.ownerName} out-of-band before continuing
        </span>
        <p className="mt-1">
          A key change happens if Windows or the Oudos agent was reinstalled, OR if an attacker is impersonating the device.
        </p>
      </div>

      <div className="mt-6 space-y-3">
        <button
          type="button"
          onClick={onBack}
          className="w-full rounded-lg bg-[#B42318] px-4 py-2.5 text-sm font-semibold text-white hover:bg-[#912018] focus:outline-none focus:ring-2 focus:ring-[#B42318]"
        >
          Keep blocked &amp; return to Devices
        </button>

        {!showRecovery ? (
          <button
            type="button"
            onClick={() => setShowRecovery(true)}
            className="w-full rounded-lg border border-[#DADDD8] bg-white px-4 py-2 text-xs font-medium text-[#4A524E] hover:bg-[#F5F6F4] hover:text-[#14181B]"
          >
            I verified this key change directly with {device.ownerName}
          </button>
        ) : (
          <div className="rounded-xl border border-[#DADDD8] bg-[#F5F6F4] p-4 space-y-3">
            <label
              htmlFor="repin-confirm-id"
              className="block text-xs font-semibold text-[#14181B]"
            >
              Type the 9-digit Device ID ({device.deviceId}) to accept &amp; re-pin the new key
            </label>
            <input
              id="repin-confirm-id"
              type="text"
              value={confirmDigits}
              onChange={(e) => setConfirmDigits(e.target.value)}
              placeholder={device.deviceId}
              className="w-full rounded-lg border border-[#DADDD8] bg-white px-3 py-2 font-mono-code text-sm text-[#14181B] focus:border-[#0F7B6C] focus:outline-none focus:ring-2 focus:ring-[#0F7B6C]"
            />
            <button
              type="button"
              disabled={!canRepin}
              onClick={() => onAcceptNewKey(presentedPubkey)}
              className="inline-flex w-full items-center justify-center gap-2 rounded-lg bg-[#0F7B6C] px-4 py-2 text-sm font-medium text-white hover:bg-[#0A6457] disabled:cursor-not-allowed disabled:bg-[#DADDD8] disabled:text-[#4A524E]"
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
    className="mx-auto max-w-lg rounded-2xl border border-[#DADDD8] bg-white p-6 shadow-xs sm:p-8"
  >
    <div className="flex items-center gap-3">
      <div className="flex h-11 w-11 items-center justify-center rounded-xl bg-[#FEF0C7] text-[#B54708]">
        <WifiOff className="h-6 w-6" aria-hidden="true" />
      </div>
      <div>
        <p className="text-xs font-semibold uppercase tracking-wider text-[#B54708]">
          Fail-closed protection
        </p>
        <h1 id="cp-offline-heading" className="text-xl font-semibold text-[#14181B]">
          Control plane unreachable
        </h1>
      </div>
    </div>

    <p className="mt-4 text-sm text-[#4A524E]">
      Zero new sessions can start while the signaling server is offline ({reason}). Any already-active peer-to-peer sessions remain running until ended.
    </p>

    <div className="mt-6 flex flex-col gap-3 sm:flex-row">
      <button
        type="button"
        onClick={onBack}
        className="flex-1 rounded-lg border border-[#DADDD8] bg-white px-4 py-2.5 text-sm font-medium text-[#14181B] hover:bg-[#F5F6F4]"
      >
        Back to Devices
      </button>
      <button
        type="button"
        onClick={onRetry}
        className="flex-1 rounded-lg bg-[#0F7B6C] px-4 py-2.5 text-sm font-medium text-white hover:bg-[#0A6457]"
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
      className="mx-auto max-w-xl rounded-2xl border border-[#DADDD8] bg-white p-6 shadow-xs sm:p-8"
    >
      <div className="flex items-center gap-3">
        <div className="flex h-11 w-11 items-center justify-center rounded-xl bg-[#E3F1EE] text-[#074A40]">
          <CheckCircle2 className="h-6 w-6" aria-hidden="true" />
        </div>
        <div>
          <p className="text-xs font-semibold uppercase tracking-wider text-[#074A40]">
            Session ended
          </p>
          <h1 id="session-ended-heading" className="text-xl font-semibold text-[#14181B]">
            Session with {session.deviceAlias} closed
          </h1>
        </div>
      </div>

      <p className="mt-3 text-sm text-[#4A524E]">
        The cryptographic grant was revoked immediately and recorded in the tamper-evident hash-chained audit log.
      </p>

      <dl className="mt-5 grid gap-3 rounded-xl border border-[#DADDD8] bg-[#F5F6F4] p-4 text-sm sm:grid-cols-2">
        <div>
          <dt className="text-xs text-[#4A524E]">End reason</dt>
          <dd className="font-semibold text-[#14181B]">
            {session.endReason ?? "Session ended"}
          </dd>
        </div>
        <div>
          <dt className="text-xs text-[#4A524E]">Duration</dt>
          <dd className="font-mono-code font-semibold text-[#14181B]">
            {session.durationLabel}
          </dd>
        </div>
        <div>
          <dt className="text-xs text-[#4A524E]">Connected by</dt>
          <dd className="font-medium text-[#14181B]">{session.requester}</dd>
        </div>
        <div>
          <dt className="text-xs text-[#4A524E]">Approved by</dt>
          <dd className="font-medium text-[#14181B]">{session.approver}</dd>
        </div>
        <div>
          <dt className="text-xs text-[#4A524E]">Permissions granted</dt>
          <dd className="font-medium text-[#14181B]">{grantedList.join(" · ")}</dd>
        </div>
        <div>
          <dt className="text-xs text-[#4A524E]">Audit chain record</dt>
          <dd className="font-mono-code text-xs text-[#074A40]">
            #{session.auditSequence ?? 0} · {(session.auditEntryHash ?? "").slice(0, 14)}…
          </dd>
        </div>
      </dl>

      <div className="mt-6 flex flex-col gap-3 sm:flex-row">
        <button
          type="button"
          onClick={onViewAudit}
          className="flex-1 rounded-lg border border-[#DADDD8] bg-white px-4 py-2.5 text-sm font-medium text-[#14181B] hover:bg-[#F5F6F4]"
        >
          View Audit Log
        </button>
        <button
          type="button"
          onClick={onBackToDevices}
          className="flex-1 rounded-lg bg-[#0F7B6C] px-4 py-2.5 text-sm font-medium text-white hover:bg-[#0A6457]"
        >
          Back to Devices
        </button>
      </div>
    </section>
  );
};
