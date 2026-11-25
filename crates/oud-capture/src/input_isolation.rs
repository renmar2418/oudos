//! Protected host consent/banner input isolation:
//! 1. Remote pointer coordinate blocking over active consent card and session banner rects.
//! 2. Windows `WH_MOUSE_LL` (`LLMHF_INJECTED`) and `WH_KEYBOARD_LL` (`LLKHF_INJECTED`)
//!    rejection so synthetic input cannot click `Allow` or dismiss the persistent banner.
//! 3. Metadata-only [`AuditEventKind::SyntheticInputBlocked`] audit recording.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use chrono::{DateTime, Utc};
use oud_protocol::audit::{AuditChain, AuditEvent, AuditEventKind};
use oud_protocol::identity::{IdentityKeyPair, IdentityPublicKey};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::capture::Rect;
use crate::error::CaptureError;

/// Protected host surfaces shielded from remote and synthetic input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtectedSurfaceKind {
    /// Host `520x660` consent prompt card.
    ConsentPrompt,
    /// Persistent always-on-top `End session` banner.
    SessionBanner,
}

impl ProtectedSurfaceKind {
    /// Returns stable metadata label for errors and audit entries.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ConsentPrompt => "consent_prompt",
            Self::SessionBanner => "session_banner",
        }
    }
}

/// Metadata-only record of a blocked synthetic or protected-rect input attempt.
///
/// Never stores screen coordinates, keystrokes, or clipboard/file payloads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyntheticBlockRecord {
    /// UTC timestamp when the attempt was blocked.
    pub timestamp_utc: DateTime<Utc>,
    /// Protected UI surface targeted.
    pub surface: ProtectedSurfaceKind,
    /// Static reason tag (`"protected_rect"`, `"llmhf_injected"`, `"llkhf_injected"`).
    pub reason: &'static str,
}

#[derive(Debug, Clone, Copy)]
struct SurfaceRegistration {
    rect: Rect,
    hwnd_raw: Option<isize>,
}

#[derive(Debug)]
struct RegistryInner {
    consent_prompt: Option<SurfaceRegistration>,
    session_banner: Option<SurfaceRegistration>,
    pending_audit: Vec<SyntheticBlockRecord>,
    total_blocked: AtomicU64,
}

impl Default for RegistryInner {
    fn default() -> Self {
        Self {
            consent_prompt: None,
            session_banner: None,
            pending_audit: Vec::new(),
            total_blocked: AtomicU64::new(0),
        }
    }
}

/// Thread-safe registry of active protected consent/banner window rects and blocked-input audit buffer.
#[derive(Debug, Clone, Default)]
pub struct ProtectedSurfaceRegistry {
    inner: Arc<Mutex<RegistryInner>>,
}

impl ProtectedSurfaceRegistry {
    /// Creates an empty protected-surface registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers or updates the bounding [`Rect`] (and optional Win32 `HWND` integer handle)
    /// for `surface`.
    pub fn set_surface(&self, surface: ProtectedSurfaceKind, rect: Rect, hwnd_raw: Option<isize>) {
        if let Ok(mut guard) = self.inner.lock() {
            let entry = Some(SurfaceRegistration { rect, hwnd_raw });
            match surface {
                ProtectedSurfaceKind::ConsentPrompt => guard.consent_prompt = entry,
                ProtectedSurfaceKind::SessionBanner => guard.session_banner = entry,
            }
        }
    }

    /// Clears the protected registration for `surface` (e.g., when hidden/closed).
    pub fn clear_surface(&self, surface: ProtectedSurfaceKind) {
        if let Ok(mut guard) = self.inner.lock() {
            match surface {
                ProtectedSurfaceKind::ConsentPrompt => guard.consent_prompt = None,
                ProtectedSurfaceKind::SessionBanner => guard.session_banner = None,
            }
        }
    }

    /// Returns the registered bounding [`Rect`] for `surface`, if active.
    #[must_use]
    pub fn get_rect(&self, surface: ProtectedSurfaceKind) -> Option<Rect> {
        let guard = self.inner.lock().ok()?;
        match surface {
            ProtectedSurfaceKind::ConsentPrompt => guard.consent_prompt.map(|s| s.rect),
            ProtectedSurfaceKind::SessionBanner => guard.session_banner.map(|s| s.rect),
        }
    }

