//! Native window routing for Clipdeck's two long-lived windows.
//!
//! Clipdeck deliberately keeps **two** pre-created webview windows alive for the
//! whole session instead of mutating a single window between two very different
//! shapes:
//!
//! * [`QUICK_LABEL`] — a frameless, always-on-top, taskbar-less flyout that is
//!   re-centred on the active monitor on every invocation and light-dismisses on
//!   focus loss. It behaves like a shell surface, not an application.
//! * [`MAIN_LABEL`] — a normal decorated, resizable, taskbar-visible desktop
//!   application window that remembers its own position and size and never hides
//!   just because it lost focus.
//!
//! Switching one window between those two contracts at runtime (toggling
//! `decorations`, `skip_taskbar`, `resizable`, and the size on every hotkey
//! press) causes visible flashing, corrupts the remembered application
//! dimensions, and races the focus-lost handler. Two windows keep each contract
//! static, and both are warm so the hotkey is instant.

use tauri::{
    AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, PhysicalSize, WebviewWindow, Window,
};

use crate::platform::{monitor, source};
use crate::window_layout::{
    centered_in_work_area, centered_origin, fit_within, titlebar_is_reachable, PhysicalRect,
    QUICK_COMPACT_WIDTH, QUICK_EXPANDED_WIDTH, QUICK_HEIGHT, QUICK_WORK_AREA_MARGIN,
};
pub use crate::window_layout::{
    mode_for_label, WindowMode, MAIN_LABEL, QUICK_LABEL, SETTINGS_LABEL,
};
use crate::AppState;

pub fn quick(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window(QUICK_LABEL)
}

pub fn main_window(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window(MAIN_LABEL)
}

// ---- shared foreground capture -------------------------------------------

/// Remembers the application that was focused before Clipdeck took over.
///
/// Clipdeck's own windows are rejected: a second hotkey press while the palette
/// is already focused must not overwrite the real paste target with our webview.
pub fn capture_previous(app: &AppHandle) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    if let Some(hwnd) = source::foreground_paste_target() {
        *state.foreground.lock() = hwnd;
    }
}

fn clear_previous(app: &AppHandle) {
    if let Some(state) = app.try_state::<AppState>() {
        *state.foreground.lock() = 0;
    }
}

// ---- quick palette --------------------------------------------------------

/// True when the user pinned the quick palette, which suppresses light-dismiss.
pub fn quick_is_pinned(app: &AppHandle) -> bool {
    app.try_state::<AppState>()
        .map(|state| {
            state
                .quick_pinned
                .load(std::sync::atomic::Ordering::Relaxed)
        })
        .unwrap_or(false)
}

/// Records the pin state where the *native* focus-lost handler can read it.
///
/// Keeping this only in React state would not work: the light-dismiss decision
/// is made in Rust when the webview has already lost focus.
pub fn set_quick_pinned(app: &AppHandle, pinned: bool) {
    if let Some(state) = app.try_state::<AppState>() {
        state
            .quick_pinned
            .store(pinned, std::sync::atomic::Ordering::Relaxed);
    }
}

/// True when the quick palette is currently showing its preview column.
pub fn quick_is_expanded(app: &AppHandle) -> bool {
    app.try_state::<AppState>()
        .map(|state| state.settings.read().quick_preview_expanded)
        .unwrap_or(false)
}

/// Snapshot of the quick palette's native readiness state. Both the
/// `frontend_ready` and `data_hydrated` flags must be true before the window
/// is allowed to reveal itself.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuickReadinessState {
    pub frontend_ready: bool,
    pub data_hydrated: bool,
    pub open_pending: bool,
}

