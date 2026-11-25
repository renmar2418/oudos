import React from "react";
import { Activity, Clock, FileText, Monitor, PhoneOff, ShieldCheck } from "lucide-react";
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
    <section aria-labelledby="sessions-heading" className="space-y-6">
      <div className="flex flex-col gap-3 sm:flex-row sm:items-end sm:justify-between">
        <div>
          <h1 id="sessions-heading" className="text-2xl font-semibold tracking-tight text-[#14181B]">
            Sessions
          </h1>
          <p className="mt-1 text-sm text-[#4A524E]">
            Track live granted sessions, pending owner approvals, and recent session receipts.
          </p>
        </div>
        <button
          type="button"
          onClick={onOpenAudit}
          className="inline-flex items-center gap-2 rounded-lg border border-[#DADDD8] bg-white px-4 py-2 text-sm font-medium text-[#14181B] hover:bg-[#F5F6F4] focus:outline-none focus:ring-2 focus:ring-[#0F7B6C]"
        >
          <FileText className="h-4 w-4 text-[#0F7B6C]" aria-hidden="true" />
          Open Audit Log
        </button>
      </div>

      <div className="grid gap-4 sm:grid-cols-3">
        <div className="rounded-xl border border-[#DADDD8] bg-white p-4">
          <p className="text-xs font-medium text-[#4A524E]">Active now</p>
          <p className="mt-1 font-mono-code text-2xl font-semibold text-[#074A40]">
            {activeSessions.length}
          </p>
        </div>
        <div className="rounded-xl border border-[#DADDD8] bg-white p-4">
          <p className="text-xs font-medium text-[#4A524E]">Awaiting approval</p>
          <p className="mt-1 font-mono-code text-2xl font-semibold text-[#14181B]">
            {pendingSessions.length}
          </p>
        </div>
        <div className="rounded-xl border border-[#DADDD8] bg-white p-4">
          <p className="text-xs font-medium text-[#4A524E]">Completed</p>
          <p className="mt-1 font-mono-code text-2xl font-semibold text-[#14181B]">
            {endedSessions.length}
          </p>
        </div>
      </div>

      <div className="space-y-4">
        <h2 className="text-base font-semibold text-[#14181B]">Live &amp; Recent Sessions</h2>
        <ul className="space-y-3" aria-label="Session records">
          {sessions.map((session) => (
            <li
              key={session.sessionId}
              className="flex flex-col justify-between gap-4 rounded-xl border border-[#DADDD8] bg-white p-5 sm:flex-row sm:items-center"
            >
              <div className="space-y-1.5">
                <div className="flex flex-wrap items-center gap-2">
                  <span className="inline-flex items-center gap-1.5 text-base font-semibold text-[#14181B]">
                    <Monitor className="h-4 w-4 text-[#0F7B6C]" aria-hidden="true" />
                    {session.deviceAlias}
                  </span>
                  <span className="font-mono-code text-xs text-[#4A524E]">
                    {session.deviceId}
                  </span>
                  {session.status === "active" && (
                    <span className="inline-flex items-center gap-1 rounded-full border border-[#0F7B6C]/30 bg-[#E3F1EE] px-2.5 py-0.5 text-xs font-semibold text-[#074A40]">
                      <Activity className="h-3 w-3" aria-hidden="true" />
                      {session.hostOfflineMidSession ? "Host offline" : "Live"}
                    </span>
                  )}
                  {session.status === "pending" && (
                    <span className="inline-flex items-center gap-1 rounded-full bg-[#FEF0C7] px-2.5 py-0.5 text-xs font-semibold text-[#B54708]">
                      <Clock className="h-3 w-3" aria-hidden="true" />
                      Awaiting approval
                    </span>
                  )}
                  {session.status === "ended" && (
                    <span className="rounded-full border border-[#DADDD8] bg-[#F5F6F4] px-2.5 py-0.5 text-xs font-medium text-[#4A524E]">
                      Ended
                    </span>
                  )}
                </div>

                <p className="text-xs text-[#4A524E]">
                  Requester: <strong className="text-[#14181B]">{session.requester}</strong> ·
                  Approved by: <strong className="text-[#14181B]">{session.approver}</strong> ·{" "}
                  <span className="inline-flex items-center gap-1 text-[#074A40]">
                    <ShieldCheck className="h-3 w-3" aria-hidden="true" />
                    {session.serverDomain}
                  </span>
                </p>

                <p className="text-xs text-[#5C6560]">
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
                      className="rounded-lg bg-[#0F7B6C] px-3.5 py-2 text-xs font-medium text-white hover:bg-[#0A6457] focus:outline-none focus:ring-2 focus:ring-[#0F7B6C]"
                    >
                      Open viewer
                    </button>
                    <button
                      type="button"
                      onClick={() => onEndSession(session.sessionId)}
                      className="inline-flex items-center gap-1 rounded-lg bg-[#B42318] px-3.5 py-2 text-xs font-semibold text-white hover:bg-[#912018] focus:outline-none focus:ring-2 focus:ring-[#B42318]"
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
                    className="rounded-lg border border-[#DADDD8] bg-white px-3.5 py-2 text-xs font-medium text-[#14181B] hover:bg-[#F5F6F4]"
                  >
                    View request
                  </button>
                )}
                {session.status === "ended" && (
                  <button
                    type="button"
                    onClick={() => onViewSummary(session)}
                    className="rounded-lg border border-[#DADDD8] bg-white px-3.5 py-2 text-xs font-medium text-[#14181B] hover:bg-[#F5F6F4]"
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
