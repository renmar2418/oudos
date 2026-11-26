import React from "react";
import { Lock } from "lucide-react";
import type { IdleTimeoutOption, PolicyScope, PolicyState } from "../types";

interface PoliciesScreenProps {
  policy: PolicyState;
  onUpdatePolicy: (next: PolicyState) => void;
}

const SCOPE_LABELS: Record<PolicyScope, string> = {
  organisation: "Locked by Organisation",
  group: "Locked by Group",
  device_owner: "Set by Device Owner",
  v0_1_lock: "Locked OFF in v0.1",
};

const IDLE_RANK: Record<IdleTimeoutOption, number> = {
  "15m": 0,
  "30m": 1,
  "1h": 2,
  never: 3,
};

export const PoliciesScreen: React.FC<PoliciesScreenProps> = ({ policy, onUpdatePolicy }) => {
  const toggleMouseKeyboard = () => {
    if (policy.allowMouseKeyboard.lockedBy) return;
    onUpdatePolicy({
      ...policy,
      allowMouseKeyboard: {
        ...policy.allowMouseKeyboard,
        value: !policy.allowMouseKeyboard.value,
      },
    });
  };

  const toggleClipboard = () => {
    if (policy.allowClipboardSync.lockedBy) return;
    onUpdatePolicy({
      ...policy,
      allowClipboardSync: {
        ...policy.allowClipboardSync,
        value: !policy.allowClipboardSync.value,
      },
    });
  };

  const changeIdleTimeout = (nextVal: IdleTimeoutOption) => {
    // Monotonic rule: owners can only tighten (lower rank), never relax.
    if (IDLE_RANK[nextVal] > IDLE_RANK[policy.idleTimeout.value]) return;
    onUpdatePolicy({
      ...policy,
      idleTimeout: { value: nextVal, lockedBy: "device_owner" },
    });
  };

  return (
    <section aria-labelledby="policies-heading" className="space-y-5">
      <div className="border-b border-[#DADDD8] pb-4">
        <h1 id="policies-heading" className="text-2xl font-semibold tracking-tight text-[#111618]">
          Policies
        </h1>
        <p className="mt-1 text-sm text-[#525D63]">
          Strictest rule always wins across Organisation, Group, and Device Owner. You can tighten rules, never loosen them.
        </p>
      </div>

      <div className="divide-y divide-[#DADDD8] border border-[#DADDD8] bg-white">
        <PolicyRow
          title="Ask before connection"
          description="Device owner must explicitly approve every session on their screen."
          lockedBy={policy.askBeforeConnection.lockedBy}
          control={
            <span className="font-mono-code text-xs font-semibold text-[#0F5C52]">
              Required ON
            </span>
          }
        />

        <PolicyRow
          title="Require matching 6-digit code"
          description="Both peers verify the locally derived 6-digit code before access begins."
          lockedBy={policy.requireMatchingCode.lockedBy}
          control={
            <span className="font-mono-code text-xs font-semibold text-[#0F5C52]">
              Required ON
            </span>
          }
        />

        <PolicyRow
          title="Unattended mode"
          description="Background access without owner consent is disabled in Asks-First mode."
          lockedBy={policy.allowUnattended.lockedBy}
          control={
            <span className="font-mono-code text-xs font-semibold text-[#525D63]">
              Locked OFF
            </span>
          }
        />

        <PolicyRow
          title="File transfer"
          description="Chunked file transfer is off by default and blocked when disabled upstream."
          lockedBy={policy.allowFileTransfer.lockedBy}
          control={
            <span className="font-mono-code text-xs font-semibold text-[#525D63]">
              {policy.allowFileTransfer.value ? "Allowed" : "Off"}
            </span>
          }
        />

        <PolicyRow
          title="Allow remote mouse & keyboard ceiling"
          description="Policy ceiling only — still defaults to OFF on every host consent card."
          lockedBy={policy.allowMouseKeyboard.lockedBy}
          control={
            <button
              type="button"
              role="switch"
              aria-checked={policy.allowMouseKeyboard.value}
              aria-label="Allow remote mouse and keyboard ceiling"
              disabled={Boolean(policy.allowMouseKeyboard.lockedBy)}
              onClick={toggleMouseKeyboard}
              className={`rounded-md border px-3 py-1.5 text-xs font-semibold transition-colors focus:outline-none focus-visible:ring-2 focus-visible:ring-[#0F5C52] ${
                policy.allowMouseKeyboard.value
                  ? "border-[#0F5C52] bg-[#0F5C52] text-white"
                  : "border-[#DADDD8] bg-[#F5F6F4] text-[#111618]"
              }`}
            >
              {policy.allowMouseKeyboard.value ? "Allowed" : "Disabled"}
            </button>
          }
        />

        <PolicyRow
          title="Allow clipboard sync ceiling"
          description="Directional text clipboard sync ceiling — still defaults to OFF per session."
          lockedBy={policy.allowClipboardSync.lockedBy}
          control={
            <button
              type="button"
              role="switch"
              aria-checked={policy.allowClipboardSync.value}
              aria-label="Allow clipboard sync ceiling"
              disabled={Boolean(policy.allowClipboardSync.lockedBy)}
              onClick={toggleClipboard}
              className={`rounded-md border px-3 py-1.5 text-xs font-semibold transition-colors focus:outline-none focus-visible:ring-2 focus-visible:ring-[#0F5C52] ${
                policy.allowClipboardSync.value
                  ? "border-[#0F5C52] bg-[#0F5C52] text-white"
                  : "border-[#DADDD8] bg-[#F5F6F4] text-[#111618]"
              }`}
            >
              {policy.allowClipboardSync.value ? "Allowed" : "Disabled"}
            </button>
          }
        />

        <PolicyRow
          title="Idle disconnect timeout"
          description="Automatically revokes the session grant after inactivity."
          lockedBy={policy.idleTimeout.lockedBy}
          control={
            <div>
              <label htmlFor="idle-timeout-select" className="sr-only">
                Idle disconnect timeout
              </label>
              <select
                id="idle-timeout-select"
                value={policy.idleTimeout.value}
                onChange={(e) => changeIdleTimeout(e.target.value as IdleTimeoutOption)}
                className="rounded-md border border-[#DADDD8] bg-white px-3 py-1.5 text-sm text-[#111618] focus:border-[#0F5C52] focus:outline-none focus:ring-2 focus:ring-[#0F5C52]"
              >
                <option value="15m">15 minutes (strictest)</option>
                <option value="30m" disabled={IDLE_RANK["30m"] > IDLE_RANK[policy.idleTimeout.value]}>
                  30 minutes
                </option>
                <option value="1h" disabled={IDLE_RANK["1h"] > IDLE_RANK[policy.idleTimeout.value]}>
                  1 hour
                </option>
                <option value="never" disabled={IDLE_RANK.never > IDLE_RANK[policy.idleTimeout.value]}>
                  Never
                </option>
              </select>
            </div>
          }
        />
      </div>
    </section>
  );
};

interface PolicyRowProps {
  title: string;
  description: string;
  lockedBy: PolicyScope | null;
  control: React.ReactNode;
}

const PolicyRow: React.FC<PolicyRowProps> = ({ title, description, lockedBy, control }) => (
  <div className="flex flex-col gap-3 px-5 py-4 sm:flex-row sm:items-center sm:justify-between">
    <div>
      <div className="flex flex-wrap items-center gap-2.5">
        <h2 className="text-sm font-semibold text-[#111618]">{title}</h2>
        {lockedBy && (
          <span className="inline-flex items-center gap-1 text-xs font-medium text-[#525D63]">
            <Lock className="h-3 w-3 text-[#0F5C52]" aria-hidden="true" />
            {SCOPE_LABELS[lockedBy]}
          </span>
        )}
      </div>
      <p className="mt-1 text-sm text-[#525D63]">{description}</p>
    </div>
    <div className="shrink-0">{control}</div>
  </div>
);
