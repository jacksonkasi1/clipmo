//! Keeps native window chrome aligned with the persisted appearance setting.
//!
//! Webview colors are handled by the frontend. This module owns the matching
//! Tauri window theme, DWM frame, and Windows backdrop so native decorations do
//! not drift from the selected theme.

use crate::models::{Backdrop, ThemeMode};
use crate::window_layout::WindowMode;

/// Resolves the persisted preference against the current operating-system
/// theme. Explicit choices never inherit a later system change.
pub fn resolve_dark(mode: ThemeMode, system_dark: bool) -> bool {
    match mode {
        ThemeMode::System => system_dark,
        ThemeMode::Light => false,
        ThemeMode::Dark => true,
    }
}

#[cfg(not(test))]
use tauri::{AppHandle, Emitter, Manager, Theme, WebviewWindow, Window};

#[cfg(not(test))]
use crate::models::{Settings, SystemAppearance};
#[cfg(not(test))]
use crate::AppState;

/// Re-applies native appearance to every open window and returns the current
/// operating-system appearance for the webview store.
#[cfg(not(test))]
#[tauri::command]
pub fn sync_native_appearance(
    app: AppHandle,
    state: tauri::State<'_, AppState>,
) -> SystemAppearance {
    let settings = state.settings.read().clone();
    apply_all(&app, &settings)
}

/// Chooses the Fluent material for a window from its behavioural contract.
///
/// Material is a property of *what the window is*, not of a single global
/// preference:
///
/// * the quick palette is a transient shell flyout, so it uses Desktop Acrylic
///   for a strong relationship with whatever is behind it;
/// * the full application and the settings window are long-lived surfaces, so
///   they use Mica, which is the Windows 11 foundation material for app windows
///   and is far cheaper to composite.
///
/// An explicit `Solid` preference always wins, and Acrylic degrades to Mica and
/// then to a solid Fluent surface when the compositor refuses it (transparency
/// effects off, high contrast, remote sessions).
pub fn material_for(mode: WindowMode, preference: Backdrop) -> Backdrop {
    match preference {
        Backdrop::Solid => Backdrop::Solid,
        _ => match mode {
            WindowMode::Quick => Backdrop::Acrylic,
            WindowMode::Full | WindowMode::Settings => Backdrop::Mica,
        },
    }
}

/// Applies the persisted appearance to one native window.
#[cfg(not(test))]
pub fn apply_window(
    window: &WebviewWindow,
    settings: &Settings,
    system: &SystemAppearance,
) -> Backdrop {
    if let Err(error) = window.set_theme(theme_override(settings.theme)) {
        log::warn!("could not update native window theme: {error}");
    }

    apply_surface(window, settings, resolve_dark(settings.theme, system.dark))
}

/// Applies the persisted appearance to every native window.
#[cfg(not(test))]
pub fn apply_all(app: &AppHandle, settings: &Settings) -> SystemAppearance {
    let system = crate::platform::appearance::read();
    for window in app.webview_windows().values() {
        apply_window(window, settings, &system);
    }
    let _ = app.emit("appearance-changed", &system);
    system
}

/// Reapplies the DWM frame after a per-monitor DPI transition.
#[cfg(not(test))]
pub fn handle_scale_factor_changed(window: &Window) {
    let app = window.app_handle();
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let settings = state.settings.read().clone();
    let system = crate::platform::appearance::read();
    if let Some(webview) = app.get_webview_window(window.label()) {
        apply_window(&webview, &settings, &system);
    }
}

/// Handles an operating-system theme notification. Only System mode follows
/// the notification; explicit Light and Dark preferences remain fixed.
#[cfg(not(test))]
pub fn handle_system_theme_changed(window: &Window, theme: Theme) {
    let app = window.app_handle();
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let settings = state.settings.read().clone();
    if settings.theme != ThemeMode::System {
        return;
    }

    let mut system = crate::platform::appearance::read();
    system.dark = matches!(theme, Theme::Dark);
    for webview in app.webview_windows().values() {
        apply_surface(webview, &settings, system.dark);
    }
    let _ = app.emit("appearance-changed", &system);
}

#[cfg(not(test))]
fn apply_surface(window: &WebviewWindow, settings: &Settings, dark: bool) -> Backdrop {
    let mode = crate::window::mode_for_label(window.label());
    let requested = material_for(mode, settings.backdrop);
    let effective = crate::platform::backdrop::apply(window, requested, dark);
    // Acrylic/theme/DPI changes can recreate the Win32 non-client frame. The
    // quick flyout's native contract must be the final operation, otherwise a
    // white DWM edge or taskbar application style can return after the initial
    // warm-up enforcement in `show_ready_quick`.
    #[cfg(windows)]
    if mode == WindowMode::Quick {
        if let Err(error) = crate::platform::window_style::enforce_quick_flyout(window) {
            log::warn!("could not restore quick-window contract after backdrop: {error}");
        }
    }
    // macOS emits after its main-thread AppKit operation actually completes.
    #[cfg(not(target_os = "macos"))]
    let _ = window.emit("clipdeck:backdrop", effective);
    effective
}

#[cfg(not(test))]
fn theme_override(mode: ThemeMode) -> Option<Theme> {
    match mode {
        ThemeMode::System => None,
        ThemeMode::Light => Some(Theme::Light),
        ThemeMode::Dark => Some(Theme::Dark),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_mode_follows_the_operating_system() {
        assert!(resolve_dark(ThemeMode::System, true));
        assert!(!resolve_dark(ThemeMode::System, false));
    }

    #[test]
    fn explicit_dark_ignores_the_operating_system() {
        assert!(resolve_dark(ThemeMode::Dark, true));
        assert!(resolve_dark(ThemeMode::Dark, false));
    }

    #[test]
    fn explicit_light_ignores_the_operating_system() {
        assert!(!resolve_dark(ThemeMode::Light, true));
        assert!(!resolve_dark(ThemeMode::Light, false));
    }

    #[test]
    fn the_quick_flyout_uses_desktop_acrylic_and_app_windows_use_mica() {
        assert_eq!(
            material_for(WindowMode::Quick, Backdrop::Acrylic),
            Backdrop::Acrylic
        );
        assert_eq!(
            material_for(WindowMode::Full, Backdrop::Acrylic),
            Backdrop::Mica
        );
        // The settings window is long lived; it must not inherit Acrylic just
        // because the transient palette uses it.
        assert_eq!(
            material_for(WindowMode::Settings, Backdrop::Acrylic),
            Backdrop::Mica
        );
    }

    #[test]
    fn an_explicit_solid_preference_disables_every_material() {
        for mode in [WindowMode::Quick, WindowMode::Full, WindowMode::Settings] {
            assert_eq!(material_for(mode, Backdrop::Solid), Backdrop::Solid);
        }
    }
}
