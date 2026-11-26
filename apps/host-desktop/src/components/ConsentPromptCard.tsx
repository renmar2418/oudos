import React, { useEffect, useState } from "react";
import { AlertTriangle, Clock, Lock, ShieldAlert } from "lucide-react";
import type {
  GrantDuration,
  HostConsentView,
  HostPermissionSelection,
} from "../types";
import { HostFacingBracketsIcon } from "./HostFacingBracketsIcon";

interface ConsentPromptCardProps {
  request: HostConsentView;
  submitting?: boolean;
  errorMessage?: string | null;
  onAllow: (permissions: HostPermissionSelection, duration: GrantDuration) => void;
  onDeny: () => void;
  onBlockAndReport?: (category: string, summary: string) => void;
}

function formatCountdown(secs: number): string {
  const clamped = Math.max(0, secs);
  const mins = Math.floor(clamped / 60);
  const rem = String(clamped % 60).padStart(2, "0");
  return `${mins}:${rem}`;
}

interface PermissionSwitchRowProps {
  id: string;
  label: string;
  note?: string | undefined;
  checked: boolean;
  disabled?: boolean | undefined;
  lockedOn?: boolean | undefined;
  onChange?: ((checked: boolean) => void) | undefined;
}

const PermissionSwitchRow: React.FC<PermissionSwitchRowProps> = ({
  id,
  label,
  note,
  checked,
  disabled = false,
  lockedOn = false,
  onChange,
}) => (
  <label
    htmlFor={id}
    className={`flex items-center justify-between gap-3 px-5 py-2 ${
      disabled ? "cursor-default" : "cursor-pointer hover:bg-[#F5F6F4]/60"
    }`}
  >
    <div className="flex min-w-0 items-baseline gap-2">
      <span className="text-[13px] font-medium text-[#111618]">{label}</span>
      {note && <span className="text-xs text-[#525D63]">{note}</span>}
    </div>

    <span className="relative inline-flex shrink-0 items-center">
      <input
        id={id}
        type="checkbox"
        role="switch"
        checked={checked}
        disabled={disabled}
        onChange={(e) => onChange?.(e.target.checked)}
        className="peer sr-only"
      />
      <span
        aria-hidden="true"
        className={`h-5 w-9 rounded-full border transition-colors peer-focus-visible:ring-2 peer-focus-visible:ring-[#0F5C52] peer-focus-visible:ring-offset-1 ${
          checked
            ? lockedOn
              ? "border-[#0F5C52] bg-[#0F5C52] opacity-75"
              : "border-[#0F5C52] bg-[#0F5C52]"
            : disabled
              ? "border-[#DADDD8] bg-[#E8EBE7]"
              : "border-[#B8BFBA] bg-[#D5DAD6]"
        }`}
      />
      <span
        aria-hidden="true"
        className={`pointer-events-none absolute left-0.5 h-4 w-4 rounded-full bg-white shadow-2xs transition-transform ${
          checked ? "translate-x-4" : "translate-x-0"
        }`}
      />
    </span>
  </label>
);

