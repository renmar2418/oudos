import type {
  GrantedPermissions,
  ScreenUnavailableReason,
  SessionEndReason,
} from "../types";

export type IncomingSignalEvent =
  | {
      type: "grant_activated";
      sessionId: string;
      approver: string;
      permissions: GrantedPermissions;
      transportMode?: "direct_p2p" | "turn_relay";
      latencyMs?: number;
    }
  | {
      type: "owner_denied";
      sessionId: string;
      streakCount?: number;
    }
  | {
      type: "request_expired";
      sessionId: string;
    }
  | {
      type: "code_mismatch";
      sessionId: string;
      reportedCode?: string;
      streakCount?: number;
    }
  | {
      type: "host_key_changed";
      sessionId: string;
      expectedPubkey: string;
      presentedPubkey: string;
    }
  | {
      type: "screen_unavailable";
      sessionId: string;
      reason: ScreenUnavailableReason;
    }
  | {
      type: "host_offline";
      sessionId: string;
    }
  | {
      type: "control_plane_offline";
      sessionId?: string;
      reason: string;
    }
  | {
      type: "control_plane_online";
    }
  | {
      type: "session_ended";
      sessionId: string;
      reason: SessionEndReason;
    };

export type OutgoingSignalCommand =
  | {
      type: "session_init";
      sessionId: string;
      targetDeviceId: string;
      sasCode: string;
      requestedPermissions: GrantedPermissions;
    }
  | {
      type: "code_mismatch_abort";
      sessionId: string;
      targetDeviceId: string;
    }
  | {
      type: "cancel_request";
      sessionId: string;
      targetDeviceId: string;
    }
  | {
      type: "end_session";
      sessionId: string;
      targetDeviceId: string;
    }
  | {
      type: "recheck_screen";
      targetDeviceId: string;
    };

export interface SessionTransport {
  subscribe: (listener: (event: IncomingSignalEvent) => void) => () => void;
  send: (command: OutgoingSignalCommand) => void;
}

/**
 * Browser signaling transport connecting to `oud-server` (`/ws/signal`)
 * when available and accepting cross-tab `BroadcastChannel` signaling frames.
 */
export function createBrowserSessionTransport(peerId = "viewer:web-pwa"): SessionTransport {
  const listeners = new Set<(event: IncomingSignalEvent) => void>();
  let socket: WebSocket | null = null;
  let channel: BroadcastChannel | null = null;

  const emit = (event: IncomingSignalEvent) => {
    for (const listener of listeners) {
      listener(event);
    }
  };

  const handlePayload = (raw: string) => {
    try {
      const parsed = JSON.parse(raw) as Record<string, unknown>;
      const mapped = parseSignalEnvelope(parsed);
      if (mapped) {
        emit(mapped);
      }
    } catch {
      // Ignore malformed signaling payloads; fail closed.
    }
  };

  const connect = () => {
    if (typeof window === "undefined") return;
    if ("BroadcastChannel" in window && !channel) {
      channel = new BroadcastChannel("oudos-signaling");
      channel.onmessage = (msg) => {
        if (typeof msg.data === "string") {
          handlePayload(msg.data);
        } else if (msg.data && typeof msg.data === "object") {
          const mapped = parseSignalEnvelope(msg.data as Record<string, unknown>);
          if (mapped) emit(mapped);
        }
      };
    }

    if ("WebSocket" in window && !socket) {
      const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
      const wsUrl = `${protocol}//${window.location.host}/ws/signal?peer_id=${encodeURIComponent(peerId)}`;
      try {
        socket = new WebSocket(wsUrl);
        socket.onmessage = (event) => {
          if (typeof event.data === "string") {
            handlePayload(event.data);
          }
        };
        socket.onerror = () => {
          // Preview/static mode runs without a live signaling server unless attached.
        };
      } catch {
        socket = null;
      }
    }
  };

  return {
    subscribe(listener) {
      listeners.add(listener);
      connect();
      return () => {
        listeners.delete(listener);
        if (listeners.size === 0) {
          socket?.close();
          socket = null;
          channel?.close();
          channel = null;
        }
      };
    },
    send(command) {
      const serialized = JSON.stringify(command);
      if (socket && socket.readyState === WebSocket.OPEN) {
        socket.send(serialized);
      }
      channel?.postMessage(command);
    },
  };
}

function parseSignalEnvelope(
  raw: Record<string, unknown>
): IncomingSignalEvent | null {
  const kind = raw.type;
  const sessionId =
    typeof raw.sessionId === "string"
      ? raw.sessionId
      : typeof raw.session_id === "string"
        ? raw.session_id
        : "";

  switch (kind) {
    case "grant_activated": {
      const permsRaw = (raw.permissions ?? {}) as Record<string, unknown>;
      return {
        type: "grant_activated",
        sessionId,
        approver: typeof raw.approver === "string" ? raw.approver : "Device owner",
        permissions: {
          viewScreen: true,
          mouseKeyboard: Boolean(permsRaw.mouseKeyboard ?? permsRaw.mouse_keyboard),
          clipboardSync: Boolean(permsRaw.clipboardSync ?? permsRaw.clipboard_sync),
          fileTransfer: Boolean(permsRaw.fileTransfer ?? permsRaw.file_transfer),
          blankScreen: false, // Strictly locked false in v0.1 Asks-First mode.
        },
        transportMode:
          raw.transportMode === "turn_relay" ? "turn_relay" : "direct_p2p",
        ...(typeof raw.latencyMs === "number" ? { latencyMs: raw.latencyMs } : {}),
      };
    }
    case "owner_denied":
      return {
        type: "owner_denied",
        sessionId,
        ...(typeof raw.streakCount === "number"
          ? { streakCount: raw.streakCount }
          : {}),
      };
    case "request_expired":
      return { type: "request_expired", sessionId };
    case "code_mismatch":
      return {
        type: "code_mismatch",
        sessionId,
        ...(typeof raw.reportedCode === "string"
          ? { reportedCode: raw.reportedCode }
          : {}),
        ...(typeof raw.streakCount === "number"
          ? { streakCount: raw.streakCount }
          : {}),
      };
    case "host_key_changed":
      return {
        type: "host_key_changed",
        sessionId,
        expectedPubkey: String(raw.expectedPubkey ?? raw.expected_pubkey ?? ""),
        presentedPubkey: String(raw.presentedPubkey ?? raw.presented_pubkey ?? ""),
      };
    case "screen_unavailable": {
      const reason = raw.reason;
      if (
        reason === "locked" ||
        reason === "asleep" ||
        reason === "permission_missing" ||
        reason === "no_display"
      ) {
        return { type: "screen_unavailable", sessionId, reason };
      }
      return { type: "screen_unavailable", sessionId, reason: "permission_missing" };
    }
    case "host_offline":
      return { type: "host_offline", sessionId };
    case "control_plane_offline":
      return {
        type: "control_plane_offline",
        ...(sessionId ? { sessionId } : {}),
        reason:
          typeof raw.reason === "string"
            ? raw.reason
            : "Signaling server unreachable",
      };
    case "control_plane_online":
      return { type: "control_plane_online" };
    case "signaling_dropped":
      return {
        type: "control_plane_offline",
        ...(sessionId ? { sessionId } : {}),
        reason:
          typeof raw.reason === "string"
            ? raw.reason
            : "Signaling channel dropped; fail-closed",
      };
    case "session_ended":
      return {
        type: "session_ended",
        sessionId,
        reason:
          typeof raw.reason === "string"
            ? (raw.reason as SessionEndReason)
            : "Owner clicked End session",
      };
    default:
      return null;
  }
}
