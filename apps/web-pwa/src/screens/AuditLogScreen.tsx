import React, { useMemo, useState } from "react";
import { Download, Search } from "lucide-react";
import type { AuditEntryItem } from "../types";

interface AuditLogScreenProps {
  entries: AuditEntryItem[];
}

export function buildAuditCsv(entries: AuditEntryItem[]): string {
  const header =
    "sequence,event_id,timestamp_utc,session_id,device_id,actor,server_domain,action,prev_hash,entry_hash,host_signature";
  const rows = entries.map((e) =>
    [
      e.sequence,
      e.eventId,
      e.timestampUtc,
      e.sessionId,
      e.deviceId.replace(/\s+/g, ""),
      e.actor,
      e.serverDomain,
      `"${e.action.replace(/"/g, '""')}"`,
      e.prevHash,
      e.entryHash,
      e.hostSignature,
    ].join(",")
  );
  return [header, ...rows].join("\n");
}

export const AuditLogScreen: React.FC<AuditLogScreenProps> = ({ entries }) => {
  const [query, setQuery] = useState("");

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return entries;
    return entries.filter(
      (e) =>
        e.deviceAlias.toLowerCase().includes(q) ||
        e.actor.toLowerCase().includes(q) ||
        e.action.toLowerCase().includes(q) ||
        e.serverDomain.toLowerCase().includes(q)
    );
  }, [entries, query]);

  const handleExportCsv = () => {
    const csv = buildAuditCsv(filtered);
    const blob = new Blob([csv], { type: "text/csv;charset=utf-8;" });
    const url = URL.createObjectURL(blob);
    const link = document.createElement("a");
    link.href = url;
    link.download = "oudos-audit-log.csv";
    link.click();
    URL.revokeObjectURL(url);
  };

  return (
    <section aria-labelledby="audit-heading" className="space-y-5">
      <div className="flex flex-col gap-3 border-b border-[#DADDD8] pb-4 sm:flex-row sm:items-end sm:justify-between">
        <div>
          <h1 id="audit-heading" className="text-2xl font-semibold tracking-tight text-[#111618]">
            Audit Log
          </h1>
          <p className="mt-1 text-sm text-[#525D63]">
            Tamper-evident SHA-256 hash-chained metadata only. Screen pixels, keystrokes, and file contents are never logged.
          </p>
        </div>

        <button
          type="button"
          onClick={handleExportCsv}
          className="inline-flex min-h-[38px] items-center gap-2 rounded-md bg-[#0F5C52] px-4 py-2 text-sm font-semibold text-white hover:bg-[#0B473F] focus:outline-none focus-visible:ring-2 focus-visible:ring-[#0F5C52] focus-visible:ring-offset-2"
        >
          <Download className="h-4 w-4" aria-hidden="true" />
          Export CSV
        </button>
      </div>

      <div className="relative max-w-md">
        <label htmlFor="audit-search" className="sr-only">
          Filter audit log entries
        </label>
        <Search
          className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-[#525D63]"
          aria-hidden="true"
        />
        <input
          id="audit-search"
          type="search"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="Filter by device, actor, or action..."
          className="w-full rounded-md border border-[#DADDD8] bg-white py-2 pl-9 pr-3 text-sm text-[#111618] placeholder:text-[#525D63] focus:border-[#0F5C52] focus:outline-none focus:ring-2 focus:ring-[#0F5C52]/25"
        />
      </div>

      <div className="overflow-x-auto border border-[#DADDD8] bg-white">
        <table className="w-full border-collapse text-left text-sm">
          <thead>
            <tr className="border-b border-[#DADDD8] bg-[#F5F6F4] text-xs font-medium text-[#525D63]">
              <th scope="col" className="px-4 py-2.5">Seq</th>
              <th scope="col" className="px-4 py-2.5">UTC Time</th>
              <th scope="col" className="px-4 py-2.5">Device</th>
              <th scope="col" className="px-4 py-2.5">Actor</th>
              <th scope="col" className="px-4 py-2.5">Action</th>
              <th scope="col" className="px-4 py-2.5">Entry Hash</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-[#DADDD8]">
            {filtered.map((entry) => (
              <tr key={entry.eventId} className="hover:bg-[#F5F6F4]/60">
                <td className="px-4 py-3 font-mono-code text-xs text-[#525D63]">
                  #{entry.sequence}
                </td>
                <td className="px-4 py-3 font-mono-code text-xs text-[#111618]">
                  {entry.timestampUtc}
                </td>
                <td className="px-4 py-3">
                  <div className="font-medium text-[#111618]">{entry.deviceAlias}</div>
                  <div className="font-mono-code text-xs text-[#525D63]">{entry.deviceId}</div>
                </td>
                <td className="px-4 py-3 text-xs text-[#525D63]">{entry.actor}</td>
                <td className="px-4 py-3 font-medium text-[#111618]">{entry.action}</td>
                <td className="px-4 py-3 font-mono-code text-xs text-[#0F5C52]">
                  {entry.entryHash.slice(0, 14)}…
                </td>
              </tr>
            ))}
            {filtered.length === 0 && (
              <tr>
                <td colSpan={6} className="px-4 py-8 text-center text-sm text-[#525D63]">
                  No matching audit entries.
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
    </section>
  );
};
