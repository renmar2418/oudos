import React, { useEffect, useState } from "react";
import { Check, Clock, ShieldAlert, ShieldCheck } from "lucide-react";
import type { DeviceItem, GrantedPermissions, PolicyState } from "../types";

interface RequestingScreenProps {
  device: DeviceItem;
  sasCode: string;
  policy: PolicyState;
  requestedPermissions: GrantedPermissions;
  isJoinRequest?: boolean;
  initialSecondsLeft?: number;
  onUpdateRequestedPermissions: (next: GrantedPermissions) => void;
  onCancel: () => void;
  onCodeMismatch: () => void;
  onExpire: () => void;
}

export const RequestingScreen: React.FC<RequestingScreenProps> = ({
  device,
  sasCode,
  policy,
  requestedPermissions,
  isJoinRequest = false,
  initialSecondsLeft = 120,
  onUpdateRequestedPermissions,
  onCancel,
  onCodeMismatch,
  onExpire,
}) => {
  const [secondsLeft, setSecondsLeft] = useState(initialSecondsLeft);

  useEffect(() => {
    const deadlineMs = Date.now() + initialSecondsLeft * 1000;
    let expiredFired = false;

    const updateRemaining = () => {
      const remaining = Math.max(0, Math.ceil((deadlineMs - Date.now()) / 1000));
      setSecondsLeft(remaining);
      if (remaining <= 0 && !expiredFired) {
        expiredFired = true;
        onExpire();
      }
    };

    updateRemaining();
    const timer = window.setInterval(updateRemaining, 250);
    return () => window.clearInterval(timer);
  }, [device.deviceId, initialSecondsLeft, onExpire]);

  const mins = Math.floor(secondsLeft / 60);
  const secs = String(secondsLeft % 60).padStart(2, "0");

  return (
    <section
      aria-labelledby="requesting-heading"
      className="mx-auto max-w-xl border border-[#DADDD8] bg-white divide-y divide-[#DADDD8]"
    >
      <div className="flex flex-wrap items-center justify-between gap-3 px-6 py-3.5">
        {device.enrolledServer ? (
          <span className="inline-flex items-center gap-1.5 font-mono-code text-xs font-semibold text-[#0F5C52]">
            <ShieldCheck className="h-3.5 w-3.5" aria-hidden="true" />
            {device.serverDomain}
          </span>
        ) : (
          <span className="inline-flex items-center gap-1.5 font-mono-code text-xs font-semibold text-[#B54708]">
            <ShieldAlert className="h-3.5 w-3.5" aria-hidden="true" />
            {device.serverDomain}
          </span>
        )}
        <span
          className="inline-flex items-center gap-1.5 font-mono-code text-xs font-semibold text-[#525D63]"
          aria-live="polite"
        >
          <Clock className="h-3.5 w-3.5 text-[#0F5C52]" aria-hidden="true" />
          {mins}:{secs} left
        </span>
      </div>

      <ol
        aria-label="Connection progress"
        className="grid grid-cols-3 divide-x divide-[#DADDD8] bg-[#F5F6F4] text-xs"
      >
        <li className="flex items-center gap-1.5 px-3 py-2.5 font-medium text-[#083630]">
          <Check className="h-3.5 w-3.5 shrink-0 text-[#0F5C52]" aria-hidden="true" />
          1. Key verified
        </li>
        <li
          aria-current="step"
          className="border-b-2 border-[#0F5C52] bg-white px-3 py-2.5 font-semibold text-[#111618]"
        >
          2. Owner approval
        </li>
        <li className="px-3 py-2.5 text-[#525D63]">3. Secure channel</li>
      </ol>

      <div className="px-6 py-5">
        <h1 id="requesting-heading" className="text-xl font-semibold text-[#111618]">
          Waiting for {device.ownerName} to allow access ({mins}:{secs} left)
        </h1>
        <p className="mt-1.5 text-sm text-[#525D63]">
          {isJoinRequest ? "Asking to join the live session on " : "Connecting to "}
          <span className="font-medium text-[#111618]">{device.alias}</span> (
          <span className="font-mono-code">{device.deviceId}</span>). Nothing starts until{" "}
          {device.ownerName} approves on their screen.
        </p>
      </div>

      <div className="bg-[#F5F6F4] px-6 py-5 text-center">
        <p
          className="font-mono-code text-3xl font-semibold tracking-[0.14em] text-[#111618] sm:text-4xl sm:tracking-[0.16em]"
          aria-label={`Verification code ${sasCode}`}
        >
          {sasCode}
        </p>
        <p className="mt-2 text-sm text-[#525D63]">
          Confirm this exact 6-digit code matches the number shown on {device.ownerName}&apos;s screen.
        </p>
      </div>

      <fieldset className="px-6 py-4">
        <legend className="text-xs font-semibold text-[#111618]">
          Requested permissions ({device.ownerName} decides final grants)
        </legend>
        <div className="mt-2.5 grid gap-2.5 text-xs sm:grid-cols-2">
          <label className="flex items-center gap-2 text-[#111618]">
            <input
              type="checkbox"
              checked
              disabled
              className="h-4 w-4 accent-[#0F5C52]"
            />
            <span>See screen (required)</span>
          </label>
          <label className="flex items-center gap-2 text-[#111618]">
            <input
              type="checkbox"
              disabled={!policy.allowMouseKeyboard.value}
              checked={requestedPermissions.mouseKeyboard}
              onChange={(e) =>
                onUpdateRequestedPermissions({
                  ...requestedPermissions,
                  mouseKeyboard: e.target.checked,
                })
              }
              className="h-4 w-4 accent-[#0F5C52]"
            />
            <span>
              Mouse &amp; keyboard{!policy.allowMouseKeyboard.value ? " (blocked by policy)" : ""}
            </span>
          </label>
          <label className="flex items-center gap-2 text-[#111618]">
            <input
              type="checkbox"
              disabled={!policy.allowClipboardSync.value}
              checked={requestedPermissions.clipboardSync}
              onChange={(e) =>
                onUpdateRequestedPermissions({
                  ...requestedPermissions,
                  clipboardSync: e.target.checked,
                })
              }
              className="h-4 w-4 accent-[#0F5C52]"
            />
            <span>
              Clipboard sync{!policy.allowClipboardSync.value ? " (blocked by policy)" : ""}
            </span>
          </label>
          <label className="flex items-center gap-2 text-[#111618]">
            <input
              type="checkbox"
              disabled={!policy.allowFileTransfer.value}
              checked={requestedPermissions.fileTransfer}
              onChange={(e) =>
                onUpdateRequestedPermissions({
                  ...requestedPermissions,
                  fileTransfer: e.target.checked,
                })
              }
              className="h-4 w-4 accent-[#0F5C52]"
            />
            <span>
              File transfer{!policy.allowFileTransfer.value ? " (off by policy)" : ""}
            </span>
          </label>
        </div>
      </fieldset>

      <div className="flex flex-col gap-3 bg-[#F5F6F4] px-6 py-4 sm:flex-row">
        <button
          type="button"
          onClick={onCancel}
          className="flex-1 rounded-md border border-[#DADDD8] bg-white px-4 py-2.5 text-sm font-medium text-[#111618] hover:bg-[#F5F6F4] focus:outline-none focus-visible:ring-2 focus-visible:ring-[#0F5C52]"
        >
          Cancel request
        </button>
        <button
          type="button"
          onClick={onCodeMismatch}
          className="flex-1 rounded-md border border-[#B42318] bg-[#FEE4E2] px-4 py-2.5 text-sm font-semibold text-[#912018] hover:bg-[#FCD5D2] focus:outline-none focus-visible:ring-2 focus-visible:ring-[#B42318]"
        >
          Codes don&apos;t match
        </button>
      </div>
    </section>
  );
};