/// Shows the quick palette on the monitor the user is working on.
///
/// The previous foreground window is captured *before* the palette is shown so
/// paste can hand focus back afterwards.
pub fn show_quick(app: &AppHandle) {
    capture_previous(app);
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };

    // Store pending before checking ready so a concurrent frontend-ready signal
    // cannot fall between the two operations and lose the user's first hotkey.
    state
        .quick_open_pending
        .store(true, std::sync::atomic::Ordering::Release);
    if !state
        .quick_frontend_ready
        .load(std::sync::atomic::Ordering::Acquire)
    {
        log::info!("quick open queued while frontend loads");
        return;
    }
    if !state
        .quick_data_hydrated
        .load(std::sync::atomic::Ordering::Acquire)
    {
        log::info!("quick open queued while history hydrates");
        return;
    }
    if state
        .quick_open_pending
        .swap(false, std::sync::atomic::Ordering::AcqRel)
    {
        show_ready_quick(app);
    }
}

/// Marks one warm webview ready and fulfils a queued first-open request.
pub fn frontend_ready(app: &AppHandle, label: &str) {
    if label != QUICK_LABEL {
        log::info!("{label} frontend ready");
        return;
    }
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    state
        .quick_frontend_ready
        .store(true, std::sync::atomic::Ordering::Release);
    log::info!("quick frontend ready");
    if !state
        .quick_data_hydrated
        .load(std::sync::atomic::Ordering::Acquire)
    {
        // The first SQLite read may not have landed yet (bootStore is awaited
        // asynchronously from the React mount). The pending open will be
        // honoured when `signal_quick_data_hydrated` arrives.
        log::info!("quick frontend ready; waiting for data hydration");
        return;
    }
    if state
        .quick_open_pending
        .swap(false, std::sync::atomic::Ordering::AcqRel)
    {
        show_ready_quick(app);
    }
}

pub fn show_ready_quick(app: &AppHandle) {
    let Some(window) = quick(app) else {
        log::error!("quick palette window is missing");
        return;
    };
    let expanded = quick_is_expanded(app);
    let foreground = app
        .try_state::<AppState>()
        .map(|state| *state.foreground.lock())
        .unwrap_or(0);

    log::info!("showing ready quick window");
    layout_quick(&window, expanded, foreground);
    if let Some(state) = app.try_state::<AppState>() {
        let settings = state.settings.read().clone();
        let system = crate::platform::appearance::read();
        crate::native_appearance::apply_window(&window, &settings, &system);
    }
    // Applying Acrylic can recreate non-client styles on Windows. Reassert the
    // flyout contract after material setup and before reveal so the warm quick
    // window never acquires a normal caption or taskbar button.
    if let Err(error) = window.set_decorations(false) {
        log::warn!("could not remove quick-window decorations: {error}");
    }
    if let Err(error) = window.set_resizable(false) {
        log::warn!("could not lock quick-window resizing: {error}");
    }
    if let Err(error) = window.set_skip_taskbar(true) {
        log::warn!("could not remove quick window from taskbar: {error}");
    }
    #[cfg(windows)]
    let before = crate::platform::window_style::read_styles(&window);
    #[cfg(windows)]
    let enforced = crate::platform::window_style::enforce_quick_flyout(&window);
    #[cfg(windows)]
    if let Err(error) = &enforced {
        log::warn!("could not enforce quick-window Win32 styles: {error}");
    }
    let _ = window.set_always_on_top(true);
    let _ = window.show();
    #[cfg(windows)]
    if let Err(error) = crate::platform::window_style::enforce_quick_flyout(&window) {
        log::warn!("could not restore quick-window Win32 styles after show: {error}");
    }
    let _ = window.set_focus();
    #[cfg(windows)]
    if let Err(error) = crate::platform::window_style::enforce_quick_flyout(&window) {
        log::warn!("could not restore quick-window Win32 styles after focus: {error}");
    }
    #[cfg(windows)]
    report_quick_styles(&window, before, enforced);
    // Emitted only after readiness, positioning, material setup, show, and focus.
    let _ = app.emit_to(QUICK_LABEL, "clipdeck:quick-opened", ());
}

