import React, { useEffect, useState } from "react";
import { Clock, ShieldCheck } from "lucide-react";
import type { DeviceItem } from "../types";

interface RequestingScreenProps {
  device: DeviceItem;
  sasCode: string;
  onCancel: () => void;
  onSimulateAllow: () => void;
}

export const RequestingScreen: React.FC<RequestingScreenProps> = ({
  device,
  sasCode,
  onCancel,
  onSimulateAllow,
}) => {
  const [secondsLeft, setSecondsLeft] = useState(102); // 1:42 default countdown

  useEffect(() => {
    const timer = window.setInterval(() => {
      setSecondsLeft((prev) => (prev > 0 ? prev - 1 : 0));
    }, 1000);
    return () => window.clearInterval(timer);
  }, []);

  const mins = Math.floor(secondsLeft / 60);
  const secs = String(secondsLeft % 60).padStart(2, "0");

  return (
    <section
      aria-labelledby="requesting-heading"
      className="mx-auto max-w-lg rounded-2xl border border-[#DADDD8] bg-white p-6 shadow-xs sm:p-8"
    >
      <div className="flex items-center justify-between gap-3">
        <span className="inline-flex items-center gap-1.5 rounded-full bg-[#E3F1EE] px-3 py-1 text-xs font-medium text-[#074A40]">
          <ShieldCheck className="h-3.5 w-3.5" aria-hidden="true" />
          {device.serverDomain}
        </span>
        <span
          className="inline-flex items-center gap-1.5 font-mono-code text-sm font-medium text-[#4A524E]"
          aria-live="polite"
        >
          <Clock className="h-4 w-4 text-[#0F7B6C]" aria-hidden="true" />
          {mins}:{secs} left
        </span>
      </div>

      <h1 id="requesting-heading" className="mt-5 text-2xl font-semibold text-[#14181B]">
        Waiting for {device.ownerName} to allow access ({mins}:{secs} left)
      </h1>
      <p className="mt-2 text-sm text-[#4A524E]">
        Connecting to <span className="font-medium text-[#14181B]">{device.alias}</span> (
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
          onClick={onSimulateAllow}
          className="flex-1 rounded-lg bg-[#0F7B6C] px-4 py-2.5 text-sm font-medium text-white hover:bg-[#0A6457] focus:outline-none focus:ring-2 focus:ring-[#0F7B6C] focus:ring-offset-2"
        >
          Preview active session
        </button>
      </div>
    </section>
  );
};
