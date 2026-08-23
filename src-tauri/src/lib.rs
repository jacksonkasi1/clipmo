//! Clipdeck library crate.
//!
//! The `run()` function is the single entry point invoked from `main.rs`.
//! Everything else in the crate is reached through commands or events emitted
//! from inside `run`.

#[cfg(not(test))]
use std::path::PathBuf;
#[cfg(not(test))]
use std::sync::Arc;

#[cfg(not(test))]
use tauri::{Manager, WindowEvent};
#[cfg(not(test))]
use tauri_plugin_autostart::{MacosLauncher, ManagerExt as AutostartManagerExt};

pub mod capture_policy;
pub mod clipboard;
#[cfg(not(test))]
pub mod commands;
pub mod db;
pub mod error;
pub mod hotkey;
pub mod models;
pub mod native_appearance;
pub mod storage;
pub mod sync;
#[cfg(not(test))]
pub mod tray;
#[cfg(not(test))]
pub mod window;
pub mod window_layout;

mod win;

/// Shared application state handed to every command handler.
#[cfg(not(test))]
pub struct AppState {
    pub db: Arc<db::Db>,
    pub storage_root: Arc<parking_lot::RwLock<PathBuf>>,
    pub storage_operation: Arc<parking_lot::RwLock<()>>,
    pub settings: Arc<parking_lot::RwLock<models::Settings>>,
    pub sync: sync::SyncService,
    /// Both global accelerators, registered as one atomic pair.
    pub hotkeys: parking_lot::Mutex<RegisteredHotkeys>,
    /// HWND of the application focused before Clipdeck took over, used as the
    /// paste target. Never holds one of Clipdeck's own windows.
    pub foreground: parking_lot::Mutex<isize>,
    /// Whether the quick palette is pinned. Read by the native focus-lost
    /// handler, so it cannot live in React state.
    pub quick_pinned: std::sync::atomic::AtomicBool,
    /// Set only after the quick React shell, search field, and listeners exist.
    pub quick_frontend_ready: std::sync::atomic::AtomicBool,
    /// Set only after the quick webview's first SQLite read lands. The native
    /// show flow must gate the reveal on both `quick_frontend_ready` and this
    /// flag so the user never sees a Quick window that has not yet loaded its
    /// current history. A subsequent `clip-updated` is allowed to flip this off
    /// briefly so a re-fetch can re-arm the contract without leaving the
    /// window blank.
    pub quick_data_hydrated: std::sync::atomic::AtomicBool,
    /// Coalesces any number of startup hotkeys into one reveal after readiness.
    pub quick_open_pending: std::sync::atomic::AtomicBool,
}

