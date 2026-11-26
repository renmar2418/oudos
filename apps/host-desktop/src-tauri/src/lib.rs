//! Tauri v2 host desktop agent wiring protected consent card, persistent banner,
//! colour-independent Facing Brackets tray icons, and Win32 input isolation.

pub mod host_state;
pub mod tray;

#[cfg(debug_assertions)]
mod dev_harness;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use chrono::Utc;
use oud_capture::{ProtectedSurfaceKind, Rect};
use oud_protocol::grant::GrantDuration;
use oud_protocol::identity::IdentityKeyPair;
use serde::{Deserialize, Serialize};
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{TrayIcon, TrayIconBuilder};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, State};
use uuid::Uuid;

use crate::host_state::{
    decide_window_close_action, HostAbuseReportParams, HostDesktopState, HostPermissionSelection,
    HostSnapshot, IncomingRequestParams, InputOrigin, WindowCloseAction,
};
use crate::tray::{render_tray_rgba, TrayVisualState};

const TRAY_ID: &str = "oudos-host-tray";

/// Shared thread-safe Tauri state wrapper.
pub struct SharedHostState {
    inner: Arc<Mutex<HostDesktopState>>,
    #[cfg(target_os = "windows")]
    _input_guard: Option<oud_capture::WindowsInputGuard>,
}

/// Tray menu handle stored in Tauri state so state updates refresh tooltip and items.
struct TrayUiHandle {
    tray: TrayIcon,
    end_session_item: MenuItem<tauri::Wry>,
}

/// Serializable payload for updating protected window coordinates from UI resize/move events.
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct WindowRectPayload {
    /// Target surface (`"consent_prompt"` or `"session_banner"`).
    pub surface: ProtectedSurfaceKind,
    /// Bounding rectangle in physical screen coordinates.
    pub rect: Rect,
}

/// Serializable payload for `allow_session`.
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct AllowSessionPayload {
    /// Selected permissions (screen required; others optional).
    pub permissions: HostPermissionSelection,
    /// Bounded `v0.1` grant duration (`this_session` or `one_hour`).
    pub duration: GrantDuration,
}

/// Serializable payload for previewing/testing an incoming verified handshake request locally
/// through the real `oud-protocol` handshake and `ConsentMachine`.
#[derive(Debug, Deserialize)]
pub struct IncomingConnectionPayload {
    /// Requester display label.
    pub requester_name: String,
    /// Raw server domain presenting the request.
    pub server_domain: String,
    /// Whether the server key is explicitly enrolled on this host.
    pub server_enrolled: bool,
    /// Whether requester is connecting for the first time (triggers 5s cooling-off).
    pub first_time_requester: bool,
}

