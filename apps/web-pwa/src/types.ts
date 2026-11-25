export type CoreTab = "devices" | "requesting" | "session" | "policies" | "audit";

export type DevicePresence = "online" | "in_session" | "offline";

export type QualityPreset = "Auto" | "Sharp" | "Fast";

export type IdleTimeoutOption = "15m" | "30m" | "1h" | "never";

export type PolicyScope = "organisation" | "group" | "device_owner" | "v0_1_lock";

export interface DeviceItem {
  deviceId: string;
  alias: string;
  ownerName: string;
  osName: string;
  presence: DevicePresence;
  serverDomain: string;
  enrolledServer: boolean;
  lastSeenUtc: string;
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
