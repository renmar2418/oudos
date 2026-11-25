import React, { useMemo, useState } from "react";
import { Monitor, Search, ShieldAlert, ShieldCheck } from "lucide-react";
import type { DeviceItem, DevicePresence } from "../types";

interface DevicesScreenProps {
  devices: DeviceItem[];
  onConnect: (device: DeviceItem) => void;
}

const PRESENCE_LABELS: Record<DevicePresence, string> = {
  online: "Online",
  in_session: "In session",
  offline: "Offline",
};

const PRESENCE_STYLES: Record<DevicePresence, string> = {
  online: "bg-[#E3F1EE] text-[#074A40] border-[#0F7B6C]/30",
  in_session: "bg-[#E3F1EE] text-[#0F7B6C] border-[#0F7B6C]/40",
  offline: "bg-[#F5F6F4] text-[#4A524E] border-[#DADDD8]",
};

export const DevicesScreen: React.FC<DevicesScreenProps> = ({ devices, onConnect }) => {
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState<"all" | DevicePresence>("all");

  const filteredDevices = useMemo(() => {
    const q = query.trim().toLowerCase();
    return devices.filter((d) => {
      if (filter !== "all" && d.presence !== filter) return false;
      if (!q) return true;
      return (
        d.alias.toLowerCase().includes(q) ||
        d.ownerName.toLowerCase().includes(q) ||
        d.deviceId.replace(/\s+/g, "").includes(q.replace(/\s+/g, "")) ||
        d.serverDomain.toLowerCase().includes(q)
      );
    });
  }, [devices, filter, query]);

  return (
    <section aria-labelledby="devices-heading" className="space-y-6">
      <div className="flex flex-col gap-2 sm:flex-row sm:items-end sm:justify-between">
        <div>
          <h1 id="devices-heading" className="text-2xl font-semibold tracking-tight text-[#14181B]">
            Devices
          </h1>
          <p className="mt-1 text-sm text-[#4A524E]">
            Every connection asks the device owner first and displays your 6-digit verification code.
          </p>
        </div>
      </div>

      <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
        <div className="relative flex-1 max-w-md">
          <label htmlFor="device-search" className="sr-only">
            Search devices by name, owner, or 9-digit ID
          </label>
          <Search
            className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-[#5C6560]"
            aria-hidden="true"
          />
          <input
            id="device-search"
            type="search"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Search name, owner, or 9-digit ID..."
            className="w-full rounded-lg border border-[#DADDD8] bg-white py-2 pl-9 pr-3 text-sm text-[#14181B] placeholder:text-[#5C6560] focus:border-[#0F7B6C] focus:outline-none focus:ring-2 focus:ring-[#0F7B6C]/30"
          />
        </div>

        <div role="group" aria-label="Filter devices by status" className="flex flex-wrap gap-2">
          {(["all", "online", "in_session", "offline"] as const).map((status) => {
            const active = filter === status;
            const label = status === "all" ? "All" : PRESENCE_LABELS[status];
            return (
              <button
                key={status}
                type="button"
                aria-pressed={active}
                onClick={() => setFilter(status)}
                className={`rounded-full px-3.5 py-1.5 text-sm font-medium transition-colors focus:outline-none focus:ring-2 focus:ring-[#0F7B6C] ${
                  active
                    ? "bg-[#E3F1EE] text-[#074A40] border border-[#0F7B6C]/40"
                    : "bg-white text-[#4A524E] border border-[#DADDD8] hover:bg-[#F5F6F4]"
                }`}
              >
                {label}
              </button>
            );
          })}
        </div>
      </div>

      <ul className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3" aria-label="Registered devices">
        {filteredDevices.map((device) => (
          <li
            key={device.deviceId}
            className="flex flex-col justify-between rounded-xl border border-[#DADDD8] bg-white p-5 shadow-xs"
          >
            <div>
              <div className="flex items-start justify-between gap-2">
                <div className="flex items-center gap-2.5">
                  <div className="flex h-9 w-9 items-center justify-center rounded-lg bg-[#E3F1EE] text-[#0F7B6C]">
                    <Monitor className="h-5 w-5" aria-hidden="true" />
                  </div>
                  <div>
                    <h2 className="text-base font-semibold text-[#14181B]">{device.alias}</h2>
                    <p className="text-xs text-[#4A524E]">
                      Owner: {device.ownerName} · {device.osName}
                    </p>
                  </div>
                </div>
                <span
                  className={`inline-flex items-center rounded-full border px-2.5 py-0.5 text-xs font-medium ${PRESENCE_STYLES[device.presence]}`}
                >
                  {PRESENCE_LABELS[device.presence]}
                </span>
              </div>

              <div className="mt-4 space-y-2 rounded-lg bg-[#F5F6F4] p-3">
                <div className="flex items-center justify-between text-xs">
                  <span className="text-[#4A524E]">Device ID</span>
                  <span className="font-mono-code text-sm font-semibold text-[#14181B]">
                    {device.deviceId}
                  </span>
                </div>
                <div className="flex items-center justify-between gap-2 text-xs">
                  <span className="text-[#4A524E]">Server</span>
                  {device.enrolledServer ? (
                    <span className="inline-flex items-center gap-1 rounded-md bg-[#E3F1EE] px-2 py-0.5 font-medium text-[#074A40]">
                      <ShieldCheck className="h-3.5 w-3.5" aria-hidden="true" />
                      {device.serverDomain}
                    </span>
                  ) : (
                    <span className="inline-flex items-center gap-1 rounded-md bg-[#FEF0C7] px-2 py-0.5 font-medium text-[#B54708]">
                      <ShieldAlert className="h-3.5 w-3.5" aria-hidden="true" />
                      {device.serverDomain}
                    </span>
                  )}
                </div>
              </div>
            </div>

            <div className="mt-4">
              <button
                type="button"
                disabled={device.presence === "offline"}
                onClick={() => onConnect(device)}
                className="w-full rounded-lg bg-[#0F7B6C] px-4 py-2.5 text-sm font-medium text-white transition-colors hover:bg-[#0A6457] focus:outline-none focus:ring-2 focus:ring-[#0F7B6C] focus:ring-offset-2 disabled:cursor-not-allowed disabled:bg-[#DADDD8] disabled:text-[#4A524E]"
              >
                {device.presence === "offline" ? "Device offline" : "Request access"}
              </button>
            </div>
          </li>
        ))}
      </ul>

      {filteredDevices.length === 0 && (
        <div className="rounded-xl border border-[#DADDD8] bg-white p-8 text-center">
          <p className="text-sm font-medium text-[#14181B]">No matching devices found.</p>
          <p className="mt-1 text-sm text-[#4A524E]">
            Check the 9-digit device ID or clear your status filter.
          </p>
        </div>
      )}
    </section>
  );
};
