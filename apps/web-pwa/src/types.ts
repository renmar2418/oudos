export type CoreTab =
  | "devices"
  | "sessions"
  | "requesting"
  | "session"
  | "policies"
  | "audit";

export type DevicePresence = "online" | "in_session" | "offline";

export type ScreenUnavailableReason =
  | "locked"
  | "asleep"
  | "permission_missing"
  | "no_display";

export type QualityPreset = "Auto" | "Sharp" | "Fast";

export type ClipboardDirection =
  | "disabled"
  | "host_to_viewer"
  | "viewer_to_host"
  | "bidirectional";

export interface FileTransferItem {
  transferId: string;
  filename: string;
  sizeBytes: number;
  transferredBytes: number;
  status: "in_progress" | "completed" | "cancelled" | "rejected";
  statusMessage?: string;
}

export type AbuseCategory =
  | "impersonation"
  | "harassment"
  | "scam_attempt"
  | "other";

export interface SupportLinkItem {
  linkId: string;
  deviceId: string;
  deviceAlias: string;
  serverDomain: string;
  serverPubkey: string;
  supportUrl: string;
  createdAtMs: number;
  expiresAtMs: number;
  redeemed: boolean;
}

export type IdleTimeoutOption = "15m" | "30m" | "1h" | "never";

export type PolicyScope = "organisation" | "group" | "device_owner" | "v0_1_lock";

export interface GrantedPermissions {
  viewScreen: true;
  mouseKeyboard: boolean;
  clipboardSync: boolean;
  fileTransfer: boolean;
  blankScreen: false;
}

export interface DeviceItem {
  deviceId: string;
  alias: string;
  ownerName: string;
  osName: string;
  presence: DevicePresence;
  serverDomain: string;
  enrolledServer: boolean;
  lastSeenUtc: string;
  pinnedPubkey: string;
  presentedPubkey?: string;
  screenUnavailable?: ScreenUnavailableReason | null;
  activeRequester?: string;
  notifyWhenOnline?: boolean;
  accessMode: "asks_first";
}

export interface PolicyState {
  askBeforeConnection: { value: true; lockedBy: PolicyScope };
  requireMatchingCode: { value: true; lockedBy: PolicyScope };
  allowUnattended: { value: false; lockedBy: PolicyScope };
  allowFileTransfer: { value: boolean; lockedBy: PolicyScope | null };
  allowMouseKeyboard: { value: boolean; lockedBy: PolicyScope | null };
  allowClipboardSync: { value: boolean; lockedBy: PolicyScope | null };
  idleTimeout: { value: IdleTimeoutOption; lockedBy: PolicyScope | null };
}

export interface AuditEntryItem {
  sequence: number;
  eventId: string;
  timestampUtc: string;
  sessionId: string;
  deviceAlias: string;
  deviceId: string;
  actor: string;
  serverDomain: string;
  action: string;
  prevHash: string;
  entryHash: string;
  hostSignature: string;
}

export type RequestFlowOutcome =
  | { kind: "awaiting_approval"; isJoinRequest: boolean }
  | { kind: "screen_unavailable"; reason: ScreenUnavailableReason }
  | { kind: "owner_denied"; deniedAtUtc: string; streakCount: number }
  | { kind: "request_expired"; expiredAtUtc: string }
  | { kind: "code_mismatch"; reportedCode?: string; streakCount: number }
  | {
      kind: "device_key_changed";
      expectedPubkey: string;
      presentedPubkey: string;
    }
  | { kind: "control_plane_offline"; reason: string };

export type SessionEndReason =
  | "Owner clicked End session"
  | "You ended the session"
  | "Idle timeout reached"
  | "1-hour grant expired"
  | "Host went offline";

export interface SessionRecord {
  sessionId: string;
  deviceId: string;
  deviceAlias: string;
  ownerName: string;
  requester: string;
  approver: string;
  serverDomain: string;
  enrolledServer: boolean;
  status: "active" | "pending" | "ended";
  startedAtUtc: string;
  endedAtUtc?: string;
  durationLabel: string;
  endReason?: SessionEndReason;
  permissions: GrantedPermissions;
  transportMode: "direct_p2p" | "turn_relay";
  latencyMs: number;
  hostOfflineMidSession?: boolean;
  hostOfflineSinceUtc?: string;
  auditSequence?: number;
  auditEntryHash?: string;
}
