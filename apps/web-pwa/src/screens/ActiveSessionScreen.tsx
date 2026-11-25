import React, { useState } from "react";
import { Command, Lock, Monitor, PhoneOff } from "lucide-react";
import type { DeviceItem, QualityPreset } from "../types";

interface ActiveSessionScreenProps {
  device: DeviceItem;
  onEndSession: () => void;
}

export const ActiveSessionScreen: React.FC<ActiveSessionScreenProps> = ({
  device,
  onEndSession,
}) => {
  const [quality, setQuality] = useState<QualityPreset>("Auto");
  const [lastShortcut, setLastShortcut] = useState<string | null>(null);
  const latencyMs = quality === "Fast" ? 29 : quality === "Auto" ? 38 : 54;

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
            E2E Encrypted · Live
          </span>
          <div>
            <h1 id="active-session-heading" className="text-base font-semibold text-white">
              {device.alias}
            </h1>
            <p className="text-xs text-[#C4CCC8]">
              {device.serverDomain} · Screen only
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
            onClick={onEndSession}
            className="inline-flex items-center gap-1.5 rounded-lg bg-[#B42318] px-3.5 py-1.5 text-xs font-semibold text-white transition-colors hover:bg-[#912018] focus:outline-none focus:ring-2 focus:ring-white"
          >
            <PhoneOff className="h-3.5 w-3.5" aria-hidden="true" />
            End
          </button>
        </div>
      </header>

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
              onClick={() => sendShortcut(shortcut)}
              className="rounded-md border border-[#343E43] bg-[#1C2327] px-2.5 py-1 font-mono-code text-xs text-white hover:border-[#5FD9BF] focus:outline-none focus:ring-2 focus:ring-[#5FD9BF]"
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

      <div className="flex min-h-[360px] flex-col items-center justify-center bg-[#090C0D] p-8 text-center">
        <Monitor className="h-12 w-12 text-[#5FD9BF]" aria-hidden="true" />
        <p className="mt-4 text-sm font-medium text-white">
          Remote desktop stream viewport ({quality} mode · 1080p@30fps)
        </p>
        <p className="mt-1 max-w-md text-xs text-[#C4CCC8]">
          {device.ownerName} can see the persistent top-most session banner and end this session at any time.
        </p>
      </div>
    </section>
  );
};
