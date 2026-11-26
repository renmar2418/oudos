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
      className="flex h-[56px] w-full items-center justify-between gap-2.5 border border-[#5FD9BF] bg-[#0E1214] px-3 text-white"
    >
      <div className="flex min-w-0 items-center gap-2">
        <HostFacingBracketsIcon
          state="in_session"
          className="h-5 w-5 shrink-0 text-[#5FD9BF]"
          label="Active session indicator"
        />
        <span
          className="h-2 w-2 shrink-0 rounded-full bg-[#5FD9BF]"
          aria-hidden="true"
        />
        <div className="min-w-0 truncate text-[13px] leading-none">
          <span className="font-mono-code font-semibold tracking-wide text-[#5FD9BF]">
            {banner.status_label} ·
          </span>{" "}
          <span
            data-testid="banner-requester-name"
            className="font-semibold text-white"
          >
            {banner.requester_name}
          </span>{" "}
          <span className="font-mono-code text-xs text-[#A8B4B0]">
            (<span data-testid="banner-server-domain">{banner.server_domain}</span>)
          </span>
          <span className="ml-2 border-l border-[#2A3236] pl-2 text-xs font-medium text-[#D2DAD7]">
            {banner.permission_summary}
          </span>
        </div>
      </div>

      <div className="flex shrink-0 items-center gap-1.5">
        {banner.permissions.mouse_keyboard && onRevokeInput && (
          <button
            type="button"
            onClick={onRevokeInput}
            className="rounded border border-[#B42318]/80 bg-[#2C1211] px-2.5 py-1.5 text-xs font-semibold text-[#FDA29B] transition-colors hover:bg-[#B42318] hover:text-white focus:outline-none focus:ring-2 focus:ring-[#FDA29B]"
          >
            Stop input
          </button>
        )}
        <button
          type="button"
          onClick={onEndSession}
          className="inline-flex items-center gap-1.5 rounded border border-transparent bg-[#B42318] px-3 py-1.5 text-xs font-semibold text-white transition-colors hover:bg-[#912018] focus:outline-none focus:ring-2 focus:ring-white"
        >
          <PhoneOff className="h-3.5 w-3.5" aria-hidden="true" />
          End session
        </button>
      </div>
    </aside>
  );
};
