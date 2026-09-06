//! System-tray icon and menu.
//!
//! Click actions:
//! * Left-click — toggle the popup (same as the global shortcut).
//! * Menu items — explicit Show / Open settings / Quit affordances.

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{App, Manager};

use crate::window;

pub fn install(app: &mut App) -> tauri::Result<()> {
    let quick = MenuItem::with_id(app, "quick", "Quick Clipboard", true, None::<&str>)?;
    let show = MenuItem::with_id(app, "show", "Show Clipmo", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "Settings…", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Clipmo", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;

    let menu = Menu::with_items(
        app,
        &[&quick, &show, &separator, &settings, &separator, &quit],
    )?;

    TrayIconBuilder::with_id("clipmo-tray")
        .tooltip("Clipmo — clipboard history")
        .icon(app.default_window_icon().cloned().unwrap_or_else(|| {
            tauri::image::Image::from_bytes(include_bytes!("../icons/tray.png"))
                .expect("tray icon must be valid PNG")
        }))
        .menu(&menu)
        .show_menu_on_left_click(cfg!(target_os = "macos"))
        .on_menu_event(|app, event| match event.id.as_ref() {
            "quick" => window::show_quick(app),
            "show" => window::show_full(app),
            "settings" => {
                if let Some(window) = app.get_webview_window("settings") {
                    let _ = window.set_title("Clipmo Settings");
                    let _ = window.unminimize();
                    let _ = window.show();
                    let _ = window.set_focus();
                } else if let Err(err) = super::commands::show_settings_window(app) {
                    log::error!("failed to open settings: {err}");
                } else if let Some(window) = app.get_webview_window("settings") {
                    let _ = window.set_title("Clipmo Settings");
                }
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button,
                button_state,
                ..
            } = event
            {
                if matches!(button, MouseButton::Left)
                    && matches!(button_state, MouseButtonState::Up)
                {
                    window::toggle_quick(tray.app_handle());
                }
            }
        })
        .build(app)?;

    Ok(())
}