/// Serializable state update emitted to Tauri webviews (`host://snapshot`).
#[derive(Debug, Clone, Serialize)]
pub struct SnapshotEvent {
    /// Current host snapshot.
    pub snapshot: HostSnapshot,
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
fn get_host_snapshot(state: State<'_, SharedHostState>) -> Result<HostSnapshot, String> {
    let mut guard = state
        .inner
        .lock()
        .map_err(|_| "host state lock poisoned".to_owned())?;
    Ok(guard.tick(Utc::now()))
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
fn tick_host_state(
    app: AppHandle,
    state: State<'_, SharedHostState>,
    tray_ui: State<'_, Mutex<TrayUiHandle>>,
) -> Result<HostSnapshot, String> {
    let snap = {
        let mut guard = state
            .inner
            .lock()
            .map_err(|_| "host state lock poisoned".to_owned())?;
        guard.tick(Utc::now())
    };
    sync_windows_and_tray(&app, &state, &tray_ui, &snap);
    Ok(snap)
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
fn present_incoming_request(
    app: AppHandle,
    state: State<'_, SharedHostState>,
    tray_ui: State<'_, Mutex<TrayUiHandle>>,
    payload: IncomingConnectionPayload,
) -> Result<HostSnapshot, String> {
    present_incoming_request_impl(&app, &state, &tray_ui, payload, false)
}

pub(crate) fn present_incoming_request_impl(
    app: &AppHandle,
    state: &SharedHostState,
    tray_ui: &Mutex<TrayUiHandle>,
    payload: IncomingConnectionPayload,
    dev_unenrolled_preview: bool,
) -> Result<HostSnapshot, String> {
    let snap = {
        let mut guard = state
            .inner
            .lock()
            .map_err(|_| "host state lock poisoned".to_owned())?;
        let params = IncomingRequestParams {
            session_id: Uuid::now_v7(),
            requester_name: payload.requester_name,
            viewer_keys: IdentityKeyPair::generate(),
            server_domain: payload.server_domain,
            server_enrolled: payload.server_enrolled,
            host_key_matches_pin: true,
            authenticated_requester: true,
            rate_limit_passed: true,
            first_time_requester: payload.first_time_requester,
            requested_permissions: HostPermissionSelection::default(),
        };
        #[cfg(debug_assertions)]
        let res = if dev_unenrolled_preview {
            guard.receive_incoming_request_dev_preview(&params, Utc::now())
        } else {
            guard.receive_incoming_request(&params, Utc::now())
        };
        #[cfg(not(debug_assertions))]
        let res = {
            let _ = dev_unenrolled_preview;
            guard.receive_incoming_request(&params, Utc::now())
        };
        match res {
            Ok(s) => s,
            Err(e) => {
                let s = guard.snapshot(Utc::now());
                drop(guard);
                sync_windows_and_tray(app, state, tray_ui, &s);
                return Err(e.to_string());
            }
        }
    };
    sync_windows_and_tray(app, state, tray_ui, &snap);
    if let Some(consent_win) = app.get_webview_window("consent") {
        let _ = consent_win.unminimize();
        let _ = consent_win.show();
        let _ = consent_win.set_focus();
    }
    Ok(snap)
}

#[cfg(debug_assertions)]
pub(crate) fn dev_reset_host_impl(
    app: &AppHandle,
    state: &SharedHostState,
    tray_ui: &Mutex<TrayUiHandle>,
) -> Result<HostSnapshot, String> {
    let snap = {
        let mut guard = state
            .inner
            .lock()
            .map_err(|_| "host state lock poisoned".to_owned())?;
        guard.dev_reset_to_idle(Utc::now())
    };
    sync_windows_and_tray(app, state, tray_ui, &snap);
    if let Some(consent_win) = app.get_webview_window("consent") {
        let _ = consent_win.unminimize();
        let _ = consent_win.show();
        let _ = consent_win.set_focus();
    }
    Ok(snap)
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
fn allow_session(
    app: AppHandle,
    state: State<'_, SharedHostState>,
    tray_ui: State<'_, Mutex<TrayUiHandle>>,
    payload: AllowSessionPayload,
) -> Result<HostSnapshot, String> {
    let snap = {
        let mut guard = state
            .inner
            .lock()
            .map_err(|_| "host state lock poisoned".to_owned())?;
        guard
            .allow_session(
                InputOrigin::LocalHost,
                payload.permissions,
                payload.duration,
                Utc::now(),
            )
            .map_err(|e| e.to_string())?
    };
    sync_windows_and_tray(&app, &state, &tray_ui, &snap);
    Ok(snap)
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
fn deny_session(
    app: AppHandle,
    state: State<'_, SharedHostState>,
    tray_ui: State<'_, Mutex<TrayUiHandle>>,
) -> Result<HostSnapshot, String> {
    let snap = {
        let mut guard = state
            .inner
            .lock()
            .map_err(|_| "host state lock poisoned".to_owned())?;
        guard.deny_session(Utc::now()).map_err(|e| e.to_string())?
    };
    sync_windows_and_tray(&app, &state, &tray_ui, &snap);
    Ok(snap)
}

#[derive(Debug, Deserialize)]
struct BlockReportPayload {
    category: String,
    summary: String,
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
fn block_and_report_pending_requester(
    app: AppHandle,
    state: State<'_, SharedHostState>,
    tray_ui: State<'_, Mutex<TrayUiHandle>>,
    payload: BlockReportPayload,
) -> Result<HostSnapshot, String> {
    let snap = {
        let mut guard = state
            .inner
            .lock()
            .map_err(|_| "host state lock poisoned".to_owned())?;
        let (pubkey, domain) = guard
            .pending_requester_identity()
            .ok_or_else(|| "no pending request to block/report".to_owned())?;
        let now = Utc::now();
        guard
            .report_abuse(
                InputOrigin::LocalHost,
                HostAbuseReportParams {
                    requester_pubkey: pubkey,
                    server_domain: &domain,
                    category: &payload.category,
                    summary: &payload.summary,
                    also_block: true,
                },
                now,
            )
            .map_err(|e| e.to_string())?;
        guard.snapshot(now)
    };
    sync_windows_and_tray(&app, &state, &tray_ui, &snap);
    Ok(snap)
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
fn update_active_permissions(
    app: AppHandle,
    state: State<'_, SharedHostState>,
    tray_ui: State<'_, Mutex<TrayUiHandle>>,
    permissions: HostPermissionSelection,
) -> Result<HostSnapshot, String> {
    let snap = {
        let mut guard = state
            .inner
            .lock()
            .map_err(|_| "host state lock poisoned".to_owned())?;
        guard
            .update_permissions(InputOrigin::LocalHost, permissions, Utc::now())
            .map_err(|e| e.to_string())?
    };
    sync_windows_and_tray(&app, &state, &tray_ui, &snap);
    Ok(snap)
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
fn end_active_session_local(
    app: AppHandle,
    state: State<'_, SharedHostState>,
    tray_ui: State<'_, Mutex<TrayUiHandle>>,
) -> Result<HostSnapshot, String> {
    let snap = {
        let mut guard = state
            .inner
            .lock()
            .map_err(|_| "host state lock poisoned".to_owned())?;
        guard
            .end_session_local(InputOrigin::LocalHost, Utc::now())
            .map_err(|e| e.to_string())?
    };
    sync_windows_and_tray(&app, &state, &tray_ui, &snap);
    Ok(snap)
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
fn register_protected_window_rect(
    state: State<'_, SharedHostState>,
    payload: WindowRectPayload,
) -> Result<(), String> {
    let guard = state
        .inner
        .lock()
        .map_err(|_| "host state lock poisoned".to_owned())?;
    guard.set_protected_surface_rect(payload.surface, payload.rect, None);
    Ok(())
}

fn sync_windows_and_tray(
    app: &AppHandle,
    state: &SharedHostState,
    tray_ui: &Mutex<TrayUiHandle>,
    snap: &HostSnapshot,
) {
    if let Ok(guard) = tray_ui.lock() {
        let icon = Image::new_owned(render_tray_rgba(snap.tray_state), 32, 32);
        let _ = guard.tray.set_icon(Some(icon));
        let tooltip = match (&snap.active_banner, &snap.alert_message) {
            (Some(banner), _) => format!(
                "Oudos — In Session (LIVE · {} ({}))",
                banner.requester_name, banner.server_domain
            ),
            (None, Some(alert)) if snap.tray_state == TrayVisualState::Alert => {
                format!("Oudos — Alert ({alert})")
            }
            _ => format!("Oudos — {}", snap.tray_state.label()),
        };
        let _ = guard.tray.set_tooltip(Some(&tooltip));
        let _ = guard
            .end_session_item
            .set_enabled(snap.active_banner.is_some());
    }

    if let Some(consent_win) = app.get_webview_window("consent") {
        if snap.pending_request.is_some() {
            let _ = consent_win.show();
            let _ = consent_win.set_always_on_top(true);
            sync_window_rect_to_registry(&consent_win, state, ProtectedSurfaceKind::ConsentPrompt);
        } else if let Ok(guard) = state.inner.lock() {
            guard
                .protected_surfaces()
                .clear_surface(ProtectedSurfaceKind::ConsentPrompt);
        }
    }

    if let Some(banner_win) = app.get_webview_window("banner") {
        if snap.active_banner.is_some() {
            position_banner_top_center(&banner_win);
            let _ = banner_win.show();
            let _ = banner_win.set_always_on_top(true);
            sync_window_rect_to_registry(&banner_win, state, ProtectedSurfaceKind::SessionBanner);
        } else {
            let _ = banner_win.hide();
            if let Ok(guard) = state.inner.lock() {
                guard
                    .protected_surfaces()
                    .clear_surface(ProtectedSurfaceKind::SessionBanner);
            }
        }
    }

    let _ = app.emit(
        "host://snapshot",
        SnapshotEvent {
            snapshot: snap.clone(),
        },
    );
}

fn position_banner_top_center(win: &tauri::WebviewWindow) {
    if let Ok(Some(monitor)) = win.current_monitor() {
        let mon_pos = monitor.position();
        let mon_size = monitor.size();
        let win_size = win.outer_size().unwrap_or(PhysicalSize::new(640, 56));
        let x = mon_pos.x
            + (i32::try_from(mon_size.width).unwrap_or(1280)
                - i32::try_from(win_size.width).unwrap_or(640))
                / 2;
        let y = mon_pos.y + 12;
        let _ = win.set_position(PhysicalPosition::new(x, y));
    }
}

fn sync_window_rect_to_registry(
    win: &tauri::WebviewWindow,
    state: &SharedHostState,
    surface: ProtectedSurfaceKind,
) {
    let hwnd_raw: Option<isize> = {
        #[cfg(target_os = "windows")]
        {
            win.hwnd().ok().map(|h| h.0 as isize)
        }
        #[cfg(not(target_os = "windows"))]
        {
            None
        }
    };
    let outer_pos = win.outer_position().ok().map(|p| (p.x, p.y));
    let outer_size = win.outer_size().ok().map(|s| (s.width, s.height));
    apply_window_rect_update(state, surface, hwnd_raw, outer_pos, outer_size);
}

fn sync_raw_window_rect_to_registry(
    win: &tauri::Window,
    state: &SharedHostState,
    surface: ProtectedSurfaceKind,
) {
    let hwnd_raw: Option<isize> = {
        #[cfg(target_os = "windows")]
        {
            win.hwnd().ok().map(|h| h.0 as isize)
        }
        #[cfg(not(target_os = "windows"))]
        {
            None
        }
    };
    let outer_pos = win.outer_position().ok().map(|p| (p.x, p.y));
    let outer_size = win.outer_size().ok().map(|s| (s.width, s.height));
    apply_window_rect_update(state, surface, hwnd_raw, outer_pos, outer_size);
}

fn apply_window_rect_update(
    state: &SharedHostState,
    surface: ProtectedSurfaceKind,
    hwnd_raw: Option<isize>,
    outer_pos: Option<(i32, i32)>,
    outer_size: Option<(u32, u32)>,
) {
    #[cfg(target_os = "windows")]
    if let Some(hwnd) = hwnd_raw {
        if let Ok(rect) = oud_capture::query_window_rect(hwnd) {
            if let Ok(guard) = state.inner.lock() {
                guard.set_protected_surface_rect(surface, rect, Some(hwnd));
            }
            return;
        }
    }

    if let (Some((x, y)), Some((width, height))) = (outer_pos, outer_size) {
        if let Ok(guard) = state.inner.lock() {
            guard.set_protected_surface_rect(
                surface,
                Rect {
                    x,
                    y,
                    width,
                    height,
                },
                hwnd_raw,
            );
        }
    }
}

fn setup_host_agent(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let show_item = MenuItem::with_id(app, "show_host", "Show Oudos Host", true, None::<&str>)?;
    let end_item = MenuItem::with_id(app, "end_session", "End session", false, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit Oudos", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show_item, &end_item, &quit_item])?;

    let initial_snap = {
        let host_state = app.state::<SharedHostState>();
        host_state.inner.lock().ok().map(|g| g.snapshot(Utc::now()))
    };
    let initial_tray_state = initial_snap
        .as_ref()
        .map_or(TrayVisualState::Idle, |s| s.tray_state);
    let initial_tooltip = initial_snap
        .as_ref()
        .and_then(|s| s.alert_message.as_ref())
        .filter(|_| initial_tray_state == TrayVisualState::Alert)
        .map_or_else(
            || format!("Oudos — {}", initial_tray_state.label()),
            |msg| format!("Oudos — Alert ({msg})"),
        );

    let tray = TrayIconBuilder::with_id(TRAY_ID)
        .icon(Image::new_owned(
            render_tray_rgba(initial_tray_state),
            32,
            32,
        ))
        .tooltip(&initial_tooltip)
        .menu(&menu)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show_host" => {
                if let Some(win) = app.get_webview_window("consent") {
                    let _ = win.show();
                    let _ = win.set_focus();
                }
            }
            "end_session" => {
                let host_state = app.state::<SharedHostState>();
                let tray_ui = app.state::<Mutex<TrayUiHandle>>();
                let snap_res =
                    host_state.inner.lock().ok().and_then(|mut g| {
                        g.end_session_local(InputOrigin::LocalHost, Utc::now()).ok()
                    });
                if let Some(snap) = snap_res {
                    sync_windows_and_tray(app, &host_state, &tray_ui, &snap);
                }
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .build(app)?;

    app.manage(Mutex::new(TrayUiHandle {
        tray,
        end_session_item: end_item,
    }));

    let app_handle = app.handle().clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_millis(500));
        let host_state = app_handle.state::<SharedHostState>();
        let tray_ui = app_handle.state::<Mutex<TrayUiHandle>>();
        let snap_opt = host_state
            .inner
            .lock()
            .ok()
            .map(|mut guard| guard.tick(Utc::now()));
        if let Some(snap) = snap_opt {
            sync_windows_and_tray(&app_handle, &host_state, &tray_ui, &snap);
        }
    });

    #[cfg(debug_assertions)]
    if std::env::var("OUDOS_DEV_HARNESS").as_deref() == Ok("1") {
        dev_harness::spawn_loopback_server(app.handle().clone());
    }

    Ok(())
}

/// Boots the Oudos Tauri v2 Host Desktop agent.
///
/// # Panics
/// Panics only if Tauri runtime initialization fails.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut initial_state = HostDesktopState::with_default_policy(IdentityKeyPair::generate());
    let registry = initial_state.protected_surfaces();

    #[cfg(target_os = "windows")]
    let input_guard = match oud_capture::WindowsInputGuard::install(registry) {
        Ok(guard) => Some(guard),
        Err(err) => {
            initial_state.raise_alert(format!(
                "Protected input guard failed to start ({err}); restart Oudos host before approving connections."
            ));
            None
        }
    };
    #[cfg(not(target_os = "windows"))]
    let _ = registry;

    let shared_inner = Arc::new(Mutex::new(initial_state));
    let shared_state = SharedHostState {
        inner: Arc::clone(&shared_inner),
        #[cfg(target_os = "windows")]
        _input_guard: input_guard,
    };

    tauri::Builder::default()
        .manage(shared_state)
        .on_window_event(handle_window_event)
        .setup(setup_host_agent)
        .invoke_handler(tauri::generate_handler![
            get_host_snapshot,
            tick_host_state,
            present_incoming_request,
            allow_session,
            deny_session,
            block_and_report_pending_requester,
            update_active_permissions,
            end_active_session_local,
            register_protected_window_rect
        ])
        .run(tauri::generate_context!())
        .expect("error while running Oudos host desktop application");
}

fn handle_window_event(window: &tauri::Window, event: &tauri::WindowEvent) {
    let app = window.app_handle();
    let host_state = app.state::<SharedHostState>();

    match event {
        tauri::WindowEvent::Moved(_)
        | tauri::WindowEvent::Resized(_)
        | tauri::WindowEvent::ScaleFactorChanged { .. } => {
            let active_surface =
                host_state
                    .inner
                    .lock()
                    .ok()
                    .and_then(|guard| match window.label() {
                        "consent" if guard.has_pending_request() => {
                            Some(ProtectedSurfaceKind::ConsentPrompt)
                        }
                        "banner" if guard.has_active_banner() => {
                            Some(ProtectedSurfaceKind::SessionBanner)
                        }
                        _ => None,
                    });
            if let Some(surface) = active_surface {
                sync_raw_window_rect_to_registry(window, &host_state, surface);
            }
        }
        tauri::WindowEvent::CloseRequested { api, .. } => {
            let (has_pending, has_active) = host_state.inner.lock().map_or((false, false), |g| {
                (g.has_pending_request(), g.has_active_banner())
            });
            match decide_window_close_action(window.label(), has_pending, has_active) {
                WindowCloseAction::PreventAndKeepVisible => {
                    api.prevent_close();
                }
                WindowCloseAction::PreventAndHide => {
                    api.prevent_close();
                    let _ = window.hide();
                }
                WindowCloseAction::PreventHideAndDenyPending => {
                    api.prevent_close();
                    let _ = window.hide();
                    let denied_snap = host_state
                        .inner
                        .lock()
                        .ok()
                        .and_then(|mut g| g.deny_session(Utc::now()).ok());
                    if let Some(snap) = denied_snap {
                        let tray_ui = app.state::<Mutex<TrayUiHandle>>();
                        sync_windows_and_tray(app, &host_state, &tray_ui, &snap);
                    }
                }
                WindowCloseAction::AllowClose => {}
            }
        }
        _ => {}
    }
}