export const ConsentPromptCard: React.FC<ConsentPromptCardProps> = ({
  request,
  submitting = false,
  errorMessage = null,
  onAllow,
  onDeny,
  onBlockAndReport,
}) => {
  const [permissions, setPermissions] = useState<HostPermissionSelection>({
    view_screen: true,
    mouse_keyboard: false,
    clipboard_sync: false,
    file_transfer: false,
  });
  const [duration, setDuration] = useState<GrantDuration>(request.duration);
  const [coolingRemaining, setCoolingRemaining] = useState<number>(
    request.cooling_off_remaining_secs
  );
  const [ttlRemaining, setTtlRemaining] = useState<number>(request.ttl_remaining_secs);

  useEffect(() => {
    setPermissions({
      view_screen: true,
      mouse_keyboard: false,
      clipboard_sync: false,
      file_transfer: false,
    });
    setDuration(request.duration);
    setCoolingRemaining(request.cooling_off_remaining_secs);
    setTtlRemaining(request.ttl_remaining_secs);
  }, [request.session_id, request.duration, request.cooling_off_remaining_secs, request.ttl_remaining_secs]);

  useEffect(() => {
    setCoolingRemaining(request.cooling_off_remaining_secs);
    setTtlRemaining(request.ttl_remaining_secs);
  }, [request.cooling_off_remaining_secs, request.ttl_remaining_secs]);

  useEffect(() => {
    if (coolingRemaining <= 0 && ttlRemaining <= 0) return;
    const timer = window.setInterval(() => {
      setCoolingRemaining((prev) => Math.max(0, prev - 1));
      setTtlRemaining((prev) => Math.max(0, prev - 1));
    }, 1000);
    return () => window.clearInterval(timer);
  }, [coolingRemaining > 0, ttlRemaining > 0]);

  const allowBlocked = submitting || coolingRemaining > 0 || ttlRemaining <= 0;

  return (
    <main
      aria-labelledby="consent-card-heading"
      className="mx-auto flex min-h-[660px] w-full max-w-[520px] flex-col bg-white text-[#111618]"
    >
      <header className="flex items-center justify-between border-b border-[#DADDD8] bg-white px-5 py-2.5">
        <div className="flex items-center gap-2">
          <HostFacingBracketsIcon
            state="idle"
            className="h-5 w-5 text-[#0F5C52]"
            label="Oudos waiting for your decision"
          />
          <span className="font-display text-sm font-semibold tracking-[-0.02em] text-[#111618]">
            Oudos
          </span>
        </div>

        <span
          className="inline-flex items-center gap-1.5 font-mono-code text-xs font-semibold text-[#525D63]"
          aria-live="polite"
          aria-label={`Request expires in ${formatCountdown(ttlRemaining)}`}
        >
          <Clock className="h-3.5 w-3.5 text-[#0F5C52]" aria-hidden="true" />
          Expires in {formatCountdown(ttlRemaining)}
        </span>
      </header>

      {!request.server_enrolled && (
        <div
          role="status"
          className="flex items-center gap-2 border-b border-[#B54708]/35 bg-[#FEF0C7] px-5 py-2 text-xs font-semibold text-[#7A2E0E]"
        >
          <ShieldAlert className="h-4 w-4 shrink-0 text-[#B54708]" aria-hidden="true" />
          <span>Unverified relay — this computer has not enrolled this server domain.</span>
        </div>
      )}

      <section className="border-b border-[#DADDD8] bg-white px-5 py-3">
        <h1
          id="consent-card-heading"
          className="font-display text-[20px] font-semibold leading-snug tracking-[-0.015em] text-[#111618]"
        >
          <span data-testid="consent-requester-name" className="break-words">
            {request.requester_name}
          </span>{" "}
          wants to see your screen
        </h1>

        <div className="mt-1 flex flex-wrap items-center gap-2">
          {request.server_enrolled ? (
            <span
              data-testid="server-domain-chip"
              className="inline-flex items-center gap-1.5 font-mono-code text-xs font-semibold text-[#0F5C52]"
            >
              <Lock className="h-3.5 w-3.5 shrink-0" aria-hidden="true" />
              <span>{request.server_domain}</span>
            </span>
          ) : (
            <span
              data-testid="server-domain-chip"
              className="inline-flex items-center gap-1.5 font-mono-code text-xs font-semibold text-[#B54708]"
            >
              <ShieldAlert className="h-3.5 w-3.5 shrink-0" aria-hidden="true" />
              <span>{request.server_domain} · Personal / Unverified</span>
            </span>
          )}
        </div>

        <p className="mt-1 text-[13px] text-[#525D63]">
          Nothing is shared until you click Allow. You can end this session at any time.
        </p>
      </section>

      <section
        aria-label="6-digit verification code"
        className="border-b border-[#DADDD8] bg-[#F5F6F4] px-5 py-3 text-center"
      >
        <p
          data-testid="sas-code-display"
          className="font-mono-code text-[30px] font-semibold leading-none tracking-[0.16em] text-[#111618]"
        >
          {request.sas_code}
        </p>
        <p className="mt-1.5 text-[13px] font-medium text-[#525D63]">
          Only allow if the caller&apos;s 6-digit code matches this exact number.
        </p>
      </section>

      {request.first_time_requester && (
        <div
          role="alert"
          data-testid="first-time-cooling-warning"
          className="border-b border-[#B54708]/35 bg-[#FEF0C7] px-5 py-2.5 text-[#7A2E0E]"
        >
          <div className="flex items-start gap-2">
            <AlertTriangle className="mt-0.5 h-4 w-4 shrink-0 text-[#B54708]" aria-hidden="true" />
            <div className="text-[13px] leading-snug">
              <p className="font-medium">
                STOP: Did someone call or message you about your bank, a refund, or a virus? Real
                banks will NEVER ask you to share your screen.
              </p>
              {coolingRemaining > 0 && (
                <p className="mt-1 font-mono-code text-xs font-semibold text-[#B54708]">
                  Safety wait: Allow unlocks in {coolingRemaining}s
                </p>
              )}
            </div>
          </div>
        </div>
      )}

      <fieldset className="border-b border-[#DADDD8] bg-white">
        <legend className="sr-only">Permissions for this session</legend>
        <div className="divide-y divide-[#DADDD8]">
          <PermissionSwitchRow
            id="perm-view-screen"
            label="See your screen"
            note="Required"
            checked
            disabled
            lockedOn
          />
          <PermissionSwitchRow
            id="perm-mouse-keyboard"
            label="Mouse & keyboard"
            note={!request.policy_ceilings.allow_mouse_keyboard ? "Off by policy" : undefined}
            checked={permissions.mouse_keyboard}
            disabled={!request.policy_ceilings.allow_mouse_keyboard}
            onChange={(checked) =>
              setPermissions((prev) => ({ ...prev, mouse_keyboard: checked }))
            }
          />
          <PermissionSwitchRow
            id="perm-clipboard"
            label="Clipboard"
            note={!request.policy_ceilings.allow_clipboard_sync ? "Off by policy" : undefined}
            checked={permissions.clipboard_sync}
            disabled={!request.policy_ceilings.allow_clipboard_sync}
            onChange={(checked) =>
              setPermissions((prev) => ({ ...prev, clipboard_sync: checked }))
            }
          />
          <PermissionSwitchRow
            id="perm-file-transfer"
            label="File transfer"
            note={!request.policy_ceilings.allow_file_transfer ? "Off by policy" : undefined}
            checked={permissions.file_transfer}
            disabled={!request.policy_ceilings.allow_file_transfer}
            onChange={(checked) =>
              setPermissions((prev) => ({ ...prev, file_transfer: checked }))
            }
          />
        </div>
      </fieldset>

      <fieldset className="flex flex-1 items-center justify-between gap-3 border-b border-[#DADDD8] bg-white px-5 py-2.5">
        <legend className="sr-only">Access duration</legend>
        <span className="text-[13px] font-medium text-[#111618]" aria-hidden="true">
          Access duration
        </span>
        <div className="inline-grid grid-cols-2 rounded-md border border-[#DADDD8] bg-[#F5F6F4] p-0.5 text-xs font-medium">
          <label
            className={`cursor-pointer rounded-[4px] px-3 py-1 text-center transition-colors ${
              duration === "this_session"
                ? "bg-white font-semibold text-[#111618] shadow-2xs"
                : "text-[#525D63] hover:text-[#111618]"
            }`}
          >
            <input
              type="radio"
              name="grant-duration"
              value="this_session"
              checked={duration === "this_session"}
              onChange={() => setDuration("this_session")}
              className="sr-only"
            />
            <span>This session</span>
          </label>
          <label
            className={`cursor-pointer rounded-[4px] px-3 py-1 text-center transition-colors ${
              duration === "one_hour"
                ? "bg-white font-semibold text-[#111618] shadow-2xs"
                : "text-[#525D63] hover:text-[#111618]"
            }`}
          >
            <input
              type="radio"
              name="grant-duration"
              value="one_hour"
              checked={duration === "one_hour"}
              onChange={() => setDuration("one_hour")}
              className="sr-only"
            />
            <span>1 hour</span>
          </label>
        </div>
      </fieldset>

      {errorMessage && (
        <p
          role="alert"
          className="border-b border-[#B42318]/30 bg-[#FEE4E2] px-5 py-2 text-xs font-medium text-[#912018]"
        >
          {errorMessage}
        </p>
      )}

      <footer className="bg-[#F5F6F4] px-5 py-3">
        <div className="grid grid-cols-2 gap-3">
          <button
            type="button"
            onClick={onDeny}
            disabled={submitting}
            className="rounded-md border border-[#B42318] bg-[#FEE4E2] px-4 py-2.5 text-sm font-semibold text-[#912018] transition-colors hover:bg-[#FCD5D2] focus:outline-none focus:ring-2 focus:ring-[#B42318]"
          >
            Deny
          </button>
          <button
            type="button"
            onClick={() => onAllow(permissions, duration)}
            disabled={allowBlocked}
            className="rounded-md bg-[#0F5C52] px-4 py-2.5 text-sm font-semibold text-white transition-colors hover:bg-[#0B473F] focus:outline-none focus:ring-2 focus:ring-[#083630] disabled:cursor-not-allowed disabled:bg-[#DCE0DC] disabled:text-[#384146]"
          >
            {coolingRemaining > 0 ? `Allow (${coolingRemaining}s)` : "Allow"}
          </button>
        </div>
        {onBlockAndReport && (
          <div className="mt-2 text-center">
            <button
              type="button"
              disabled={submitting}
              onClick={() =>
                onBlockAndReport(
                  "scam_attempt",
                  "Blocked & reported from host consent prompt"
                )
              }
              className="rounded px-2 py-1 text-xs font-medium text-[#B42318] hover:underline focus:outline-none focus:ring-2 focus:ring-[#B42318]"
            >
              Block &amp; report requester
            </button>
          </div>
        )}
      </footer>
    </main>
  );
};
