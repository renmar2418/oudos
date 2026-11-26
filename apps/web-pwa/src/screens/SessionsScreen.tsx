import React from "react";
import { FileText, Monitor, PhoneOff, ShieldCheck } from "lucide-react";
import type { SessionRecord } from "../types";

interface SessionsScreenProps {
  sessions: SessionRecord[];
  onOpenSession: (session: SessionRecord) => void;
  onEndSession: (sessionId: string) => void;
  onViewSummary: (session: SessionRecord) => void;
  onOpenAudit: () => void;
}

export const SessionsScreen: React.FC<SessionsScreenProps> = ({
  sessions,
  onOpenSession,
  onEndSession,
  onViewSummary,
  onOpenAudit,
}) => {
  const activeSessions = sessions.filter((s) => s.status === "active");
  const pendingSessions = sessions.filter((s) => s.status === "pending");
  const endedSessions = sessions.filter((s) => s.status === "ended");

  return (
    <section aria-labelledby="sessions-heading" className="space-y-5">
      <div className="flex flex-col gap-3 border-b border-[#DADDD8] pb-4 sm:flex-row sm:items-end sm:justify-between">
        <div>
          <h1 id="sessions-heading" className="text-2xl font-semibold tracking-tight text-[#111618]">
            Sessions
          </h1>
          <p className="mt-1 text-sm text-[#525D63]">
            Track live granted sessions, pending owner approvals, and recent session receipts.
          </p>
        </div>
        <button
          type="button"
          onClick={onOpenAudit}
          className="inline-flex min-h-[38px] items-center gap-2 rounded-md border border-[#DADDD8] bg-white px-4 py-2 text-sm font-medium text-[#111618] hover:bg-[#F5F6F4] focus:outline-none focus-visible:ring-2 focus-visible:ring-[#0F5C52]"
        >
          <FileText className="h-4 w-4 text-[#0F5C52]" aria-hidden="true" />
          Open Audit Log
        </button>
      </div>

      <div className="flex flex-wrap items-center gap-x-6 gap-y-2 border border-[#DADDD8] bg-white px-4 py-2.5 text-xs text-[#525D63]">
        <div className="flex items-center gap-2">
          <span className="h-2 w-2 rounded-full bg-[#0F5C52]" aria-hidden="true" />
          <span>Active now</span>
          <span className="font-mono-code font-semibold text-[#111618]">
            {activeSessions.length}
          </span>
        </div>
        <div className="flex items-center gap-2">
          <span className="h-2 w-2 rounded-full bg-[#B54708]" aria-hidden="true" />
          <span>Awaiting approval</span>
          <span className="font-mono-code font-semibold text-[#111618]">
            {pendingSessions.length}
          </span>
        </div>
        <div className="flex items-center gap-2">
          <span className="h-2 w-2 rounded-full bg-[#8A938E]" aria-hidden="true" />
          <span>Completed</span>
          <span className="font-mono-code font-semibold text-[#111618]">
            {endedSessions.length}
          </span>
        </div>
      </div>

      <div className="border border-[#DADDD8] bg-white">
        <div className="border-b border-[#DADDD8] bg-[#F5F6F4] px-4 py-2.5">
          <h2 className="text-xs font-semibold text-[#525D63]">
            Live &amp; Recent Sessions
          </h2>
        </div>

        <ul className="divide-y divide-[#DADDD8]" aria-label="Session records">
          {sessions.map((session) => (
            <li
              key={session.sessionId}
              className="flex flex-col justify-between gap-4 px-4 py-4 sm:flex-row sm:items-center"
            >
              <div className="space-y-1">
                <div className="flex flex-wrap items-center gap-2.5">
                  <span className="inline-flex items-center gap-2 text-sm font-semibold text-[#111618]">
                    <Monitor className="h-4 w-4 text-[#0F5C52]" aria-hidden="true" />
                    {session.deviceAlias}
                  </span>
                  <span className="font-mono-code text-xs text-[#525D63]">
                    {session.deviceId}
                  </span>
                  {session.status === "active" && (
                    <span className="inline-flex items-center gap-1.5 text-xs font-semibold text-[#0F5C52]">
                      <span className="h-2 w-2 rounded-full bg-[#0F5C52]" aria-hidden="true" />
                      {session.hostOfflineMidSession ? "Host offline" : "Live"}
                    </span>
                  )}
                  {session.status === "pending" && (
                    <span className="inline-flex items-center gap-1.5 text-xs font-semibold text-[#B54708]">
                      <span className="h-2 w-2 rounded-full bg-[#B54708]" aria-hidden="true" />
                      Awaiting approval
                    </span>
                  )}
                  {session.status === "ended" && (
                    <span className="text-xs font-medium text-[#525D63]">Ended</span>
                  )}
                </div>

                <p className="text-xs text-[#525D63]">
                  Requester: <strong className="text-[#111618]">{session.requester}</strong> ·
                  Approved by: <strong className="text-[#111618]">{session.approver}</strong> ·{" "}
                  <span className="inline-flex items-center gap-1 font-mono-code text-[#0F5C52]">
                    <ShieldCheck className="h-3 w-3" aria-hidden="true" />
                    {session.serverDomain}
                  </span>
                </p>

                <p className="font-mono-code text-xs text-[#525D63]">
                  {session.status === "ended"
                    ? `${session.endReason ?? "Ended"} · Duration ${session.durationLabel}`
                    : `Duration ${session.durationLabel} · ${session.transportMode === "direct_p2p" ? "Direct P2P" : "TURN Relay"} · ${session.latencyMs} ms`}
                </p>
              </div>

              <div className="flex flex-wrap gap-2">
                {session.status === "active" && (
                  <>
                    <button
                      type="button"
                      onClick={() => onOpenSession(session)}
                      className="rounded-md bg-[#0F5C52] px-3.5 py-2 text-xs font-semibold text-white hover:bg-[#0B473F] focus:outline-none focus-visible:ring-2 focus-visible:ring-[#0F5C52]"
                    >
                      Open viewer
                    </button>
                    <button
                      type="button"
                      onClick={() => onEndSession(session.sessionId)}
                      className="inline-flex items-center gap-1.5 rounded-md bg-[#B42318] px-3.5 py-2 text-xs font-semibold text-white hover:bg-[#912018] focus:outline-none focus-visible:ring-2 focus-visible:ring-[#B42318]"
                    >
                      <PhoneOff className="h-3.5 w-3.5" aria-hidden="true" />
                      End session
                    </button>
                  </>
                )}
                {session.status === "pending" && (
                  <button
                    type="button"
                    onClick={() => onOpenSession(session)}
                    className="rounded-md border border-[#DADDD8] bg-white px-3.5 py-2 text-xs font-medium text-[#111618] hover:bg-[#F5F6F4]"
                  >
                    View request
                  </button>
                )}
                {session.status === "ended" && (
                  <button
                    type="button"
                    onClick={() => onViewSummary(session)}
                    className="rounded-md border border-[#DADDD8] bg-white px-3.5 py-2 text-xs font-medium text-[#111618] hover:bg-[#F5F6F4]"
                  >
                    View summary
                  </button>
                )}
              </div>
            </li>
          ))}
        </ul>
      </div>
    </section>
  );
};
