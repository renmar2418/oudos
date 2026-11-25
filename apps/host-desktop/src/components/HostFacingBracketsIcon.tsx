import React from "react";
import type { TrayVisualState } from "../types";

interface HostFacingBracketsIconProps {
  state: TrayVisualState;
  className?: string;
  label?: string;
}

/**
 * Colour-independent monochrome Facing Brackets tray & status mark:
 * - idle: open facing brackets with empty center
 * - in_session: facing brackets enclosing a solid filled center dot
 * - alert: facing brackets with center exclamation mark and top-right diamond
 */
export const HostFacingBracketsIcon: React.FC<HostFacingBracketsIconProps> = ({
  state,
  className = "h-6 w-6",
  label,
}) => {
  return (
    <svg
      viewBox="0 0 32 32"
      fill="none"
      className={className}
      role={label ? "img" : undefined}
      aria-label={label}
      aria-hidden={label ? undefined : true}
      data-tray-state={state}
    >
      <path
        d="M11 7H7.5C6.11929 7 5 8.11929 5 9.5V22.5C5 23.8807 6.11929 25 7.5 25H11"
        stroke="currentColor"
        strokeWidth="2.5"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
      <path
        d="M21 7H24.5C25.8807 7 27 8.11929 27 9.5V22.5C27 23.8807 25.8807 25 24.5 25H21"
        stroke="currentColor"
        strokeWidth="2.5"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
      {state === "in_session" && (
        <circle cx="16" cy="16" r="3" fill="currentColor" data-testid="tray-center-dot" />
      )}
      {state === "alert" && (
        <g data-testid="tray-alert-mark">
          <path d="M16 11V17" stroke="currentColor" strokeWidth="2.4" strokeLinecap="round" />
          <circle cx="16" cy="20.5" r="1.4" fill="currentColor" />
          <path d="M27 2L30 5L27 8L24 5L27 2Z" fill="currentColor" />
        </g>
      )}
    </svg>
  );
};