    /// Hit-tests screen coordinates `(x, y)` against active protected surfaces.
    ///
    /// Checks `SessionBanner` first, then `ConsentPrompt`.
    #[must_use]
    pub fn hit_test(&self, x: i32, y: i32) -> Option<ProtectedSurfaceKind> {
        let guard = self.inner.lock().ok()?;
        if let Some(banner) = guard.session_banner {
            if point_in_rect(banner.rect, x, y) {
                return Some(ProtectedSurfaceKind::SessionBanner);
            }
        }
        if let Some(consent) = guard.consent_prompt {
            if point_in_rect(consent.rect, x, y) {
                return Some(ProtectedSurfaceKind::ConsentPrompt);
            }
        }
        None
    }

    /// Returns the protected surface owning `hwnd_raw` if registered, or `ConsentPrompt`
    /// if a consent dialog is currently active and `hwnd_raw` is unknown/unmatched.
    #[must_use]
    pub fn active_keyboard_surface_for_hwnd(
        &self,
        hwnd_raw: isize,
    ) -> Option<ProtectedSurfaceKind> {
        let guard = self.inner.lock().ok()?;
        if let Some(consent) = guard.consent_prompt {
            if consent.hwnd_raw == Some(hwnd_raw) || consent.hwnd_raw.is_none() {
                return Some(ProtectedSurfaceKind::ConsentPrompt);
            }
        }
        if let Some(banner) = guard.session_banner {
            if banner.hwnd_raw == Some(hwnd_raw) {
                return Some(ProtectedSurfaceKind::SessionBanner);
            }
        }
        None
    }

    /// Blocks a remote pointer action at `(x, y)` if it lands inside an active consent or banner rect,
    /// recording a metadata-only block event for the audit log.
    ///
    /// # Errors
    /// Returns [`CaptureError::ProtectedSurfaceInputBlocked`] when `(x, y)` lies inside
    /// a protected surface rect.
    pub fn enforce_remote_pointer(
        &self,
        x: i32,
        y: i32,
        now: DateTime<Utc>,
    ) -> Result<(), CaptureError> {
        if let Some(surface) = self.hit_test(x, y) {
            self.record_block(surface, "protected_rect", now);
            return Err(CaptureError::ProtectedSurfaceInputBlocked {
                surface: surface.as_str(),
                reason: "protected_rect",
            });
        }
        Ok(())
    }

    /// Records a blocked synthetic/protected input event into the metadata queue.
    pub fn record_block(
        &self,
        surface: ProtectedSurfaceKind,
        reason: &'static str,
        timestamp_utc: DateTime<Utc>,
    ) {
        if let Ok(mut guard) = self.inner.lock() {
            guard.total_blocked.fetch_add(1, Ordering::Relaxed);
            guard.pending_audit.push(SyntheticBlockRecord {
                timestamp_utc,
                surface,
                reason,
            });
        }
    }

    /// Total number of blocked remote-rect or synthetic input attempts observed.
    #[must_use]
    pub fn total_blocked(&self) -> u64 {
        self.inner
            .lock()
            .map_or(0, |guard| guard.total_blocked.load(Ordering::Relaxed))
    }

    /// Drains pending block records without modifying the monotonic total counter.
    #[must_use]
    pub fn drain_blocked_records(&self) -> Vec<SyntheticBlockRecord> {
        self.inner.lock().map_or_else(
            |_| Vec::new(),
            |mut guard| std::mem::take(&mut guard.pending_audit),
        )
    }

    /// Drains all pending blocked-input records and appends them as signed, hash-chained
    /// [`AuditEventKind::SyntheticInputBlocked`] entries on `chain`.
    ///
    /// Returns the number of audit entries appended.
    ///
    /// # Errors
    /// Returns [`CaptureError::GrantError`] if signing or hash chaining fails.
    pub fn flush_to_audit_chain(
        &self,
        chain: &mut AuditChain,
        host_keys: &IdentityKeyPair,
        session_id: Uuid,
        actor_pubkey: IdentityPublicKey,
        server_domain: &str,
    ) -> Result<usize, CaptureError> {
        let records = self.drain_blocked_records();
        let count = records.len();
        let device_pubkey = host_keys.public_key();
        for rec in records {
            chain.append(
                AuditEvent {
                    event_id: Uuid::now_v7(),
                    timestamp_utc: rec.timestamp_utc,
                    session_id,
                    device_pubkey,
                    actor_pubkey,
                    server_domain: server_domain.to_owned(),
                    kind: AuditEventKind::SyntheticInputBlocked {
                        surface: rec.surface.as_str().to_owned(),
                        reason: rec.reason.to_owned(),
                    },
                },
                host_keys,
            )?;
        }
        Ok(count)
    }
}

