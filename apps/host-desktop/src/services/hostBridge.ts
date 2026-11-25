import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type {
  GrantDuration,
  HostPermissionSelection,
  HostSnapshot,
  IncomingConnectionPayload,
} from "../types";

export interface HostBridge {
  getSnapshot(): Promise<HostSnapshot>;
  tick(): Promise<HostSnapshot>;
  presentIncomingRequest(payload: IncomingConnectionPayload): Promise<HostSnapshot>;
  allowSession(permissions: HostPermissionSelection, duration: GrantDuration): Promise<HostSnapshot>;
  denySession(): Promise<HostSnapshot>;
  blockAndReportPending?(category: string, summary: string): Promise<HostSnapshot>;
  updateActivePermissions(permissions: HostPermissionSelection): Promise<HostSnapshot>;
  endActiveSessionLocal(): Promise<HostSnapshot>;
  subscribe(listener: (snapshot: HostSnapshot) => void): Promise<() => void>;
}

function isTauriRuntime(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export function createTauriHostBridge(): HostBridge {
  if (!isTauriRuntime()) {
    return createInMemoryFallbackBridge();
  }

  return {
    getSnapshot: () => invoke<HostSnapshot>("get_host_snapshot"),
    tick: () => invoke<HostSnapshot>("tick_host_state"),
    presentIncomingRequest: (payload) =>
      invoke<HostSnapshot>("present_incoming_request", { payload }),
    allowSession: (permissions, duration) =>
      invoke<HostSnapshot>("allow_session", { payload: { permissions, duration } }),
    denySession: () => invoke<HostSnapshot>("deny_session"),
    blockAndReportPending: (category, summary) =>
      invoke<HostSnapshot>("block_and_report_pending_requester", {
        payload: { category, summary },
      }),
    updateActivePermissions: (permissions) =>
      invoke<HostSnapshot>("update_active_permissions", { permissions }),
    endActiveSessionLocal: () => invoke<HostSnapshot>("end_active_session_local"),
    subscribe: async (listener) => {
      const unlisten = await listen<{ snapshot: HostSnapshot }>("host://snapshot", (ev) => {
        listener(ev.payload.snapshot);
      });
      return unlisten;
    },
  };
}

function createInMemoryFallbackBridge(): HostBridge {
  let state: HostSnapshot = {
    tray_state: "idle",
    consent_state_name: "IDLE",
    pending_request: null,
    active_banner: null,
    alert_message: null,
    audit_entry_count: 0,
    blocked_input_count: 0,
    dropped_frame_count: 0,
    recent_audit: [],
  };
  const listeners = new Set<(s: HostSnapshot) => void>();

  const publish = (next: HostSnapshot) => {
    state = next;
    for (const cb of listeners) cb(state);
    return state;
  };

  return {
    getSnapshot: async () => state,
    tick: async () => state,
    presentIncomingRequest: async () => state,
    allowSession: async () => state,
    denySession: async () => state,
    updateActivePermissions: async () => state,
    endActiveSessionLocal: async () =>
      publish({
        ...state,
        tray_state: "idle",
        consent_state_name: "REVOKED_LOGGED",
        active_banner: null,
      }),
    subscribe: async (listener) => {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
  };
}
