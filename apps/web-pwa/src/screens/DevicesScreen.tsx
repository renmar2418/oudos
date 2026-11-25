import React, { useMemo, useState } from "react";
import {
  Bell,
  BellRing,
  Monitor,
  MonitorOff,
  Plus,
  Search,
  ShieldAlert,
  ShieldCheck,
} from "lucide-react";
import type { DeviceItem, DevicePresence } from "../types";

interface DevicesScreenProps {
  devices: DeviceItem[];
  onConnect: (device: DeviceItem) => void;
  onOpenAddDevice: () => void;
  onToggleNotify: (deviceId: string) => void;
}

const PRESENCE_LABELS: Record<DevicePresence, string> = {
  online: "Online",
  in_session: "In session",
  offline: "Offline",
};

const PRESENCE_STYLES: Record<DevicePresence, string> = {
  online: "bg-[#E3F1EE] text-[#074A40] border-[#0F7B6C]/30",
  in_session: "bg-[#E3F1EE] text-[#074A40] border-[#0F7B6C]/40",
  offline: "bg-[#F5F6F4] text-[#4A524E] border-[#DADDD8]",
};

export const DevicesScreen: React.FC<DevicesScreenProps> = ({
  devices,
  onConnect,
  onOpenAddDevice,
  onToggleNotify,
}) => {
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState<"all" | DevicePresence>("all");

  const counts = useMemo(
    () => ({
      all: devices.length,
      online: devices.filter((d) => d.presence === "online").length,
      in_session: devices.filter((d) => d.presence === "in_session").length,
      offline: devices.filter((d) => d.presence === "offline").length,
    }),
    [devices]
  );

  const filteredDevices = useMemo(() => {
    const q = query.trim().toLowerCase();
    return devices.filter((d) => {
      if (filter !== "all" && d.presence !== filter) return false;
      if (!q) return true;
      return (
        d.alias.toLowerCase().includes(q) ||
        d.ownerName.toLowerCase().includes(q) ||
        d.osName.toLowerCase().includes(q) ||
        d.deviceId.replace(/\s+/g, "").includes(q.replace(/\s+/g, "")) ||
        d.serverDomain.toLowerCase().includes(q)
      );
    });
  }, [devices, filter, query]);

  return (
    <section aria-labelledby="devices-heading" className="space-y-6">
      <div className="flex flex-col gap-3 sm:flex-row sm:items-end sm:justify-between">
        <div>
          <h1 id="devices-heading" className="text-2xl font-semibold tracking-tight text-[#14181B]">
            Devices
          </h1>
          <p className="mt-1 text-sm text-[#4A524E]">
            Every connection asks the device owner first and displays your 6-digit verification code.
          </p>
        </div>

        <button
          type="button"
          onClick={onOpenAddDevice}
          className="inline-flex items-center justify-center gap-2 rounded-lg bg-[#0F7B6C] px-4 py-2 text-sm font-medium text-white transition-colors hover:bg-[#0A6457] focus:outline-none focus:ring-2 focus:ring-[#0F7B6C] focus:ring-offset-2"
        >
          <Plus className="h-4 w-4" aria-hidden="true" />
          Add device
        </button>
      </div>

      <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
        <div className="relative flex-1 max-w-md">
          <label htmlFor="device-search" className="sr-only">
            Search devices by name, owner, OS, or 9-digit ID
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
            placeholder="Search name, owner, OS, or 9-digit ID..."
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
                {label} ({counts[status]})
              </button>
            );
          })}
        </div>
      </div>

      <ul className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3" aria-label="Registered devices">
        {filteredDevices.map((device) => {
          const isScreenUnavailable = Boolean(device.screenUnavailable);
          return (
            <li
              key={device.deviceId}
              className="flex flex-col justify-between rounded-xl border border-[#DADDD8] bg-white p-5 shadow-xs"
            >
              <div>
                <div className="flex items-start justify-between gap-2">
                  <div className="flex items-center gap-2.5">
                    <div
                      className={`flex h-9 w-9 items-center justify-center rounded-lg ${
                        isScreenUnavailable
                          ? "bg-[#FEF0C7] text-[#B54708]"
                          : "bg-[#E3F1EE] text-[#0F7B6C]"
                      }`}
                    >
                      {isScreenUnavailable ? (
                        <MonitorOff className="h-5 w-5" aria-hidden="true" />
                      ) : (
                        <Monitor className="h-5 w-5" aria-hidden="true" />
                      )}
                    </div>
                    <div>
                      <h2 className="text-base font-semibold text-[#14181B]">{device.alias}</h2>
                      <p className="text-xs text-[#4A524E]">
                        Owner: {device.ownerName} · {device.osName}
                      </p>
                    </div>
                  </div>
                  <span
                    className={`inline-flex items-center rounded-full border px-2.5 py-0.5 text-xs font-medium ${
                      isScreenUnavailable
                        ? "border-[#B54708]/30 bg-[#FEF0C7] text-[#B54708]"
                        : PRESENCE_STYLES[device.presence]
                    }`}
                  >
                    {isScreenUnavailable
                      ? "Screen unavailable"
                      : PRESENCE_LABELS[device.presence]}
                  </span>
                </div>

                {device.presence === "in_session" && device.activeRequester && (
                  <p className="mt-2.5 rounded-lg bg-[#E3F1EE] px-2.5 py-1 text-xs font-medium text-[#074A40]">
                    In session with {device.activeRequester}
                  </p>
                )}

                {device.presence === "offline" && (
                  <p className="mt-2.5 text-xs text-[#5C6560]">
                    Last seen <span className="font-mono-code">{device.lastSeenUtc}</span>
                  </p>
                )}

                <div className="mt-4 space-y-2 rounded-lg bg-[#F5F6F4] p-3">
                  <div className="flex items-center justify-between text-xs">
                    <span className="text-[#4A524E]">Device ID</span>
                    <span className="font-mono-code text-sm font-semibold text-[#14181B]">
                      {device.deviceId}
                    </span>
                  </div>
                  <div className="flex items-center justify-between text-xs">
                    <span className="text-[#4A524E]">Access mode</span>
                    <span className="font-medium text-[#14181B]">Asks first</span>
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
                {device.presence === "offline" ? (
                  <button
                    type="button"
                    aria-pressed={Boolean(device.notifyWhenOnline)}
                    onClick={() => onToggleNotify(device.deviceId)}
                    className={`inline-flex w-full items-center justify-center gap-2 rounded-lg border px-4 py-2.5 text-sm font-medium transition-colors focus:outline-none focus:ring-2 focus:ring-[#0F7B6C] ${
                      device.notifyWhenOnline
                        ? "border-[#0F7B6C] bg-[#E3F1EE] text-[#074A40]"
                        : "border-[#DADDD8] bg-white text-[#14181B] hover:bg-[#F5F6F4]"
                    }`}
                  >
                    {device.notifyWhenOnline ? (
                      <>
                        <BellRing className="h-4 w-4" aria-hidden="true" />
                        Notifying when online
                      </>
                    ) : (
                      <>
                        <Bell className="h-4 w-4 text-[#0F7B6C]" aria-hidden="true" />
                        Notify me when online
                      </>
                    )}
                  </button>
                ) : isScreenUnavailable ? (
                  <button
                    type="button"
                    onClick={() => onConnect(device)}
                    className="w-full rounded-lg border border-[#B54708]/40 bg-[#FEF0C7] px-4 py-2.5 text-sm font-semibold text-[#B54708] transition-colors hover:bg-[#FDE6A0] focus:outline-none focus:ring-2 focus:ring-[#B54708]"
                  >
                    View fix &amp; retry
                  </button>
                ) : device.presence === "in_session" ? (
                  <button
                    type="button"
                    onClick={() => onConnect(device)}
                    className="w-full rounded-lg border border-[#0F7B6C] bg-[#E3F1EE] px-4 py-2.5 text-sm font-semibold text-[#074A40] transition-colors hover:bg-[#D2E9E4] focus:outline-none focus:ring-2 focus:ring-[#0F7B6C]"
                  >
                    Ask to join
                  </button>
                ) : (
                  <button
                    type="button"
                    onClick={() => onConnect(device)}
                    className="w-full rounded-lg bg-[#0F7B6C] px-4 py-2.5 text-sm font-medium text-white transition-colors hover:bg-[#0A6457] focus:outline-none focus:ring-2 focus:ring-[#0F7B6C] focus:ring-offset-2"
                  >
                    Request access
                  </button>
                )}
              </div>
            </li>
          );
        })}
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