/// Records the native quick-window styles before enforcement, right after
/// enforcement, and after the window is shown and focused. Windows can restore
/// non-client chrome during `show`/`focus`, so all three observations are
/// needed to tell an ineffective enforcement apart from a later revert. The
/// payload is written only when the packaged smoke test asks for it.
#[cfg(windows)]
fn report_quick_styles(
    window: &WebviewWindow,
    before: Result<crate::platform::window_style::StyleSnapshot, String>,
    enforced: Result<crate::platform::window_style::StyleSnapshot, String>,
) {
    let shown = crate::platform::window_style::read_styles(window);
    let describe =
        |snapshot: &Result<crate::platform::window_style::StyleSnapshot, String>| match snapshot {
            Ok(value) => value.to_string(),
            Err(error) => format!("unavailable: {error}"),
        };
    log::info!(
        "quick window styles: before [{}], enforced [{}], shown [{}]",
        describe(&before),
        describe(&enforced),
        describe(&shown)
    );

    let Ok(base_path) = std::env::var("CLIPDECK_READY_FILE") else {
        return;
    };
    let field =
        |snapshot: &Result<crate::platform::window_style::StyleSnapshot, String>| match snapshot {
            Ok(value) => serde_json::json!({
                "hwnd": format!("0x{:X}", value.hwnd),
                "style": format!("0x{:08X}", value.style as u32),
                "exStyle": format!("0x{:08X}", value.ex_style as u32),
            }),
            Err(error) => serde_json::json!({ "error": error }),
        };
    let mut path = std::path::PathBuf::from(base_path);
    path.set_extension("quick-style.json");
    let payload = serde_json::json!({
        "before": field(&before),
        "enforced": field(&enforced),
        "shown": field(&shown),
        "processId": std::process::id(),
    });
    if let Err(error) = serde_json::to_vec(&payload)
        .map_err(|error| error.to_string())
        .and_then(|bytes| std::fs::write(&path, bytes).map_err(|error| error.to_string()))
    {
        log::warn!("could not write quick-window style diagnostics: {error}");
    }
}

pub fn hide_quick(app: &AppHandle) {
    if let Some(state) = app.try_state::<AppState>() {
        state
            .quick_open_pending
            .store(false, std::sync::atomic::Ordering::Release);
    }
    if let Some(window) = quick(app) {
        let _ = window.hide();
    }
}

pub fn toggle_quick(app: &AppHandle) {
    let pending = app
        .try_state::<AppState>()
        .map(|state| {
            state
                .quick_open_pending
                .load(std::sync::atomic::Ordering::Acquire)
        })
        .unwrap_or(false);
    let visible = quick(app)
        .and_then(|window| window.is_visible().ok())
        .unwrap_or(false);
    if visible || pending {
        hide_quick(app);
    } else {
        show_quick(app);
    }
}

/// Resizes the quick palette and re-centres it in the active monitor's work
/// area. Called on every invocation and whenever the preview column is toggled;
/// the quick window deliberately never restores a previously dragged position.
pub fn layout_quick(window: &WebviewWindow, expanded: bool, foreground: isize) {
    let scale = window.scale_factor().unwrap_or(1.0).max(0.1);
    let desired_width = if expanded {
        QUICK_EXPANDED_WIDTH
    } else {
        QUICK_COMPACT_WIDTH
    };

    let area = monitor::resolve(foreground).or_else(|| fallback_work_area(window));
    let Some(area) = area else {
        let _ = window.set_size(LogicalSize::new(desired_width, QUICK_HEIGHT));
        let _ = window.center();
        return;
    };

    let (width, height) = fit_within(
        desired_width * scale,
        QUICK_HEIGHT * scale,
        f64::from(area.width),
        f64::from(area.height),
        QUICK_WORK_AREA_MARGIN * scale,
    );
    let _ = window.set_size(PhysicalSize::new(
        width.round() as u32,
        height.round() as u32,
    ));

    let (x, y) = centered_in_work_area(
        PhysicalRect {
            x: i64::from(area.x),
            y: i64::from(area.y),
            width: i64::from(area.width),
            height: i64::from(area.height),
        },
        width,
        height,
    );
    let _ = window.set_position(PhysicalPosition::new(x, y));
}

/// Falls back to Tauri's monitor list when Win32 cannot report a work area.
fn fallback_work_area(window: &WebviewWindow) -> Option<monitor::WorkArea> {
    let target = window
        .current_monitor()
        .ok()
        .flatten()
        .or_else(|| window.primary_monitor().ok().flatten())?;
    Some(monitor::WorkArea {
        x: target.position().x,
        y: target.position().y,
        width: target.size().width as i32,
        height: target.size().height as i32,
    })
}

