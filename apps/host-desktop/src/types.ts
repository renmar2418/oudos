export type TrayVisualState = "idle" | "in_session" | "alert";

export type GrantDuration = "this_session" | "one_hour";

export interface HostPermissionSelection {
  view_screen: boolean;
  mouse_keyboard: boolean;
  clipboard_sync: boolean;
  file_transfer: boolean;
}

export interface PolicyCeilingView {
  allow_mouse_keyboard: boolean;
  allow_clipboard_sync: boolean;
  allow_file_transfer: boolean;
  idle_timeout_label: string;
}

export interface HostConsentView {
  session_id: string;
  requester_name: string;
  requester_pubkey: string;
  server_domain: string;
  server_enrolled: boolean;
  sas_code: string;
  first_time_requester: boolean;
  cooling_off_remaining_secs: number;
  ttl_remaining_secs: number;
  permissions: HostPermissionSelection;
  policy_ceilings: PolicyCeilingView;
  duration: GrantDuration;
}

export interface GrantPermissions {
  view_screen: boolean;
  mouse_keyboard: boolean;
  clipboard_sync: boolean;
  file_transfer: boolean;
  blank_screen: boolean;
}

export interface ActiveBannerView {
  session_id: string;
  requester_name: string;
  server_domain: string;
  server_enrolled: boolean;
  status_label: string;
  permissions: GrantPermissions;
  permission_summary: string;
  duration: GrantDuration;
  expires_at_utc: string;
}

export interface AuditSummaryView {
  sequence: number;
  timestamp_utc: string;
  server_domain: string;
  summary: string;
}

export interface HostSnapshot {
  tray_state: TrayVisualState;
  consent_state_name: string;
  pending_request: HostConsentView | null;
  active_banner: ActiveBannerView | null;
  alert_message: string | null;
  audit_entry_count: number;
  blocked_input_count: number;
  dropped_frame_count: number;
  recent_audit: AuditSummaryView[];
}

export interface IncomingConnectionPayload {
  requester_name: string;
  server_domain: string;
  server_enrolled: boolean;
  first_time_requester: boolean;
}
