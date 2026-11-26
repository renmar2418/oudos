import React, { useMemo, useState } from "react";
import {
  Bell,
  BellRing,
  Clock,
  Link2,
  Monitor,
  MonitorOff,
  Plus,
  Search,
  ShieldAlert,
  ShieldBan,
  ShieldCheck,
} from "lucide-react";
import type {
  AbuseCategory,
  DeviceItem,
  DevicePresence,
  SupportLinkItem,
} from "../types";

const SUPPORT_LINK_TTL_MS = 15 * 60 * 1000;
const UTC_MONTHS = [
  "Jan",
  "Feb",
  "Mar",
  "Apr",
  "May",
  "Jun",
  "Jul",
  "Aug",
  "Sep",
  "Oct",
  "Nov",
  "Dec",
] as const;

export function formatUtcTimestamp(isoUtc: string): string {
  const date = new Date(isoUtc);
  if (Number.isNaN(date.getTime())) return isoUtc;
  const month = UTC_MONTHS[date.getUTCMonth()];
  const day = date.getUTCDate();
  const hh = String(date.getUTCHours()).padStart(2, "0");
  const mm = String(date.getUTCMinutes()).padStart(2, "0");
  return `${month} ${day}, ${hh}:${mm} UTC`;
}

interface DevicesScreenProps {
  devices: DeviceItem[];
  onConnect: (device: DeviceItem) => void;
  onOpenAddDevice: () => void;
  onToggleNotify: (deviceId: string) => void;
  onCreateSupportLink?: (link: SupportLinkItem) => void;
  onRedeemSupportLink?: (linkId: string) => void;
  onBlockAndReport?: (
    device: DeviceItem,
    requester: string,
    category: AbuseCategory,
    summary: string
  ) => void;
}

const PRESENCE_LABELS: Record<DevicePresence, string> = {
  online: "Online",
  in_session: "In session",
  offline: "Offline",
};