/// Checks whether `(x, y)` falls inside `rect` using overflow-safe 64-bit arithmetic
/// on half-open `[x, x + width) x [y, y + height)` bounds.
#[must_use]
pub fn point_in_rect(rect: Rect, x: i32, y: i32) -> bool {
    if rect.width == 0 || rect.height == 0 {
        return false;
    }
    let left = i64::from(rect.x);
    let top = i64::from(rect.y);
    let right = left.saturating_add(i64::from(rect.width));
    let bottom = top.saturating_add(i64::from(rect.height));
    let px = i64::from(x);
    let py = i64::from(y);
    px >= left && px < right && py >= top && py < bottom
}

/// Returns `true` when Win32 `MSLLHOOKSTRUCT::flags` marks injected synthetic mouse input
/// (`LLMHF_INJECTED = 0x1` or `LLMHF_LOWER_IL_INJECTED = 0x2`).
#[must_use]
pub const fn is_synthetic_mouse_flags(flags: u32) -> bool {
    (flags & (0x0000_0001 | 0x0000_0002)) != 0
}

/// Returns `true` when Win32 `KBDLLHOOKSTRUCT::flags` marks injected synthetic keyboard input
/// (`LLKHF_INJECTED = 0x10` or `LLKHF_LOWER_IL_INJECTED = 0x2`).
#[must_use]
pub const fn is_synthetic_keyboard_flags(flags: u32) -> bool {
    (flags & (0x0000_0010 | 0x0000_0002)) != 0
}

#[cfg(target_os = "windows")]
mod windows_hooks {
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::sync::{mpsc, Arc, Mutex, OnceLock};
    use std::thread::{self, JoinHandle};

