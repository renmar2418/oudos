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
      className="mx-auto max-w-xl rounded-2xl border border-[#DADDD8] bg-white p-6 shadow-xs sm:p-8"
    >
      <div className="flex flex-wrap items-center justify-between gap-3">
        {device.enrolledServer ? (
          <span className="inline-flex items-center gap-1.5 rounded-full bg-[#E3F1EE] px-3 py-1 text-xs font-medium text-[#074A40]">
            <ShieldCheck className="h-3.5 w-3.5" aria-hidden="true" />
            {device.serverDomain}
          </span>
        ) : (
          <span className="inline-flex items-center gap-1.5 rounded-full bg-[#FEF0C7] px-3 py-1 text-xs font-medium text-[#B54708]">
            <ShieldAlert className="h-3.5 w-3.5" aria-hidden="true" />
            {device.serverDomain}
          </span>
        )}
        <span
          className="inline-flex items-center gap-1.5 font-mono-code text-sm font-medium text-[#4A524E]"
          aria-live="polite"
        >
          <Clock className="h-4 w-4 text-[#0F7B6C]" aria-hidden="true" />
          {mins}:{secs} left
        </span>
      </div>

      <ol
        aria-label="Connection progress"
        className="mt-5 grid grid-cols-3 gap-2 text-xs"
      >
        <li className="flex items-center gap-1.5 rounded-lg border border-[#0F7B6C]/30 bg-[#E3F1EE] px-2.5 py-2 font-medium text-[#074A40]">
          <Check className="h-3.5 w-3.5 shrink-0" aria-hidden="true" />
          1. Key verified
        </li>
        <li
          aria-current="step"
          className="rounded-lg border border-[#0F7B6C] bg-white px-2.5 py-2 font-semibold text-[#14181B]"
        >
          2. Owner approval
        </li>
        <li className="rounded-lg border border-[#DADDD8] bg-[#F5F6F4] px-2.5 py-2 text-[#5C6560]">
          3. Secure channel
        </li>
      </ol>

      <h1 id="requesting-heading" className="mt-5 text-2xl font-semibold text-[#14181B]">
        Waiting for {device.ownerName} to allow access ({mins}:{secs} left)
      </h1>
      <p className="mt-2 text-sm text-[#4A524E]">
        {isJoinRequest ? "Asking to join the live session on " : "Connecting to "}
        <span className="font-medium text-[#14181B]">{device.alias}</span> (
        <span className="font-mono-code">{device.deviceId}</span>). Nothing starts until{" "}
        {device.ownerName} approves on their screen.
      </p>

      <div className="mt-6 rounded-xl border border-[#0F7B6C]/30 bg-[#E3F1EE] p-5 text-center">
        <p className="text-xs font-semibold uppercase tracking-wider text-[#074A40]">
          6-Digit Verification Code
        </p>
        <p
          className="mt-2 font-mono-code text-4xl font-semibold tracking-widest text-[#14181B]"
          aria-label={`Verification code ${sasCode}`}
        >
          {sasCode}
        </p>
        <p className="mt-2 text-sm text-[#074A40]">
          Confirm this exact number matches the code shown on {device.ownerName}&apos;s screen.
        </p>
      </div>

      <fieldset className="mt-5 rounded-xl border border-[#DADDD8] bg-[#F5F6F4] p-4">
        <legend className="px-1 text-xs font-semibold uppercase tracking-wider text-[#4A524E]">
          Requested permissions ({device.ownerName} decides final grants)
        </legend>
        <div className="mt-2 grid gap-2 text-xs sm:grid-cols-2">
          <label className="flex items-center gap-2 text-[#14181B]">
            <input
              type="checkbox"
              checked
              disabled
              className="h-4 w-4 accent-[#0F7B6C]"
            />
            <span>See screen (required)</span>
          </label>
          <label className="flex items-center gap-2 text-[#14181B]">
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
              className="h-4 w-4 accent-[#0F7B6C]"
            />
            <span>
              Mouse &amp; keyboard{!policy.allowMouseKeyboard.value ? " (blocked by policy)" : ""}
            </span>
          </label>
          <label className="flex items-center gap-2 text-[#14181B]">
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
              className="h-4 w-4 accent-[#0F7B6C]"
            />
            <span>
              Clipboard sync{!policy.allowClipboardSync.value ? " (blocked by policy)" : ""}
            </span>
          </label>
          <label className="flex items-center gap-2 text-[#14181B]">
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
              className="h-4 w-4 accent-[#0F7B6C]"
            />
            <span>
              File transfer{!policy.allowFileTransfer.value ? " (off by policy)" : ""}
            </span>
          </label>
        </div>
      </fieldset>

      <div className="mt-6 flex flex-col gap-3 sm:flex-row">
        <button
          type="button"
          onClick={onCancel}
          className="flex-1 rounded-lg border border-[#DADDD8] bg-white px-4 py-2.5 text-sm font-medium text-[#14181B] hover:bg-[#F5F6F4] focus:outline-none focus:ring-2 focus:ring-[#0F7B6C]"
        >
          Cancel request
        </button>
        <button
          type="button"
          onClick={onCodeMismatch}
          className="flex-1 rounded-lg border border-[#B42318]/40 bg-[#FEE4E2] px-4 py-2.5 text-sm font-semibold text-[#B42318] hover:bg-[#FCD5D2] focus:outline-none focus:ring-2 focus:ring-[#B42318]"
        >
          Codes don&apos;t match
        </button>
      </div>
    </section>
  );
};