export const DevicesScreen: React.FC<DevicesScreenProps> = ({
  devices,
  onConnect,
  onOpenAddDevice,
  onToggleNotify,
  onCreateSupportLink,
  onRedeemSupportLink,
  onBlockAndReport,
}) => {
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState<"all" | DevicePresence>("all");
  const [supportLinks, setSupportLinks] = useState<SupportLinkItem[]>([]);
  const [linkVerifyDomain, setLinkVerifyDomain] = useState("");
  const [linkStatusMessage, setLinkStatusMessage] = useState<string | null>(null);
  const [linkErrorMessage, setLinkErrorMessage] = useState<string | null>(null);
  const [abuseDeviceId, setAbuseDeviceId] = useState<string>(devices[0]?.deviceId ?? "");
  const [abuseRequester, setAbuseRequester] = useState("");
  const [abuseCategory, setAbuseCategory] = useState<AbuseCategory>("scam_attempt");
  const [abuseSummary, setAbuseSummary] = useState("");
  const [abuseConfirmation, setAbuseConfirmation] = useState<string | null>(null);

  const handleIssue15MinLink = (device: DeviceItem) => {
    const nowMs = Date.now();
    const linkId = `sup-${nowMs}-${Math.floor(Math.random() * 1000)}`;
    const link: SupportLinkItem = {
      linkId,
      deviceId: device.deviceId,
      deviceAlias: device.alias,
      serverDomain: device.serverDomain,
      serverPubkey: device.pinnedPubkey,
      supportUrl: `https://${device.serverDomain}/support/${linkId}`,
      createdAtMs: nowMs,
      expiresAtMs: nowMs + SUPPORT_LINK_TTL_MS,
      redeemed: false,
    };
    setSupportLinks((prev) => [link, ...prev.slice(0, 3)]);
    setLinkVerifyDomain(device.serverDomain);
    setLinkErrorMessage(null);
    setLinkStatusMessage(
      `One-time link issued for ${device.alias} (bound to ${device.serverDomain}, expires in 15 minutes).`
    );
    onCreateSupportLink?.(link);
  };

  const handleRedeemLink = (link: SupportLinkItem, simulatedOffsetMs = 0) => {
    setLinkErrorMessage(null);
    setLinkStatusMessage(null);
    const checkNow = Date.now() + simulatedOffsetMs;
    if (link.redeemed) {
      setLinkErrorMessage("Support link rejected: single-use token already redeemed.");
      return;
    }
    if (
      linkVerifyDomain.trim() &&
      linkVerifyDomain.trim().toLowerCase() !== link.serverDomain.toLowerCase()
    ) {
      setLinkErrorMessage(
        `Support link rejected: server domain mismatch (expected ${link.serverDomain}).`
      );
      return;
    }
    if (checkNow >= link.expiresAtMs) {
      setLinkErrorMessage("Support link rejected: 15-minute expiry window elapsed.");
      return;
    }
    setSupportLinks((prev) =>
      prev.map((item) =>
        item.linkId === link.linkId ? { ...item, redeemed: true } : item
      )
    );
    setLinkStatusMessage(`Support link redeemed once on ${link.serverDomain}.`);
    onRedeemSupportLink?.(link.linkId);
  };

  const handleSubmitAbuse = (e: React.FormEvent) => {
    e.preventDefault();
    const target = devices.find((d) => d.deviceId === abuseDeviceId) ?? devices[0];
    if (!target || !abuseRequester.trim() || !abuseSummary.trim()) return;
    onBlockAndReport?.(
      target,
      abuseRequester.trim(),
      abuseCategory,
      abuseSummary.trim()
    );
    setAbuseConfirmation(
      `Blocked "${abuseRequester.trim()}" on ${target.alias} and logged abuse report (24h review target).`
    );
    setAbuseRequester("");
    setAbuseSummary("");
  };

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
    <section aria-labelledby="devices-heading" className="min-w-0 space-y-5">
      <div className="flex flex-col gap-3 border-b border-[#DADDD8] pb-4 sm:flex-row sm:items-end sm:justify-between">
        <div className="min-w-0">
          <h1 id="devices-heading" className="text-2xl font-semibold tracking-tight text-[#111618]">
            Devices
          </h1>
          <p className="mt-1 text-sm text-[#525D63]">
            Every connection asks the device owner first and displays your 6-digit verification code.
          </p>
        </div>

        <button
          type="button"
          onClick={onOpenAddDevice}
          className="inline-flex min-h-[44px] shrink-0 items-center justify-center gap-2 rounded-md bg-[#0F5C52] px-4 py-2 text-sm font-semibold text-white transition-colors hover:bg-[#0B473F] focus:outline-none focus-visible:ring-2 focus-visible:ring-[#0F5C52] focus-visible:ring-offset-2 sm:min-h-[38px]"
        >
          <Plus className="h-4 w-4" aria-hidden="true" />
          Add device
        </button>
      </div>

      <div className="flex flex-col gap-3 lg:flex-row lg:items-center lg:justify-between">
        <div className="relative min-w-0 flex-1 lg:max-w-md">
          <label htmlFor="device-search" className="sr-only">
            Search devices by name, owner, OS, or 9-digit ID
          </label>
          <Search
            className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-[#525D63]"
            aria-hidden="true"
          />
          <input
            id="device-search"
            type="search"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Search name, owner, OS, or 9-digit ID..."
            className="min-h-[44px] w-full rounded-md border border-[#DADDD8] bg-white py-2 pl-9 pr-3 text-sm text-[#111618] placeholder:text-[#525D63] focus:border-[#0F5C52] focus:outline-none focus:ring-2 focus:ring-[#0F5C52]/25 sm:min-h-[38px]"
          />
        </div>

        <div
          role="group"
          aria-label="Filter devices by status"
          className="grid grid-cols-2 gap-px border border-[#DADDD8] bg-[#DADDD8] sm:inline-flex sm:w-fit"
        >
          {(["all", "online", "in_session", "offline"] as const).map((status) => {
            const active = filter === status;
            const label = status === "all" ? "All" : PRESENCE_LABELS[status];
            return (
              <button
                key={status}
                type="button"
                aria-pressed={active}
                onClick={() => setFilter(status)}
                className={`min-h-[44px] px-3 py-1.5 text-xs font-medium whitespace-nowrap transition-colors focus:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-[#0F5C52] sm:min-h-[36px] ${
                  active
                    ? "bg-[#E6F2F0] font-semibold text-[#083630]"
                    : "bg-white text-[#525D63] hover:bg-[#F5F6F4] hover:text-[#111618]"
                }`}
              >
                {label} <span className="font-mono-code">({counts[status]})</span>
              </button>
            );
          })}
        </div>
      </div>

      <div className="border border-[#DADDD8] bg-white">
        <div
          aria-hidden="true"
          className="hidden border-b border-[#DADDD8] bg-[#F5F6F4] px-4 py-2 text-xs font-medium text-[#525D63] lg:grid lg:grid-cols-[minmax(0,1.4fr)_125px_minmax(0,1.1fr)_minmax(175px,1fr)_170px] lg:gap-4"
        >
          <span>Device</span>
          <span>Device ID</span>
          <span>Server</span>
          <span>Status</span>
          <span className="text-right">Action</span>
        </div>

        {filteredDevices.length > 0 ? (
          <ul className="divide-y divide-[#DADDD8]" aria-label="Registered devices">
            {filteredDevices.map((device) => {
              const isScreenUnavailable = Boolean(device.screenUnavailable);
              return (
                <li
                  key={device.deviceId}
                  className="flex min-w-0 flex-col gap-3 px-4 py-3.5 transition-colors hover:bg-[#F5F6F4]/50 md:grid md:grid-cols-[minmax(0,1fr)_auto] md:items-center md:gap-4 lg:grid-cols-[minmax(0,1.4fr)_125px_minmax(0,1.1fr)_minmax(175px,1fr)_170px]"
                >
                  <div className="flex min-w-0 items-start gap-3">
                    <span
                      className={`mt-0.5 shrink-0 ${
                        isScreenUnavailable ? "text-[#B54708]" : "text-[#525D63]"
                      }`}
                    >
                      {isScreenUnavailable ? (
                        <MonitorOff className="h-4 w-4" aria-hidden="true" />
                      ) : (
                        <Monitor className="h-4 w-4" aria-hidden="true" />
                      )}
                    </span>
                    <div className="min-w-0">
                      <h2 className="truncate text-sm font-semibold text-[#111618]">
                        {device.alias}
                      </h2>
                      <p className="text-xs text-[#525D63]">
                        {device.ownerName} · {device.osName}
                      </p>
                      <div className="mt-2 flex flex-wrap items-center gap-x-4 gap-y-1 text-xs md:flex lg:hidden">
                        <span className="font-mono-code font-medium text-[#111618]">
                          {device.deviceId}
                        </span>
                        {device.enrolledServer ? (
                          <span className="inline-flex items-center gap-1 font-mono-code text-[#0F5C52]">
                            <ShieldCheck className="h-3.5 w-3.5 shrink-0" aria-hidden="true" />
                            <span>{device.serverDomain}</span>
                          </span>
                        ) : (
                          <span className="inline-flex items-center gap-1 font-mono-code font-medium text-[#B54708]">
                            <ShieldAlert className="h-3.5 w-3.5 shrink-0" aria-hidden="true" />
                            <span>{device.serverDomain}</span>
                          </span>
                        )}
                        <span className="inline-flex items-center gap-1.5 font-medium text-[#111618]">
                          <span
                            aria-hidden="true"
                            className={`h-2 w-2 shrink-0 rounded-full ${
                              isScreenUnavailable
                                ? "bg-[#B54708]"
                                : device.presence === "offline"
                                  ? "bg-[#8A938E]"
                                  : "bg-[#0F5C52]"
                            }`}
                          />
                          <span>
                            {isScreenUnavailable
                              ? "Screen unavailable"
                              : PRESENCE_LABELS[device.presence]}
                          </span>
                        </span>
                        {device.presence === "in_session" && device.activeRequester && (
                          <span className="text-[#525D63]">With {device.activeRequester}</span>
                        )}
                        {device.presence === "offline" && (
                          <time
                            dateTime={device.lastSeenUtc}
                            title={device.lastSeenUtc}
                            className="text-[#525D63]"
                          >
                            Last seen {formatUtcTimestamp(device.lastSeenUtc)}
                          </time>
                        )}
                      </div>
                    </div>
                  </div>

                  <div className="hidden min-w-0 lg:block">
                    <span className="font-mono-code text-sm font-medium text-[#111618]">
                      {device.deviceId}
                    </span>
                  </div>

                  <div className="hidden min-w-0 lg:block">
                    {device.enrolledServer ? (
                      <span className="inline-flex max-w-full items-center gap-1.5 font-mono-code text-xs font-medium text-[#0F5C52]">
                        <ShieldCheck className="h-3.5 w-3.5 shrink-0" aria-hidden="true" />
                        <span className="truncate">{device.serverDomain}</span>
                      </span>
                    ) : (
                      <span className="inline-flex max-w-full items-center gap-1.5 font-mono-code text-xs font-medium text-[#B54708]">
                        <ShieldAlert className="h-3.5 w-3.5 shrink-0" aria-hidden="true" />
                        <span className="truncate">{device.serverDomain}</span>
                      </span>
                    )}
                  </div>

                  <div className="hidden min-w-0 lg:block">
                    <div className="flex items-center gap-2 text-xs font-medium text-[#111618]">
                      <span
                        aria-hidden="true"
                        className={`h-2 w-2 shrink-0 rounded-full ${
                          isScreenUnavailable
                            ? "bg-[#B54708]"
                            : device.presence === "offline"
                              ? "bg-[#8A938E]"
                              : "bg-[#0F5C52]"
                        }`}
                      />
                      <span>
                        {isScreenUnavailable
                          ? "Screen unavailable"
                          : PRESENCE_LABELS[device.presence]}
                      </span>
                    </div>
                    {device.presence === "in_session" && device.activeRequester && (
                      <p className="mt-0.5 pl-4 text-[11px] text-[#525D63]">
                        With {device.activeRequester}
                      </p>
                    )}
                    {device.presence === "offline" && (
                      <p className="mt-0.5 pl-4 text-[11px] text-[#525D63]">
                        <time dateTime={device.lastSeenUtc} title={device.lastSeenUtc}>
                          Last seen {formatUtcTimestamp(device.lastSeenUtc)}
                        </time>
                      </p>
                    )}
                  </div>

                  <div className="md:flex md:justify-end">
                    {device.presence === "offline" ? (
                      <button
                        type="button"
                        aria-pressed={Boolean(device.notifyWhenOnline)}
                        onClick={() => onToggleNotify(device.deviceId)}
                        className={`inline-flex min-h-[44px] w-full items-center justify-center gap-1.5 rounded-md border px-3 py-1.5 text-xs font-semibold transition-colors focus:outline-none focus-visible:ring-2 focus-visible:ring-[#0F5C52] md:w-auto lg:min-h-[34px] ${
                          device.notifyWhenOnline
                            ? "border-[#0F5C52] bg-[#E6F2F0] text-[#083630]"
                            : "border-[#DADDD8] bg-white text-[#111618] hover:bg-[#F5F6F4]"
                        }`}
                      >
                        {device.notifyWhenOnline ? (
                          <>
                            <BellRing className="h-3.5 w-3.5" aria-hidden="true" />
                            Notifying when online
                          </>
                        ) : (
                          <>
                            <Bell className="h-3.5 w-3.5 text-[#0F5C52]" aria-hidden="true" />
                            Notify me when online
                          </>
                        )}
                      </button>
                    ) : isScreenUnavailable ? (
                      <button
                        type="button"
                        onClick={() => onConnect(device)}
                        className="min-h-[44px] w-full rounded-md border border-[#B54708]/50 bg-[#FEF0C7] px-3 py-1.5 text-xs font-semibold text-[#7A2E0E] transition-colors hover:bg-[#FDE6A0] focus:outline-none focus-visible:ring-2 focus-visible:ring-[#B54708] md:w-auto lg:min-h-[34px]"
                      >
                        View fix &amp; retry
                      </button>
                    ) : device.presence === "in_session" ? (
                      <button
                        type="button"
                        onClick={() => onConnect(device)}
                        className="min-h-[44px] w-full rounded-md border border-[#0F5C52] bg-[#E6F2F0] px-3.5 py-1.5 text-xs font-semibold text-[#083630] transition-colors hover:bg-[#D5E9E5] focus:outline-none focus-visible:ring-2 focus-visible:ring-[#0F5C52] md:w-auto lg:min-h-[34px]"
                      >
                        Ask to join
                      </button>
                    ) : (
                      <button
                        type="button"
                        onClick={() => onConnect(device)}
                        className="min-h-[44px] w-full rounded-md bg-[#0F5C52] px-3.5 py-1.5 text-xs font-semibold text-white transition-colors hover:bg-[#0B473F] focus:outline-none focus-visible:ring-2 focus-visible:ring-[#0F5C52] focus-visible:ring-offset-1 md:w-auto lg:min-h-[34px]"
                      >
                        Request access
                      </button>
                    )}
                  </div>
                </li>
              );
            })}
          </ul>
        ) : (
          <div className="px-4 py-10 text-center">
            <p className="text-sm font-medium text-[#111618]">No matching devices found.</p>
            <p className="mt-1 text-sm text-[#525D63]">
              Check the 9-digit device ID or clear your status filter.
            </p>
          </div>
        )}
      </div>

      <div className="grid border border-[#DADDD8] bg-white divide-y divide-[#DADDD8] lg:grid-cols-2 lg:items-start lg:divide-x lg:divide-y-0">
        <section
          aria-labelledby="support-links-heading"
          className="flex min-w-0 flex-col justify-between p-5"
        >
          <div className="flex flex-wrap items-start justify-between gap-3">
            <div className="min-w-0">
              <h2 id="support-links-heading" className="text-base font-semibold text-[#111618]">
                15-Minute One-Time Support Links
              </h2>
              <p className="mt-0.5 text-xs text-[#525D63]">
                Single-use links expire after 15 minutes and stay bound to the enrolled server domain.
              </p>
            </div>
            {devices[0] && (
              <button
                type="button"
                onClick={() => handleIssue15MinLink(devices[0]!)}
                className="inline-flex min-h-[44px] shrink-0 items-center gap-1.5 rounded-md border border-[#0F5C52] bg-[#E6F2F0] px-3 py-1.5 text-xs font-semibold text-[#083630] hover:bg-[#D5E9E5] focus:outline-none focus-visible:ring-2 focus-visible:ring-[#0F5C52] lg:min-h-[34px]"
              >
                <Link2 className="h-3.5 w-3.5" aria-hidden="true" />
                Issue 15-min link
              </button>
            )}
          </div>

          {linkStatusMessage && (
            <p
              role="status"
              className="mt-3 border border-[#0F5C52]/30 bg-[#E6F2F0] px-3 py-2 text-xs font-medium text-[#083630]"
            >
              {linkStatusMessage}
            </p>
          )}
          {linkErrorMessage && (
            <p
              role="alert"
              className="mt-3 border border-[#B42318]/30 bg-[#FEE4E2] px-3 py-2 text-xs font-medium text-[#912018]"
            >
              {linkErrorMessage}
            </p>
          )}

          {supportLinks.length === 0 ? (
            <div className="mt-4 border border-[#DADDD8] bg-[#F5F6F4]/60">
              <div className="border-b border-[#DADDD8] px-3.5 py-2.5">
                <p className="text-xs font-semibold text-[#111618]">No active support links</p>
                <p className="mt-0.5 text-xs text-[#525D63]">
                  Issue a temporary link to start a one-off session without permanently pairing a guest account.
                </p>
              </div>
              <dl className="divide-y divide-[#DADDD8] bg-white text-xs">
                <div className="flex items-center justify-between gap-3 px-3.5 py-2">
                  <dt className="text-[#525D63]">Validity window</dt>
                  <dd className="whitespace-nowrap font-mono-code font-medium text-[#111618]">
                    15 min · single-use
                  </dd>
                </div>
                <div className="flex items-center justify-between gap-3 px-3.5 py-2">
                  <dt className="text-[#525D63]">Server binding</dt>
                  <dd className="font-mono-code font-medium text-[#0F5C52]">
                    {devices[0]?.serverDomain ?? "Enrolled server only"}
                  </dd>
                </div>
                <div className="flex items-center justify-between gap-3 px-3.5 py-2">
                  <dt className="text-[#525D63]">Host consent</dt>
                  <dd className="font-medium text-[#111618]">6-digit match still required</dd>
                </div>
                <div className="flex items-center justify-between gap-3 px-3.5 py-2">
                  <dt className="text-[#525D63]">Revocation</dt>
                  <dd className="font-medium text-[#111618]">Auto-expires on use or timeout</dd>
                </div>
              </dl>
            </div>
          ) : (
            <div className="mt-4 space-y-3 border-t border-[#DADDD8] pt-3">
              <div>
                <label
                  htmlFor="support-link-domain-check"
                  className="block text-xs font-medium text-[#525D63]"
                >
                  Enrolled server domain check
                </label>
                <input
                  id="support-link-domain-check"
                  type="text"
                  value={linkVerifyDomain}
                  onChange={(e) => setLinkVerifyDomain(e.target.value)}
                  className="mt-1 min-h-[40px] w-full rounded-md border border-[#DADDD8] bg-white px-2.5 py-1.5 font-mono-code text-xs text-[#111618] focus:border-[#0F5C52] focus:outline-none lg:min-h-[32px]"
                />
              </div>

              <div className="divide-y divide-[#DADDD8] border border-[#DADDD8]">
                {supportLinks.map((link) => (
                  <div key={link.linkId} className="bg-[#F5F6F4]/60 p-3 text-xs">
                    <div className="flex items-center justify-between gap-2">
                      <span className="font-semibold text-[#111618]">{link.deviceAlias}</span>
                      <span className="inline-flex items-center gap-1 font-mono-code text-[11px] font-semibold text-[#0F5C52]">
                        <Clock className="h-3 w-3" aria-hidden="true" />
                        {link.redeemed ? "Redeemed" : "15:00 max"}
                      </span>
                    </div>
                    <p className="mt-1 break-all font-mono-code text-[11px] text-[#525D63]">
                      {link.supportUrl}
                    </p>
                    <div className="mt-2.5 flex flex-wrap gap-2">
                      <button
                        type="button"
                        onClick={() => handleRedeemLink(link, 0)}
                        className="min-h-[38px] rounded-md bg-[#0F5C52] px-3 py-1.5 text-xs font-semibold text-white hover:bg-[#0B473F] lg:min-h-0"
                      >
                        Redeem link
                      </button>
                      <button
                        type="button"
                        onClick={() => handleRedeemLink(link, SUPPORT_LINK_TTL_MS + 1000)}
                        className="min-h-[38px] rounded-md border border-[#DADDD8] bg-white px-3 py-1.5 text-xs font-medium text-[#111618] hover:bg-[#F5F6F4] lg:min-h-0"
                      >
                        Test &gt;15m expiry
                      </button>
                    </div>
                  </div>
                ))}
              </div>
            </div>
          )}
        </section>

        <section aria-labelledby="abuse-report-heading" className="min-w-0 p-5">
          <div className="flex items-center gap-2">
            <ShieldBan className="h-4 w-4 text-[#B42318]" aria-hidden="true" />
            <h2 id="abuse-report-heading" className="text-base font-semibold text-[#111618]">
              Block Requester &amp; Report Abuse
            </h2>
          </div>
          <p className="mt-0.5 text-xs text-[#525D63]">
            Automatic 15-minute lockout triggers after 3 denials/mismatches. Block and report suspected scam callers immediately.
          </p>

          <form onSubmit={handleSubmitAbuse} className="mt-3.5 space-y-2.5 text-xs">
            <div className="grid gap-2.5 sm:grid-cols-2">
              <div className="min-w-0">
                <label htmlFor="abuse-device-select" className="block font-medium text-[#525D63]">
                  Target device
                </label>
                <select
                  id="abuse-device-select"
                  value={abuseDeviceId}
                  onChange={(e) => setAbuseDeviceId(e.target.value)}
                  className="mt-1 min-h-[40px] w-full rounded-md border border-[#DADDD8] bg-white px-2.5 py-1.5 text-xs text-[#111618] focus:border-[#0F5C52] focus:outline-none lg:min-h-[32px]"
                >
                  {devices.map((d) => (
                    <option key={d.deviceId} value={d.deviceId}>
                      {d.alias}
                    </option>
                  ))}
                </select>
              </div>
              <div className="min-w-0">
                <label htmlFor="abuse-category-select" className="block font-medium text-[#525D63]">
                  Category
                </label>
                <select
                  id="abuse-category-select"
                  value={abuseCategory}
                  onChange={(e) => setAbuseCategory(e.target.value as AbuseCategory)}
                  className="mt-1 min-h-[40px] w-full rounded-md border border-[#DADDD8] bg-white px-2.5 py-1.5 text-xs text-[#111618] focus:border-[#0F5C52] focus:outline-none lg:min-h-[32px]"
                >
                  <option value="scam_attempt">Scam attempt</option>
                  <option value="impersonation">Impersonation</option>
                  <option value="harassment">Repeated unwanted requests</option>
                  <option value="other">Other</option>
                </select>
              </div>
            </div>
            <div>
              <label htmlFor="abuse-requester-input" className="block font-medium text-[#525D63]">
                Requester identity or key
              </label>
              <input
                id="abuse-requester-input"
                type="text"
                value={abuseRequester}
                onChange={(e) => setAbuseRequester(e.target.value)}
                placeholder="caller@untrusted.example"
                className="mt-1 min-h-[40px] w-full rounded-md border border-[#DADDD8] bg-white px-2.5 py-1.5 text-xs text-[#111618] placeholder:text-[#525D63] focus:border-[#0F5C52] focus:outline-none lg:min-h-[32px]"
              />
            </div>
            <div>
              <label htmlFor="abuse-summary-input" className="block font-medium text-[#525D63]">
                Incident summary (metadata only)
              </label>
              <input
                id="abuse-summary-input"
                type="text"
                value={abuseSummary}
                onChange={(e) => setAbuseSummary(e.target.value)}
                placeholder="Claimed to be bank fraud support"
                className="mt-1 min-h-[40px] w-full rounded-md border border-[#DADDD8] bg-white px-2.5 py-1.5 text-xs text-[#111618] placeholder:text-[#525D63] focus:border-[#0F5C52] focus:outline-none lg:min-h-[32px]"
              />
            </div>
            <button
              type="submit"
              className="min-h-[44px] w-full rounded-md bg-[#B42318] px-3 py-2 text-xs font-semibold text-white transition-colors hover:bg-[#912018] focus:outline-none focus-visible:ring-2 focus-visible:ring-[#B42318] lg:min-h-[38px]"
            >
              Block &amp; submit abuse report
            </button>
          </form>

          {abuseConfirmation && (
            <p
              role="status"
              className="mt-3 border border-[#B42318]/30 bg-[#FEE4E2] px-3 py-2 text-xs font-medium text-[#912018]"
            >
              {abuseConfirmation}
            </p>
          )}
        </section>
      </div>
    </section>
  );
};
