import React from "react";
import { PhoneOff } from "lucide-react";
import type { ActiveBannerView } from "../types";
import { HostFacingBracketsIcon } from "./HostFacingBracketsIcon";

interface PersistentSessionBannerProps {
  banner: ActiveBannerView;
  onEndSession: () => void;
  onRevokeInput?: () => void;
}

export const PersistentSessionBanner: React.FC<PersistentSessionBannerProps> = ({
  banner,
  onEndSession,
  onRevokeInput,
}) => {
  return (
    <aside
      aria-label="Active remote session controls"
      data-testid="persistent-session-banner"
      className="flex w-full items-center justify-between gap-3 rounded-xl border-2 border-[#5FD9BF] bg-[#0E1214] px-3.5 py-2 text-white shadow-lg"
    >
      <div className="flex min-w-0 items-center gap-2.5">
        <HostFacingBracketsIcon
          state="in_session"
          className="h-5 w-5 shrink-0 text-[#5FD9BF]"
          label="Active session indicator"
        />
        <span className="inline-flex shrink-0 items-center gap-1.5 rounded-full border border-[#5FD9BF]/50 bg-[#141D20] px-2.5 py-0.5 text-sm font-semibold text-[#5FD9BF]">
          <span className="h-2 w-2 rounded-full bg-[#5FD9BF]" aria-hidden="true" />
          {banner.status_label}
        </span>
        <div className="min-w-0 truncate text-sm">
          <span className="font-semibold text-white">
            {banner.status_label} ·{" "}
            <span data-testid="banner-requester-name">&ldquo;{banner.requester_name}&rdquo;</span>{" "}
            (<span data-testid="banner-server-domain">{banner.server_domain}</span>)
          </span>
          <span className="ml-2 text-[#C4CCC8]">· {banner.permission_summary}</span>
        </div>
      </div>

      <div className="flex shrink-0 items-center gap-2">
        {banner.permissions.mouse_keyboard && onRevokeInput && (
          <button
            type="button"
            onClick={onRevokeInput}
            className="rounded-lg border border-[#343E43] bg-[#161C1F] px-2.5 py-1.5 text-sm font-medium text-white hover:border-[#5FD9BF] focus:outline-none focus:ring-2 focus:ring-[#5FD9BF]"
          >
            Stop input
          </button>
        )}
        <button
          type="button"
          onClick={onEndSession}
          className="inline-flex items-center gap-1.5 rounded-lg bg-[#B42318] px-3 py-1.5 text-sm font-semibold text-white hover:bg-[#912018] focus:outline-none focus:ring-2 focus:ring-white"
        >
          <PhoneOff className="h-4 w-4" aria-hidden="true" />
          End session
        </button>
      </div>
    </aside>
  );
};
