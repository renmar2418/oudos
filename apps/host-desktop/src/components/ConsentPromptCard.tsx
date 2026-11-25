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
  }, [request.session_id]);

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
      className="mx-auto flex min-h-[660px] w-[520px] max-w-full flex-col justify-between border border-[#DADDD8] bg-[#F5F6F4] p-5 text-[#14181B]"
    >
      <div className="space-y-3.5">
        <header className="flex items-center justify-between gap-2">
          <div className="flex items-center gap-2">
            <HostFacingBracketsIcon
              state="idle"
              className="h-6 w-6 text-[#14181B]"
              label="Oudos waiting for your decision"
            />
            <span className="font-display text-base font-semibold tracking-[-0.02em] text-[#14181B]">
              Oudos
            </span>
          </div>

          <div className="flex items-center gap-2">
            {request.server_enrolled ? (
              <span
                data-testid="server-domain-chip"
                className="inline-flex items-center gap-1.5 rounded-full border border-[#0F7B6C]/30 bg-[#E3F1EE] px-3 py-1 text-sm font-medium text-[#074A40]"
              >
                <Lock className="h-3.5 w-3.5 shrink-0" aria-hidden="true" />
                <span>{request.server_domain}</span>
              </span>
            ) : (
              <span
                data-testid="server-domain-chip"
                className="inline-flex items-center gap-1.5 rounded-full border border-[#B54708]/40 bg-[#FEF0C7] px-3 py-1 text-sm font-medium text-[#B54708]"
              >
                <ShieldAlert className="h-3.5 w-3.5 shrink-0" aria-hidden="true" />
                <span>{request.server_domain} · Personal / Unverified</span>
              </span>
            )}
          </div>
        </header>

        <div className="flex items-start justify-between gap-3">
          <div>
            <h1 id="consent-card-heading" className="font-display text-xl font-semibold text-[#14181B]">
              <span data-testid="consent-requester-name" className="break-words">
                &ldquo;{request.requester_name}&rdquo;
              </span>{" "}
              wants to see your screen
            </h1>
            <p className="mt-1 text-sm text-[#4A524E]">
              Nothing is shared until you click Allow. You can end this session at any time.
            </p>
          </div>
          <span
            className="inline-flex shrink-0 items-center gap-1 rounded-lg border border-[#DADDD8] bg-white px-2.5 py-1 font-mono-code text-sm font-medium text-[#14181B]"
            aria-live="polite"
            aria-label={`Request expires in ${formatCountdown(ttlRemaining)}`}
          >
            <Clock className="h-4 w-4 text-[#0F7B6C]" aria-hidden="true" />
            {formatCountdown(ttlRemaining)}
          </span>
        </div>

        {request.first_time_requester && (
          <div
            role="alert"
            data-testid="first-time-cooling-warning"
            className="rounded-xl border border-[#B54708]/50 bg-[#FEF0C7] p-3 text-sm text-[#7A2E0E]"
          >
            <div className="flex items-start gap-2">
              <AlertTriangle className="mt-0.5 h-4 w-4 shrink-0 text-[#B54708]" aria-hidden="true" />
              <p className="font-medium leading-snug">
                STOP: Did someone call or message you about your bank, a refund, or a virus? Real
                banks will NEVER ask you to share your screen.
              </p>
            </div>
            {coolingRemaining > 0 && (
              <p className="mt-1.5 font-mono-code text-sm font-semibold text-[#B54708]">
                First-time safety wait: Allow unlocks in {coolingRemaining}s
              </p>
            )}
          </div>
        )}

        <section
          aria-label="6-digit verification code"
          className="rounded-xl border border-[#0F7B6C]/30 bg-[#E3F1EE] px-4 py-3 text-center"
        >
          <p className="text-sm font-semibold text-[#074A40]">6-digit verification code</p>
          <p
            data-testid="sas-code-display"
            className="mt-1 font-mono-code text-3xl font-semibold tracking-widest text-[#14181B]"
          >
            {request.sas_code}
          </p>
          <p className="mt-1 text-sm text-[#074A40]">
            Only allow if the caller&apos;s code matches this exact number.
          </p>
        </section>

        <fieldset className="rounded-xl border border-[#DADDD8] bg-white p-3.5">
          <legend className="px-1 text-sm font-semibold text-[#14181B]">
            What they can do
          </legend>
          <div className="mt-1.5 grid grid-cols-2 gap-2.5 text-sm">
            <label className="flex items-center gap-2 text-[#14181B]">
              <input
                type="checkbox"
                checked
                disabled
                className="h-4 w-4 accent-[#0F7B6C]"
              />
              <span>See screen (required)</span>
            </label>

            <label className="flex items-center gap-2 text-[#14181B]">
              <input
                type="checkbox"
                checked={permissions.mouse_keyboard}
                disabled={!request.policy_ceilings.allow_mouse_keyboard}
                onChange={(e) =>
                  setPermissions((prev) => ({
                    ...prev,
                    mouse_keyboard: e.target.checked,
                  }))
                }
                className="h-4 w-4 accent-[#0F7B6C]"
              />
              <span>Mouse &amp; keyboard</span>
            </label>

            <label className="flex items-center gap-2 text-[#14181B]">
              <input
                type="checkbox"
                checked={permissions.clipboard_sync}
                disabled={!request.policy_ceilings.allow_clipboard_sync}
                onChange={(e) =>
                  setPermissions((prev) => ({
                    ...prev,
                    clipboard_sync: e.target.checked,
                  }))
                }
                className="h-4 w-4 accent-[#0F7B6C]"
              />
              <span>Clipboard sync</span>
            </label>

            <label className="flex items-center gap-2 text-[#14181B]">
              <input
                type="checkbox"
                checked={permissions.file_transfer}
                disabled={!request.policy_ceilings.allow_file_transfer}
                onChange={(e) =>
                  setPermissions((prev) => ({
                    ...prev,
                    file_transfer: e.target.checked,
                  }))
                }
                className="h-4 w-4 accent-[#0F7B6C]"
              />
              <span>
                File transfer
                {!request.policy_ceilings.allow_file_transfer ? " (policy off)" : ""}
              </span>
            </label>
          </div>
        </fieldset>

        <fieldset className="rounded-xl border border-[#DADDD8] bg-white p-3.5">
          <legend className="px-1 text-sm font-semibold text-[#14181B]">
            How long access lasts (v0.1 limit)
          </legend>
          <div className="mt-1.5 grid grid-cols-2 gap-2.5 text-sm">
            <label className="flex cursor-pointer items-center gap-2 rounded-lg border border-[#DADDD8] px-3 py-2 text-[#14181B]">
              <input
                type="radio"
                name="grant-duration"
                value="this_session"
                checked={duration === "this_session"}
                onChange={() => setDuration("this_session")}
                className="h-4 w-4 accent-[#0F7B6C]"
              />
              <span>This session</span>
            </label>
            <label className="flex cursor-pointer items-center gap-2 rounded-lg border border-[#DADDD8] px-3 py-2 text-[#14181B]">
              <input
                type="radio"
                name="grant-duration"
                value="one_hour"
                checked={duration === "one_hour"}
                onChange={() => setDuration("one_hour")}
                className="h-4 w-4 accent-[#0F7B6C]"
              />
              <span>1 hour</span>
            </label>
          </div>
        </fieldset>

        {errorMessage && (
          <p role="alert" className="rounded-lg bg-[#FEE4E2] px-3 py-2 text-sm font-medium text-[#B42318]">
            {errorMessage}
          </p>
        )}
      </div>

      <div className="mt-4 space-y-2 pt-2">
        <div className="flex gap-3">
          <button
            type="button"
            onClick={onDeny}
            disabled={submitting}
            className="flex-1 rounded-lg border border-[#B42318] bg-[#FEE4E2] px-4 py-2.5 text-sm font-semibold text-[#912018] hover:bg-[#FCD5D2] focus:outline-none focus:ring-2 focus:ring-[#B42318]"
          >
            Deny
          </button>
          <button
            type="button"
            onClick={() => onAllow(permissions, duration)}
            disabled={allowBlocked}
            className="flex-1 rounded-lg bg-[#0F7B6C] px-4 py-2.5 text-sm font-semibold text-white hover:bg-[#0A6457] focus:outline-none focus:ring-2 focus:ring-[#074A40] disabled:cursor-not-allowed disabled:bg-[#5C6560]"
          >
            {coolingRemaining > 0 ? `Allow (${coolingRemaining}s)` : "Allow"}
          </button>
        </div>
        {onBlockAndReport && (
          <button
            type="button"
            disabled={submitting}
            onClick={() =>
              onBlockAndReport(
                "scam_attempt",
                "Blocked & reported from host consent prompt"
              )
            }
            className="w-full rounded-lg border border-[#DADDD8] bg-white px-3 py-1.5 text-xs font-medium text-[#912018] hover:bg-[#FEE4E2]/40 focus:outline-none focus:ring-2 focus:ring-[#B42318]"
          >
            Block &amp; report requester
          </button>
        )}
      </div>
    </main>
  );
};