/// The currently registered accelerators for the two global actions.
#[cfg(not(test))]
#[derive(Default)]
pub struct RegisteredHotkeys {
    pub quick: Option<tauri_plugin_global_shortcut::Shortcut>,
    pub full: Option<tauri_plugin_global_shortcut::Shortcut>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
#[cfg(not(test))]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            // `--show-quick` is also the deterministic installed-build smoke
            // command. It travels through the same readiness-gated native path
            // as the global shortcut and never creates another webview.
            if argv.iter().any(|argument| argument == "--show-quick") {
                window::show_quick(app);
            } else if argv.iter().any(|argument| argument == "--hide-quick") {
                window::hide_quick(app);
            } else {
                window::show_full(app);
            }
        }))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec!["--autostart"]),
        ))
        .invoke_handler(tauri::generate_handler![
            commands::list_items,
            commands::get_item,
            commands::flavors_for,
            commands::copy_to_clipboard,
            commands::paste_active,
            commands::set_favorite,
            commands::set_item_tags,
            commands::edit_item,
            commands::delete_item,
            commands::clear_history,
            commands::clear_category,
            commands::counts,
            commands::known_devices,
            commands::known_tags,
            commands::known_sources,
            commands::apply_filter_action,
            commands::load_settings,
            commands::save_settings,
            commands::set_launch_at_login,
            commands::set_ignored_apps,
            commands::change_storage_location,
            commands::prune_now,
            commands::list_installed_apps,
            commands::list_running_apps,
            commands::resolve_application_identity,
            commands::extract_application_icon,
            commands::appearance,
            commands::open_settings_window,
            commands::open_external_url,
            commands::open_storage_folder,
            commands::hide_window,
            commands::window_mode,
            commands::signal_frontend_ready,
            commands::signal_quick_data_hydrated,
            commands::quick_readiness_state,
            commands::signal_quick_search_focused,
            commands::show_quick_palette,
            commands::hide_quick_palette,
            commands::toggle_quick_palette,
            commands::show_full_application,
            commands::hide_full_application,
            commands::set_quick_pinned,
            commands::set_always_on_top,
            commands::set_preview_visible,
            commands::sync_state,
            commands::sync_history_now,
            sync::load_sync_preferences,
            sync::save_sync_preferences,
            commands::regenerate_pairing_code,
            commands::quit_app,
            native_appearance::sync_native_appearance,
        ])
        .setup(|app| {
            bootstrap(app)?;
            // A normal Start Menu/direct launch must display the application.
            // Autostart remains tray-only and does not interrupt sign-in.
            let arguments = std::env::args_os().collect::<Vec<_>>();
            if arguments.iter().any(|argument| argument == "--show-quick") {
                window::show_quick(app.handle());
            } else if !arguments.iter().any(|argument| argument == "--autostart") {
                window::show_full(app.handle());
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // Every handler is label-aware: only the quick palette
            // light-dismisses, and only the quick palette is re-centred.
            match event {
                WindowEvent::CloseRequested { api, .. } => {
                    // Hiding rather than destroying keeps both windows warm, so
                    // the next hotkey press is instant.
                    api.prevent_close();
                    window::handle_close_requested(window);
                }
                WindowEvent::Focused(focused) => {
                    window::handle_focus_changed(window, *focused);
                }
                WindowEvent::ScaleFactorChanged { .. } => {
                    native_appearance::handle_scale_factor_changed(window);
                }
                WindowEvent::ThemeChanged(theme) => {
                    native_appearance::handle_system_theme_changed(window, *theme);
                }
                _ => {}
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_, _| {});
}

/// Wires up everything that has to be alive before the first window appears:
/// the SQLite database, the asset directories, the clipboard listener, the
/// tray icon, and the global shortcut.
#[cfg(not(test))]
fn bootstrap(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("could not resolve app data dir: {e}"))?;
    let db_path = data_dir.join("clipdeck.db");
    let db = Arc::new(db::Db::open(&db_path)?);
    let mut loaded_settings = db.load_settings().unwrap_or_default();
    match app.autolaunch().is_enabled() {
        Ok(registered) => loaded_settings.launch_at_login = registered,
        Err(error) => log::warn!("launch-at-login state could not be read during startup: {error}"),
    }
    db.save_settings(&loaded_settings)?;
    let settings = Arc::new(parking_lot::RwLock::new(loaded_settings));
    let requested_root = settings
        .read()
        .storage_path
        .as_ref()
        .map(PathBuf::from)
        .unwrap_or_else(|| data_dir.clone());
    let storage_path = if storage::prepare_root(&requested_root).is_ok() {
        requested_root
    } else {
        storage::prepare_root(&data_dir)?;
        settings.write().storage_path = None;
        db.save_settings(&settings.read())?;
        data_dir.clone()
    };
    for managed_root in storage::managed_asset_roots(&storage_path) {
        app.asset_protocol_scope()
            .allow_directory(managed_root, true)?;
    }

    let sync = match sync::SyncService::start(
        app.handle().clone(),
        Arc::clone(&db),
        Arc::clone(&settings),
    ) {
        Ok(service) => service,
        Err(error) => {
            log::warn!("LAN sync could not start: {error}");
            sync::SyncService::inactive()
        }
    };
    let state = AppState {
        db: Arc::clone(&db),
        storage_root: Arc::new(parking_lot::RwLock::new(storage_path)),
        storage_operation: Arc::new(parking_lot::RwLock::new(())),
        settings: Arc::clone(&settings),
        sync,
        hotkeys: parking_lot::Mutex::new(RegisteredHotkeys::default()),
        foreground: parking_lot::Mutex::new(0),
        quick_pinned: std::sync::atomic::AtomicBool::new(false),
        quick_frontend_ready: std::sync::atomic::AtomicBool::new(false),
        quick_data_hydrated: std::sync::atomic::AtomicBool::new(false),
        quick_open_pending: std::sync::atomic::AtomicBool::new(false),
    };
    app.manage(state);

    // Both windows are declared hidden in tauri.conf.json and stay alive for
    // the whole session. Applying the material now — before either is ever
    // shown — means the user never sees a transparent or unstyled frame flash
    // in front of the Acrylic/Mica backdrop.
    {
        let app_state: tauri::State<AppState> = app.state();
        let settings = app_state.settings.read().clone();
        let system = crate::win::appearance::read();
        for label in [window::MAIN_LABEL, window::QUICK_LABEL] {
            let Some(window) = app.get_webview_window(label) else {
                log::error!("window '{label}' is missing from tauri.conf.json");
                continue;
            };
            let backdrop = native_appearance::apply_window(&window, &settings, &system);
            log::info!("applied {backdrop:?} backdrop to '{label}'");
        }
    }

    // Tray icon and global shortcuts are installed even on autostart.
    tray::install(app)?;
    commands::install_hotkeys(app);
    if let Err(error) = commands::enforce_history_policy_on_startup(app) {
        log::error!("startup history cleanup failed: {error}");
    }
    if let Err(error) = commands::install_clipboard_listener(app) {
        log::warn!("clipboard listener unavailable: {error}");
    }

    Ok(())
}