    use chrono::Utc;
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
    use windows::Win32::System::Threading::GetCurrentThreadId;
    use windows::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, DispatchMessageW, GetForegroundWindow, GetMessageW, GetWindowRect,
        PostThreadMessageW, SetWindowsHookExW, TranslateMessage, UnhookWindowsHookEx, HHOOK,
        KBDLLHOOKSTRUCT, LLKHF_INJECTED, LLKHF_LOWER_IL_INJECTED, LLMHF_INJECTED,
        LLMHF_LOWER_IL_INJECTED, MSG, MSLLHOOKSTRUCT, WH_KEYBOARD_LL, WH_MOUSE_LL, WM_QUIT,
    };

    use super::{
        is_synthetic_keyboard_flags, is_synthetic_mouse_flags, ProtectedSurfaceKind,
        ProtectedSurfaceRegistry,
    };
    use crate::capture::Rect;
    use crate::error::CaptureError;

    const HC_ACTION_CODE: i32 = 0;

    static NEXT_GUARD_ID: AtomicU64 = AtomicU64::new(1);
    static HOOK_REGISTRIES: OnceLock<Mutex<Vec<(u64, ProtectedSurfaceRegistry)>>> = OnceLock::new();

    fn registries() -> &'static Mutex<Vec<(u64, ProtectedSurfaceRegistry)>> {
        HOOK_REGISTRIES.get_or_init(|| Mutex::new(Vec::new()))
    }

    /// Evaluates a Win32 `MSLLHOOKSTRUCT` payload against `registry`, blocking when
    /// `LLMHF_INJECTED` / `LLMHF_LOWER_IL_INJECTED` targets any active protected surface rect.
    #[must_use]
    pub fn evaluate_mouse_ll_hook(
        registry: &ProtectedSurfaceRegistry,
        info: &MSLLHOOKSTRUCT,
    ) -> Option<ProtectedSurfaceKind> {
        let injected_mask = LLMHF_INJECTED | LLMHF_LOWER_IL_INJECTED;
        if (info.flags & injected_mask) == 0 && !is_synthetic_mouse_flags(info.flags) {
            return None;
        }
        let surface = registry.hit_test(info.pt.x, info.pt.y)?;
        registry.record_block(surface, "llmhf_injected", Utc::now());
        Some(surface)
    }

    /// Evaluates a Win32 `KBDLLHOOKSTRUCT` payload against `registry` and the current foreground
    /// window handle, blocking when `LLKHF_INJECTED` / `LLKHF_LOWER_IL_INJECTED` targets a
    /// protected consent or session banner surface.
    #[must_use]
    pub fn evaluate_keyboard_ll_hook(
        registry: &ProtectedSurfaceRegistry,
        info: &KBDLLHOOKSTRUCT,
        foreground_hwnd_raw: isize,
    ) -> Option<ProtectedSurfaceKind> {
        let injected_mask = LLKHF_INJECTED.0 | LLKHF_LOWER_IL_INJECTED.0;
        if (info.flags.0 & injected_mask) == 0 && !is_synthetic_keyboard_flags(info.flags.0) {
            return None;
        }
        let surface = registry.active_keyboard_surface_for_hwnd(foreground_hwnd_raw)?;
        registry.record_block(surface, "llkhf_injected", Utc::now());
        Some(surface)
    }

    /// Queries the live OS bounding [`Rect`] of a Win32 `HWND` via `GetWindowRect`.
    ///
    /// # Errors
    /// Returns [`CaptureError::CaptureFailed`] if `GetWindowRect` fails or returns non-positive dimensions.
    pub fn query_window_rect(hwnd_raw: isize) -> Result<Rect, CaptureError> {
        if hwnd_raw == 0 {
            return Err(CaptureError::CaptureFailed(
                "cannot query GetWindowRect for null HWND".to_owned(),
            ));
        }
        let hwnd = HWND(hwnd_raw as *mut core::ffi::c_void);
        let mut rect = RECT::default();
        // SAFETY: `GetWindowRect` writes to a valid stack `RECT` for the given `HWND`.
        unsafe {
            GetWindowRect(hwnd, &raw mut rect)
                .map_err(|e| CaptureError::CaptureFailed(format!("GetWindowRect failed: {e}")))?;
        }
        let width = rect
            .right
            .checked_sub(rect.left)
            .and_then(|w| u32::try_from(w).ok())
            .filter(|&w| w > 0)
            .ok_or_else(|| {
                CaptureError::CaptureFailed("GetWindowRect returned non-positive width".to_owned())
            })?;
        let height = rect
            .bottom
            .checked_sub(rect.top)
            .and_then(|h| u32::try_from(h).ok())
            .filter(|&h| h > 0)
            .ok_or_else(|| {
                CaptureError::CaptureFailed("GetWindowRect returned non-positive height".to_owned())
            })?;
        Ok(Rect {
            x: rect.left,
            y: rect.top,
            width,
            height,
        })
    }

    /// Active Win32 low-level mouse & keyboard hook guard rejecting `LLMHF_INJECTED` and
    /// `LLKHF_INJECTED` events targeted at protected consent/banner surfaces.
    pub struct WindowsInputGuard {
        guard_id: u64,
        thread_id: u32,
        stopped: Arc<AtomicBool>,
        join_handle: Option<JoinHandle<()>>,
    }

    impl WindowsInputGuard {
        /// Installs `WH_MOUSE_LL` and `WH_KEYBOARD_LL` hooks on a dedicated Win32 message-loop
        /// thread wired to `registry`.
        ///
        /// # Errors
        /// Returns [`CaptureError::CaptureFailed`] if Win32 hook registration fails.
        pub fn install(registry: ProtectedSurfaceRegistry) -> Result<Self, CaptureError> {
            let guard_id = NEXT_GUARD_ID.fetch_add(1, Ordering::Relaxed);
            {
                let mut list = registries().lock().map_err(|_| {
                    CaptureError::CaptureFailed("hook registry poisoned".to_owned())
                })?;
                list.push((guard_id, registry));
            }

            let stopped = Arc::new(AtomicBool::new(false));
            let stopped_worker = Arc::clone(&stopped);
            let (ready_tx, ready_rx) = mpsc::sync_channel::<Result<u32, String>>(1);

            let join_handle = thread::Builder::new()
                .name("oudos-win32-input-guard".to_owned())
                .spawn(move || {
                    run_hook_message_loop(&ready_tx, &stopped_worker);
                })
                .map_err(|e| {
                    remove_guard_registration(guard_id);
                    CaptureError::CaptureFailed(format!("failed to spawn hook thread: {e}"))
                })?;

            let thread_id = match ready_rx.recv() {
                Ok(Ok(tid)) => tid,
                Ok(Err(err)) => {
                    remove_guard_registration(guard_id);
                    let _ = join_handle.join();
                    return Err(CaptureError::CaptureFailed(err));
                }
                Err(e) => {
                    remove_guard_registration(guard_id);
                    let _ = join_handle.join();
                    return Err(CaptureError::CaptureFailed(format!(
                        "hook thread startup channel closed: {e}"
                    )));
                }
            };

            Ok(Self {
                guard_id,
                thread_id,
                stopped,
                join_handle: Some(join_handle),
            })
        }
    }

    impl Drop for WindowsInputGuard {
        fn drop(&mut self) {
            self.stopped.store(true, Ordering::SeqCst);
            // SAFETY: `self.thread_id` is a valid Win32 thread running `GetMessageW`.
            unsafe {
                let _ = PostThreadMessageW(self.thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
            }
            if let Some(handle) = self.join_handle.take() {
                let _ = handle.join();
            }
            remove_guard_registration(self.guard_id);
        }
    }

    fn remove_guard_registration(guard_id: u64) {
        if let Ok(mut list) = registries().lock() {
            list.retain(|(id, _)| *id != guard_id);
        }
    }

    struct HookHandleGuard(HHOOK);

    impl Drop for HookHandleGuard {
        fn drop(&mut self) {
            // SAFETY: `self.0` is a valid handle returned by `SetWindowsHookExW`.
            unsafe {
                let _ = UnhookWindowsHookEx(self.0);
            }
        }
    }

    fn run_hook_message_loop(
        ready_tx: &mpsc::SyncSender<Result<u32, String>>,
        stopped: &AtomicBool,
    ) {
        // SAFETY: `WH_MOUSE_LL` and `WH_KEYBOARD_LL` hooks run on this thread's message pump
        // and are guaranteed unhooked via RAII `HookHandleGuard` before thread exit.
        unsafe {
            let tid = GetCurrentThreadId();
            let mouse_hook = match SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_ll_proc), None, 0) {
                Ok(h) => HookHandleGuard(h),
                Err(e) => {
                    let _ =
                        ready_tx.send(Err(format!("SetWindowsHookExW(WH_MOUSE_LL) failed: {e}")));
                    return;
                }
            };
            let kbd_hook = match SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_ll_proc), None, 0)
            {
                Ok(h) => HookHandleGuard(h),
                Err(e) => {
                    drop(mouse_hook);
                    let _ = ready_tx.send(Err(format!(
                        "SetWindowsHookExW(WH_KEYBOARD_LL) failed: {e}"
                    )));
                    return;
                }
            };

            if ready_tx.send(Ok(tid)).is_err() {
                drop(kbd_hook);
                drop(mouse_hook);
                return;
            }

            let mut msg = MSG::default();
            while !stopped.load(Ordering::SeqCst) {
                let status = GetMessageW(&raw mut msg, None, 0, 0).0;
                if status <= 0 {
                    break;
                }
                let _ = TranslateMessage(&raw const msg);
                DispatchMessageW(&raw const msg);
            }

            drop(kbd_hook);
            drop(mouse_hook);
        }
    }

    fn snapshot_registries() -> Vec<ProtectedSurfaceRegistry> {
        registries()
            .lock()
            .map_or_default(|list| list.iter().map(|(_, reg)| reg.clone()).collect())
    }

    unsafe extern "system" fn mouse_ll_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if code == HC_ACTION_CODE && lparam.0 != 0 {
            // SAFETY: When `code == HC_ACTION`, Win32 passes a non-null pointer to `MSLLHOOKSTRUCT`.
            let info = unsafe { &*(lparam.0 as *const MSLLHOOKSTRUCT) };
            for reg in snapshot_registries() {
                if evaluate_mouse_ll_hook(&reg, info).is_some() {
                    return LRESULT(1);
                }
            }
        }
        // SAFETY: Forward permitted events down the Win32 hook chain.
        unsafe { CallNextHookEx(None, code, wparam, lparam) }
    }

    unsafe extern "system" fn keyboard_ll_proc(
        code: i32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        if code == HC_ACTION_CODE && lparam.0 != 0 {
            // SAFETY: When `code == HC_ACTION`, Win32 passes a non-null pointer to `KBDLLHOOKSTRUCT`.
            let info = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
            // SAFETY: `GetForegroundWindow` is a thread-safe read of current foreground HWND.
            let fg_raw = unsafe { GetForegroundWindow().0 as isize };
            for reg in snapshot_registries() {
                if evaluate_keyboard_ll_hook(&reg, info, fg_raw).is_some() {
                    return LRESULT(1);
                }
            }
        }
        // SAFETY: Forward permitted events down the Win32 hook chain.
        unsafe { CallNextHookEx(None, code, wparam, lparam) }
    }
}