// ---- full application window ---------------------------------------------

/// Shows the normal desktop application window, restoring its remembered
/// position and size.
pub fn show_full(app: &AppHandle) {
    capture_previous(app);
    let Some(window) = main_window(app) else {
        log::error!("main application window is missing");
        return;
    };
    let _ = window.unminimize();
    ensure_titlebar_reachable(&window);
    let _ = window.show();
    let _ = window.set_focus();
}

pub fn hide_full(app: &AppHandle) {
    clear_previous(app);
    if let Some(window) = main_window(app) {
        let _ = window.hide();
    }
}

pub fn toggle_full(app: &AppHandle) {
    let visible = main_window(app)
        .and_then(|window| window.is_visible().ok())
        .unwrap_or(false);
    if visible {
        hide_full(app);
    } else {
        show_full(app);
    }
}

/// Hides whichever window issued a request, honouring that window's contract.
pub fn hide_self(window: &WebviewWindow) {
    let app = window.app_handle().clone();
    match mode_for_label(window.label()) {
        WindowMode::Quick => hide_quick(&app),
        WindowMode::Full => hide_full(&app),
        WindowMode::Settings => {
            let _ = window.hide();
        }
    }
}

// ---- native window events -------------------------------------------------

/// Applies the label-specific focus contract.
///
/// Only the quick palette light-dismisses, and only when it is not pinned. The
/// full application window must stay visible when the user clicks another app.
pub fn handle_focus_changed(window: &Window, focused: bool) {
    if focused || mode_for_label(window.label()) != WindowMode::Quick {
        return;
    }
    let app = window.app_handle().clone();
    if quick_is_pinned(&app) {
        return;
    }
    hide_quick(&app);
}

/// Applies the label-specific close contract. Every Clipdeck window hides
/// instead of being destroyed so the next open is warm.
pub fn handle_close_requested(window: &Window) {
    let app = window.app_handle().clone();
    match mode_for_label(window.label()) {
        WindowMode::Quick => hide_quick(&app),
        WindowMode::Full => hide_full(&app),
        WindowMode::Settings => {
            let _ = window.hide();
        }
    }
}

// ---- remembered position safety net (full application only) ---------------

/// Keeps the reused **application** window movable after a monitor is
/// disconnected or its resolution changes. Normal in-bounds positions are
/// deliberately preserved. This must never run for the quick palette, which has
/// no titlebar and is re-centred on every invocation instead.
fn ensure_titlebar_reachable(window: &WebviewWindow) {
    debug_assert_eq!(mode_for_label(window.label()), WindowMode::Full);
    let Ok(position) = window.outer_position() else {
        return;
    };
    let Ok(size) = window.outer_size() else {
        return;
    };
    let Ok(monitors) = window.available_monitors() else {
        return;
    };

    let window_rect = PhysicalRect {
        x: i64::from(position.x),
        y: i64::from(position.y),
        width: i64::from(size.width),
        height: i64::from(size.height),
    };
    let monitor_rects = monitors
        .iter()
        .map(|monitor| PhysicalRect {
            x: i64::from(monitor.position().x),
            y: i64::from(monitor.position().y),
            width: i64::from(monitor.size().width),
            height: i64::from(monitor.size().height),
        })
        .collect::<Vec<_>>();

    if titlebar_is_reachable(window_rect, &monitor_rects) {
        return;
    }

    let target = window
        .primary_monitor()
        .ok()
        .flatten()
        .or_else(|| monitors.first().cloned());
    let Some(target) = target else {
        return;
    };
    let target_rect = PhysicalRect {
        x: i64::from(target.position().x),
        y: i64::from(target.position().y),
        width: i64::from(target.size().width),
        height: i64::from(target.size().height),
    };
    let (x, y) = centered_origin(window_rect, target_rect);
    let _ = window.set_position(PhysicalPosition::new(x, y));
}