#[cfg(target_os = "windows")]
pub use windows_hooks::{
    evaluate_keyboard_ll_hook, evaluate_mouse_ll_hook, query_window_rect, WindowsInputGuard,
};

#[cfg(test)]
mod tests {
    use super::*;
    use oud_protocol::audit::verify_chain;

    #[test]
    fn protected_rects_block_remote_coordinates_and_log_audit_entries() {
        let host = IdentityKeyPair::generate();
        let viewer = IdentityKeyPair::generate().public_key();
        let session_id = Uuid::now_v7();
        let now = Utc::now();

        let registry = ProtectedSurfaceRegistry::new();
        // 520x660 consent card at (400, 120) and persistent top-center banner at (500, 16)
        registry.set_surface(
            ProtectedSurfaceKind::ConsentPrompt,
            Rect {
                x: 400,
                y: 120,
                width: 520,
                height: 660,
            },
            None,
        );
        registry.set_surface(
            ProtectedSurfaceKind::SessionBanner,
            Rect {
                x: 500,
                y: 16,
                width: 460,
                height: 44,
            },
            None,
        );

        // Inside consent prompt (Allow button coordinates) -> blocked
        let err_consent = registry
            .enforce_remote_pointer(720, 720, now)
            .expect_err("pointer inside consent card must be blocked");
        assert!(matches!(
            err_consent,
            CaptureError::ProtectedSurfaceInputBlocked {
                surface: "consent_prompt",
                reason: "protected_rect",
            }
        ));

        // Inside persistent session banner (End session button coordinates) -> blocked
        let err_banner = registry
            .enforce_remote_pointer(900, 38, now)
            .expect_err("pointer inside banner must be blocked");
        assert!(matches!(
            err_banner,
            CaptureError::ProtectedSurfaceInputBlocked {
                surface: "session_banner",
                reason: "protected_rect",
            }
        ));

        // Outside both protected rects -> allowed
        assert!(registry.enforce_remote_pointer(120, 120, now).is_ok());
        assert_eq!(registry.total_blocked(), 2);

        let mut chain = AuditChain::new();
        let flushed = registry
            .flush_to_audit_chain(&mut chain, &host, session_id, viewer, "support.example.org")
            .expect("audit flush succeeds");
        assert_eq!(flushed, 2);
        assert_eq!(chain.entries().len(), 2);
        assert!(verify_chain(chain.entries(), &host.public_key()).is_ok());
        assert_eq!(
            chain.entries()[0].event.kind,
            AuditEventKind::SyntheticInputBlocked {
                surface: "consent_prompt".to_owned(),
                reason: "protected_rect".to_owned(),
            }
        );
        assert_eq!(
            chain.entries()[1].event.kind,
            AuditEventKind::SyntheticInputBlocked {
                surface: "session_banner".to_owned(),
                reason: "protected_rect".to_owned(),
            }
        );
    }

    #[test]
    fn injected_flag_helpers_detect_llmhf_and_llkhf_masks() {
        assert!(!is_synthetic_mouse_flags(0));
        assert!(is_synthetic_mouse_flags(0x0000_0001)); // LLMHF_INJECTED
        assert!(is_synthetic_mouse_flags(0x0000_0002)); // LLMHF_LOWER_IL_INJECTED
        assert!(!is_synthetic_keyboard_flags(0));
        assert!(is_synthetic_keyboard_flags(0x0000_0010)); // LLKHF_INJECTED
        assert!(is_synthetic_keyboard_flags(0x0000_0002)); // LLKHF_LOWER_IL_INJECTED
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_ll_hook_evaluators_reject_llmhf_and_llkhf_injected_events() {
        use windows::Win32::Foundation::POINT;
        use windows::Win32::UI::Input::KeyboardAndMouse::VK_RETURN;
        use windows::Win32::UI::WindowsAndMessaging::{
            KBDLLHOOKSTRUCT, KBDLLHOOKSTRUCT_FLAGS, LLKHF_INJECTED, LLMHF_INJECTED, MSLLHOOKSTRUCT,
        };

        let host = IdentityKeyPair::generate();
        let viewer = IdentityKeyPair::generate().public_key();
        let session_id = Uuid::now_v7();
        let registry = ProtectedSurfaceRegistry::new();
        registry.set_surface(
            ProtectedSurfaceKind::ConsentPrompt,
            Rect {
                x: 100,
                y: 100,
                width: 520,
                height: 660,
            },
            Some(0x1234),
        );

        let synthetic_ms = MSLLHOOKSTRUCT {
            pt: POINT { x: 500, y: 700 },
            mouseData: 0,
            flags: LLMHF_INJECTED,
            time: 0,
            dwExtraInfo: 0,
        };
        assert_eq!(
            evaluate_mouse_ll_hook(&registry, &synthetic_ms),
            Some(ProtectedSurfaceKind::ConsentPrompt)
        );
        assert_eq!(
            evaluate_mouse_ll_hook(
                &registry,
                &MSLLHOOKSTRUCT {
                    flags: 0,
                    ..synthetic_ms
                }
            ),
            None
        );

        let synthetic_kbd = KBDLLHOOKSTRUCT {
            vkCode: u32::from(VK_RETURN.0),
            scanCode: 0,
            flags: KBDLLHOOKSTRUCT_FLAGS(LLKHF_INJECTED.0),
            time: 0,
            dwExtraInfo: 0,
        };
        assert_eq!(
            evaluate_keyboard_ll_hook(&registry, &synthetic_kbd, 0x1234),
            Some(ProtectedSurfaceKind::ConsentPrompt)
        );

        let mut chain = AuditChain::new();
        let appended = registry
            .flush_to_audit_chain(&mut chain, &host, session_id, viewer, "support.example.org")
            .expect("audit flush");
        assert_eq!(appended, 2);
        assert!(verify_chain(chain.entries(), &host.public_key()).is_ok());
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_live_ll_hooks_block_sendinput_over_protected_rect() {
        use std::sync::Mutex as StdMutex;
        use std::time::Duration;
        use windows::Win32::UI::Input::KeyboardAndMouse::{
            SendInput, INPUT, INPUT_0, INPUT_MOUSE, MOUSEEVENTF_ABSOLUTE, MOUSEEVENTF_MOVE,
            MOUSEINPUT,
        };

        static SENDINPUT_SERIAL: StdMutex<()> = StdMutex::new(());
        let _serial = SENDINPUT_SERIAL.lock().expect("serial test lock");

        let registry = ProtectedSurfaceRegistry::new();
        // (0, 0) maps identically across DPI scales so protected 520x660 rect always contains abs(1,1).
        registry.set_surface(
            ProtectedSurfaceKind::ConsentPrompt,
            Rect {
                x: 0,
                y: 0,
                width: 520,
                height: 660,
            },
            None,
        );

        let _guard = WindowsInputGuard::install(registry.clone()).expect("install Win32 LL hooks");
        let inputs = [INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dx: 1,
                    dy: 1,
                    mouseData: 0,
                    dwFlags: MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        }];

        // On headless/locked Session 0 CI desktops SendInput may return 0; on interactive desktops
        // WH_MOUSE_LL catches LLMHF_INJECTED over the protected rect and blocks it immediately.
        let sent = unsafe {
            SendInput(
                &inputs,
                i32::try_from(std::mem::size_of::<INPUT>()).unwrap_or(40),
            )
        };
        std::thread::sleep(Duration::from_millis(60));
        if sent > 0 {
            assert!(
                registry.total_blocked() >= 1,
                "expected live SendInput inside protected rect to trigger WH_MOUSE_LL block"
            );
        }
    }
}
