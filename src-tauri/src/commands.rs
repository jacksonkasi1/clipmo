//! Tauri command handlers and one-shot helper functions.
//!
//! Commands return `Result<T, Error>` so the frontend always receives a
//! uniform error shape. Helper functions (no `#[tauri::command]`) live here
//! too when they need to be called from `lib.rs` or `tray.rs`.

use std::path::PathBuf;
use std::sync::{mpsc, Arc};

use tauri::{App, AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_autostart::ManagerExt as AutostartManagerExt;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
use tauri_plugin_opener::OpenerExt;

use crate::clipboard::listener::{self, CaptureSink, ClipEvent};
use crate::db::Db;
use crate::error::{Error, Result};
use crate::models::{
    BulkFilterAction, ClipItem, CollectionSummary, Counts, DeviceIdentity, FilterScope, IgnoredApp,
    ImageCompression, ImageFormat, ImageMeta, ItemKind, ListQuery, PasteFlavor, Settings,
    SourceApp, StoredFile, StoredFileStatus, SyncState, SystemAppearance,
};
use crate::win::paste;
use crate::AppState;

// ---- command handlers ----------------------------------------------------

#[tauri::command]
pub async fn list_items(
    state: tauri::State<'_, AppState>,
    query: ListQuery,
) -> Result<Vec<ClipItem>> {
    state.db.list(&query).map_err(|error| {
        log::error!("list_items failed: {error}");
        error
    })
}

#[tauri::command]
pub async fn get_item(state: tauri::State<'_, AppState>, id: i64) -> Result<ClipItem> {
    state.db.get(id)?.ok_or(Error::NotFound("clipboard item"))
}

#[tauri::command]
pub async fn flavors_for(state: tauri::State<'_, AppState>, id: i64) -> Result<FlavorBundle> {
    let item = state.db.get_required(id)?;
    let (_, html, rtf) = state
        .db
        .flavors(id)?
        .ok_or(Error::NotFound("clipboard item"))?;
    Ok(FlavorBundle {
        text: if item.kind == ItemKind::Text
            || item.kind == ItemKind::Link
            || item.kind == ItemKind::Email
            || item.kind == ItemKind::Color
        {
            Some(item.content.clone())
        } else {
            None
        },
        html,
        rtf,
        files: item.files.clone(),
        image: item.image.clone(),
    })
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FlavorBundle {
    pub text: Option<String>,
    pub html: Option<String>,
    pub rtf: Option<String>,
    pub files: Vec<String>,
    pub image: Option<ImageMeta>,
}

#[tauri::command]
pub async fn copy_to_clipboard(
    app: AppHandle,
    state: tauri::State<'_, AppState>,
    id: i64,
    flavor: PasteFlavor,
) -> Result<()> {
    let item = state.db.get_required(id)?;
    let (_, html, rtf) = state
        .db
        .flavors(id)?
        .ok_or(Error::NotFound("clipboard item"))?;
    state.db.touch(id)?;
    crate::clipboard::writer::put_back_on_clipboard(
        &item,
        flavor,
        html.as_deref(),
        rtf.as_deref(),
    )?;
    // Tasting bumps `last_copied_at`, which reorders the row. The other window
    // is not told by the local React state because the user action happens in
    // just one webview, so broadcast the refresh signal.
    let _ = app.emit("clip-updated", ());
    Ok(())
}

#[tauri::command]
pub async fn paste_active(
    app: AppHandle,
    window: tauri::WebviewWindow,
    state: tauri::State<'_, AppState>,
    id: i64,
    flavor: PasteFlavor,
) -> Result<()> {
    let item = state.db.get_required(id)?;
    let (_, html, rtf) = state
        .db
        .flavors(id)?
        .ok_or(Error::NotFound("clipboard item"))?;
    state.db.touch(id)?;

    crate::clipboard::writer::put_back_on_clipboard(
        &item,
        flavor,
        html.as_deref(),
        rtf.as_deref(),
    )?;

    // A paste touches the row so it floats to the top in both windows. The
    // user only sees the paste source window update optimistically, so emit
    // the broadcast so the partner webview can re-fetch the new ordering.
    let _ = app.emit("clip-updated", ());

    // Hide the window the paste came from *before* restoring focus. Hiding
    // `main` unconditionally would dismiss the full application whenever the
    // user pasted from the quick palette, and vice versa.
    let target = *state.foreground.lock();
    crate::window::hide_self(&window);
    if !paste::paste_to(target) {
        return Err(Error::Other(
            "the previous application could not receive the paste command".into(),
        ));
    }
    Ok(())
}

#[tauri::command]
pub async fn copy_multiple_to_clipboard(
    app: AppHandle,
    state: tauri::State<'_, AppState>,
    ids: Vec<i64>,
    flavor: PasteFlavor,
) -> Result<()> {
    if ids.is_empty() {
        return Ok(());
    }
    let mut items = Vec::new();
    for id in &ids {
        if let Ok(item) = state.db.get_required(*id) {
            let _ = state.db.touch(*id);
            items.push(item);
        }
    }
    if items.is_empty() {
        return Err(Error::NotFound("clipboard items"));
    }
    crate::clipboard::writer::put_multiple_back_on_clipboard(&items, flavor)?;
    let _ = app.emit("clip-updated", ());
    Ok(())
}

#[tauri::command]
pub async fn paste_multiple_active(
    app: AppHandle,
    window: tauri::WebviewWindow,
    state: tauri::State<'_, AppState>,
    ids: Vec<i64>,
    flavor: PasteFlavor,
) -> Result<()> {
    if ids.is_empty() {
        return Ok(());
    }
    let mut items = Vec::new();
    for id in &ids {
        if let Ok(item) = state.db.get_required(*id) {
            let _ = state.db.touch(*id);
            items.push(item);
        }
    }
    if items.is_empty() {
        return Err(Error::NotFound("clipboard items"));
    }
    crate::clipboard::writer::put_multiple_back_on_clipboard(&items, flavor)?;
    let _ = app.emit("clip-updated", ());

    let target = *state.foreground.lock();
    crate::window::hide_self(&window);
    if !paste::paste_to(target) {
        return Err(Error::Other(
            "the previous application could not receive the paste command".into(),
        ));
    }
    Ok(())
}

#[tauri::command]
pub async fn set_favorite(
    app: AppHandle,
    state: tauri::State<'_, AppState>,
    id: i64,
    value: bool,
) -> Result<()> {
    state.db.set_favorite(id, value)?;
    // A favourite toggle in one window must also resync the other: the other
    // window's store may be hidden and would otherwise hold the old pin state
    // until its own `clipdeck:quick-opened` rescue. Emit with the updated row.
    let _ = app.emit("clip-updated", ());
    Ok(())
}

#[tauri::command]
pub async fn set_item_tags(
    app: AppHandle,
    state: tauri::State<'_, AppState>,
    id: i64,
    tags: Vec<String>,
) -> Result<ClipItem> {
    let item = state.db.set_tags(id, &tags)?;
    let _ = app.emit("clip-updated", &item);
    Ok(item)
}

#[tauri::command]
pub async fn edit_item(
    app: AppHandle,
    state: tauri::State<'_, AppState>,
    id: i64,
    content: String,
) -> Result<ClipItem> {
    if content.trim().is_empty() {
        return Err(Error::Other("clipboard content cannot be empty".into()));
    }
    let kind = crate::clipboard::classify(content.trim());
    let hash = crate::clipboard::hash_text(&content);
    let item = state.db.update_text_content(id, &content, kind, &hash)?;
    let _ = app.emit("clip-updated", &item);
    Ok(item)
}

#[tauri::command]
pub async fn delete_item(app: AppHandle, state: tauri::State<'_, AppState>, id: i64) -> Result<()> {
    let _storage_guard = state.storage_operation.read();
    let orphans = state.db.delete(id)?;
    cleanup_asset_paths(&state.storage_root.read(), orphans);
    // Both the quick and full windows must drop the row immediately. The
    // window that did not perform the delete does not run its own `refresh()`
    // after the IPC call, so the broadcast is the only way to keep the two
    // stores in sync without waiting for a hotkey reopen.
    let _ = app.emit("clip-updated", ());
    Ok(())
}

#[tauri::command]
pub async fn clear_history(
    app: AppHandle,
    state: tauri::State<'_, AppState>,
    include_favorites: bool,
) -> Result<()> {
    let _storage_guard = state.storage_operation.read();
    let orphans = state.db.clear(include_favorites)?;
    cleanup_asset_paths(&state.storage_root.read(), orphans);
    let _ = app.emit("clip-updated", ());
    Ok(())
}

#[tauri::command]
pub async fn clear_category(
    app: AppHandle,
    state: tauri::State<'_, AppState>,
    kind: ItemKind,
    include_favorites: bool,
) -> Result<()> {
    let _storage_guard = state.storage_operation.read();
    let orphans = state.db.clear_kind(kind, include_favorites)?;
    cleanup_asset_paths(&state.storage_root.read(), orphans);
    let _ = app.emit("clip-updated", ());
    Ok(())
}

#[tauri::command]
pub async fn counts(state: tauri::State<'_, AppState>) -> Result<Counts> {
    state.db.counts()
}

#[tauri::command]
pub async fn known_devices(state: tauri::State<'_, AppState>) -> Result<Vec<DeviceIdentity>> {
    let mut devices = state.db.known_devices()?;
    let local = state.settings.read().device_identity();
    if let Some(stored_local) = devices.iter_mut().find(|device| device.id == "local") {
        stored_local.name = local.name;
        stored_local.platform = local.platform;
        stored_local.color = local.color;
    }
    for peer in state.sync.state(&state.settings.read()).peers {
        if !devices.iter().any(|device| device.id == peer.device.id) {
            devices.push(peer.device);
        }
    }
    Ok(devices)
}

#[tauri::command]
pub async fn known_tags(state: tauri::State<'_, AppState>) -> Result<Vec<String>> {
    state.db.known_tags()
}

#[tauri::command]
pub async fn list_collections(state: tauri::State<'_, AppState>) -> Result<Vec<CollectionSummary>> {
    state.db.collections()
}

#[tauri::command]
pub async fn create_collection(
    app: AppHandle,
    state: tauri::State<'_, AppState>,
    name: String,
) -> Result<()> {
    state.db.create_collection(&name)?;
    let _ = app.emit("clip-updated", ());
    Ok(())
}

#[tauri::command]
pub async fn delete_collection(
    app: AppHandle,
    state: tauri::State<'_, AppState>,
    name: String,
) -> Result<()> {
    state.db.delete_collection(&name)?;
    let _ = app.emit("clip-updated", ());
    Ok(())
}

#[tauri::command]
pub async fn known_sources(state: tauri::State<'_, AppState>) -> Result<Vec<SourceApp>> {
    let mut sources = state.db.known_sources()?;
    for source in &mut sources {
        if source
            .icon_path
            .as_ref()
            .is_none_or(|path| !std::path::Path::new(path).exists())
        {
            source.icon_path = crate::win::icon::cached(&source.exe_path);
        }
    }
    Ok(sources)
}

#[tauri::command]
pub async fn apply_filter_action(
    app: AppHandle,
    state: tauri::State<'_, AppState>,
    scope: FilterScope,
    action: BulkFilterAction,
) -> Result<()> {
    let _storage_guard = state.storage_operation.read();
    let orphans = state.db.apply_filter_action(&scope, action)?;
    cleanup_asset_paths(&state.storage_root.read(), orphans);
    let _ = app.emit("clip-updated", ());
    Ok(())
}

#[tauri::command]
pub async fn load_settings(app: AppHandle, state: tauri::State<'_, AppState>) -> Result<Settings> {
    let registered = autostart_enabled(&app)?;
    let mut settings = state.settings.read().clone();
    if settings.launch_at_login != registered {
        settings.launch_at_login = registered;
        state.db.save_settings(&settings)?;
        *state.settings.write() = settings.clone();
    }
    Ok(settings)
}

#[tauri::command]
pub async fn set_launch_at_login(
    app: AppHandle,
    state: tauri::State<'_, AppState>,
    enabled: bool,
) -> Result<Settings> {
    let previous = autostart_enabled(&app)?;
    if enabled != previous {
        set_autostart_enabled(&app, enabled)?;
    }

    let mut settings = state.settings.read().clone();
    settings.launch_at_login = enabled;
    if let Err(error) = state.db.save_settings(&settings) {
        if enabled != previous {
            let _ = set_autostart_enabled(&app, previous);
        }
        return Err(error);
    }

    *state.settings.write() = settings.clone();
    let _ = app.emit("settings-updated", &settings);
    Ok(settings)
}

#[tauri::command]
pub async fn set_ignored_apps(
    app: AppHandle,
    state: tauri::State<'_, AppState>,
    ignored_apps: Vec<IgnoredApp>,
) -> Result<Settings> {
    let mut settings = state.settings.read().clone();
    settings.ignored_apps = crate::capture_policy::normalize_ignored_apps(&ignored_apps);
    state.db.save_settings(&settings)?;
    *state.settings.write() = settings.clone();
    let _ = app.emit("settings-updated", &settings);
    Ok(settings)
}

#[tauri::command]
pub async fn save_settings(
    app: AppHandle,
    state: tauri::State<'_, AppState>,
    mut settings: Settings,
) -> Result<Settings> {
    // Storage changes use the verified migration command; never accept a raw
    // path mutation through the general settings form.
    let previous = state.settings.read().clone();
    let previous_autostart = autostart_enabled(&app)?;
    settings.storage_path = previous.storage_path.clone();
    settings.file_include_extensions =
        crate::capture_policy::normalize_extensions(&settings.file_include_extensions);
    settings.file_exclude_extensions =
        crate::capture_policy::normalize_extensions(&settings.file_exclude_extensions);
    settings.ignored_apps = crate::capture_policy::normalize_ignored_apps(&settings.ignored_apps);
    settings.image_quality = settings.image_quality.clamp(1, 100);
    validate_filter_shortcuts(&settings)?;
    // Both accelerators are validated together so Settings can surface a clear
    // "same shortcut" error instead of one action silently stealing the other.
    let hotkeys_changed = settings.hotkey != previous.hotkey
        || settings.full_window_hotkey != previous.full_window_hotkey;
    if settings.launch_at_login != previous_autostart {
        set_autostart_enabled(&app, settings.launch_at_login)?;
    }
    if hotkeys_changed {
        if let Err(error) = register_hotkeys(&app, &settings.hotkey, &settings.full_window_hotkey) {
            if settings.launch_at_login != previous_autostart {
                let _ = set_autostart_enabled(&app, previous_autostart);
            }
            return Err(error);
        }
    }
    if let Err(error) = state.db.save_settings(&settings) {
        if hotkeys_changed {
            if let Err(rollback_error) =
                register_hotkeys(&app, &previous.hotkey, &previous.full_window_hotkey)
            {
                log::error!("could not restore the previous hotkeys: {rollback_error}");
            }
        }
        if settings.launch_at_login != previous_autostart {
            if let Err(rollback_error) = set_autostart_enabled(&app, previous_autostart) {
                log::error!(
                    "could not restore the previous launch-at-login state: {rollback_error}"
                );
            }
        }
        return Err(error);
    }
    {
        let mut current = state.settings.write();
        *current = settings.clone();
    }
    let sync_settings_changed = state.sync.settings_changed(&previous, &settings);
    apply_runtime_settings(&app, &settings)?;
    enforce_history_policy(&state)?;
    let _ = app.emit("settings-updated", &settings);
    if sync_settings_changed {
        let _ = app.emit("sync-peers-updated", ());
    }
    Ok(settings)
}

#[tauri::command]
pub async fn change_storage_location(
    app: AppHandle,
    state: tauri::State<'_, AppState>,
    path: String,
) -> Result<Settings> {
    let _storage_guard = state.storage_operation.write();
    let target = PathBuf::from(path.trim());
    if !target.is_absolute() {
        return Err(Error::Other(
            "storage location must be an absolute path".into(),
        ));
    }
    let old_root = state.storage_root.read().clone();
    if target == old_root {
        return Ok(state.settings.read().clone());
    }
    if crate::storage::paths_overlap(&target, &old_root)? {
        return Err(Error::Other(
            "choose a storage folder outside the current storage tree".into(),
        ));
    }

    // Reject occupied targets before the asset scope is granted or any files
    // are copied. A successful validation makes later rollback safe.
    crate::storage::validate_empty_migration_target(&target)?;

    crate::storage::copy_managed_storage(&old_root, &target)?;
    if let Err(error) = allow_storage_target_scope(&app, &target) {
        rollback_storage_target(&app, &target);
        return Err(error);
    }
    let mut next = state.settings.read().clone();
    next.storage_path = Some(target.to_string_lossy().into_owned());
    if let Err(error) = state.db.migrate_storage(&old_root, &target, &next) {
        rollback_storage_target(&app, &target);
        return Err(error);
    }
    *state.storage_root.write() = target;
    *state.settings.write() = next.clone();
    revoke_storage_target_scope(&app, &old_root);
    if let Err(error) = crate::storage::remove_managed_directories(&old_root) {
        log::warn!("old managed storage cleanup was skipped: {error}");
    }
    let _ = app.emit("settings-updated", &next);
    let _ = app.emit("clip-storage-migrated", ());
    Ok(next)
}

#[tauri::command]
pub async fn prune_now(state: tauri::State<'_, AppState>) -> Result<()> {
    enforce_history_policy(&state)
}

#[tauri::command]
pub async fn list_installed_apps(
    refresh: Option<bool>,
) -> Result<Vec<crate::models::ApplicationInfo>> {
    Ok(crate::win::apps::installed(refresh.unwrap_or(false)))
}

#[tauri::command]
pub async fn list_running_apps() -> Result<Vec<crate::models::ApplicationInfo>> {
    Ok(crate::win::apps::running())
}

#[tauri::command]
pub async fn resolve_application_identity(
    executable_path: String,
) -> Result<crate::models::IgnoredApp> {
    crate::win::apps::resolve(&executable_path)
        .ok_or_else(|| Error::Other("executable path is empty".into()))
}

#[tauri::command]
pub async fn extract_application_icon(executable_path: String) -> Result<Option<String>> {
    Ok(crate::win::icon::extract(&executable_path))
}

#[tauri::command]
pub async fn appearance() -> Result<SystemAppearance> {
    Ok(crate::win::appearance::read())
}

#[tauri::command]
pub async fn open_settings_window(app: AppHandle) -> Result<()> {
    show_settings_window(&app).map_err(Error::Other)
}

#[tauri::command]
pub async fn open_external_url(app: AppHandle, url: String) -> Result<()> {
    let parsed = url::Url::parse(&url).map_err(|_| Error::Other("invalid URL".into()))?;
    if !matches!(parsed.scheme(), "http" | "https" | "mailto") {
        return Err(Error::Other("unsupported URL scheme".into()));
    }
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|error| Error::Other(error.to_string()))
}

#[tauri::command]
pub async fn open_storage_folder(app: AppHandle, state: tauri::State<'_, AppState>) -> Result<()> {
    let path = state.storage_root.read().to_string_lossy().into_owned();
    app.opener()
        .open_path(path, None::<&str>)
        .map_err(|error| Error::Other(error.to_string()))
}

#[tauri::command]
pub async fn hide_window(window: tauri::WebviewWindow) -> Result<()> {
    crate::window::hide_self(&window);
    Ok(())
}

/// Reports which behavioural contract the calling window implements.
///
/// The frontend uses this rather than guessing from the viewport width, which
/// would misclassify a narrow full application window as the quick palette.
#[tauri::command]
pub async fn window_mode(window: tauri::WebviewWindow) -> Result<crate::window::WindowMode> {
    Ok(crate::window::mode_for_label(window.label()))
}

/// Per-window handshake emitted only after React, search/layout, and listeners
/// are ready. Quick open requests are fulfilled before the optional smoke-test
/// payload is written, so native visibility can never outrun the web surface.
#[tauri::command]
pub async fn signal_frontend_ready(
    window: tauri::WebviewWindow,
    search_visible: bool,
    layout_visible: bool,
) -> Result<()> {
    if !search_visible || !layout_visible {
        return Err(Error::Other(
            "frontend readiness requires a visible search field and layout".into(),
        ));
    }
    crate::window::frontend_ready(window.app_handle(), window.label());

    let Ok(base_path) = std::env::var("CLIPDECK_READY_FILE") else {
        return Ok(());
    };
    let mut path = std::path::PathBuf::from(base_path);
    if window.label() != crate::window::MAIN_LABEL {
        let extension = format!("{}.json", window.label());
        path.set_extension(extension);
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let payload = serde_json::json!({
        "frontendReady": true,
        "windowCreated": true,
        "windowVisible": window.is_visible()?,
        "windowLabel": window.label(),
        "searchVisible": search_visible,
        "layoutVisible": layout_visible,
        "processId": std::process::id(),
    });
    let temporary = path.with_extension("tmp");
    std::fs::write(
        &temporary,
        serde_json::to_vec(&payload).map_err(|error| Error::Other(error.to_string()))?,
    )?;
    std::fs::rename(temporary, path)?;
    Ok(())
}

#[tauri::command]
pub async fn signal_quick_search_focused(window: tauri::WebviewWindow) -> Result<()> {
    if window.label() != crate::window::QUICK_LABEL {
        return Ok(());
    }
    log::info!("quick search focus confirmed");
    let Ok(base_path) = std::env::var("CLIPDECK_READY_FILE") else {
        return Ok(());
    };
    let mut path = std::path::PathBuf::from(base_path);
    path.set_extension("quick-focus.json");
    let payload = serde_json::json!({
        "searchFocused": true,
        "windowLabel": window.label(),
        "processId": std::process::id(),
    });
    std::fs::write(
        path,
        serde_json::to_vec(&payload).map_err(|error| Error::Other(error.to_string()))?,
    )?;
    Ok(())
}

/// Records that the quick webview's first SQLite read has landed. The native
/// `show_quick` flow refuses to reveal the window until both this and the
/// frontend-readiness flag are set, so a Quick View that opens for the first
/// time cannot present a stale, partial or empty list. The readiness state is
/// also exposed via `quick_readiness_state` so the frontend can render an
/// explicit "Loading clipboard history…" surface while it waits, instead of
/// pretending the list is complete.
#[tauri::command]
pub async fn signal_quick_data_hydrated(
    app: AppHandle,
    window: tauri::WebviewWindow,
    hydrated: bool,
) -> Result<()> {
    if window.label() != crate::window::QUICK_LABEL {
        return Ok(());
    }
    let Some(state) = app.try_state::<AppState>() else {
        return Ok(());
    };
    state
        .quick_data_hydrated
        .store(hydrated, std::sync::atomic::Ordering::Release);
    log::info!("quick data hydrated = {hydrated}");
    if hydrated
        && state
            .quick_frontend_ready
            .load(std::sync::atomic::Ordering::Acquire)
        && state
            .quick_open_pending
            .swap(false, std::sync::atomic::Ordering::AcqRel)
    {
        crate::window::show_ready_quick(&app);
    }
    Ok(())
}

/// Inspectable readiness snapshot. The frontend uses it to decide whether
/// its loading chrome should still be on screen and to log any
/// mismatch between the React and native views of the world.
#[tauri::command]
pub async fn quick_readiness_state(app: AppHandle) -> Result<crate::window::QuickReadinessState> {
    let state = app
        .try_state::<AppState>()
        .ok_or_else(|| Error::Other("AppState is missing".into()))?;
    Ok(crate::window::QuickReadinessState {
        frontend_ready: state
            .quick_frontend_ready
            .load(std::sync::atomic::Ordering::Acquire),
        data_hydrated: state
            .quick_data_hydrated
            .load(std::sync::atomic::Ordering::Acquire),
        open_pending: state
            .quick_open_pending
            .load(std::sync::atomic::Ordering::Acquire),
    })
}

#[tauri::command]
pub async fn show_quick_palette(app: AppHandle) -> Result<()> {
    crate::window::show_quick(&app);
    Ok(())
}

#[tauri::command]
pub async fn hide_quick_palette(app: AppHandle) -> Result<()> {
    crate::window::hide_quick(&app);
    Ok(())
}

#[tauri::command]
pub async fn toggle_quick_palette(app: AppHandle) -> Result<()> {
    crate::window::toggle_quick(&app);
    Ok(())
}

#[tauri::command]
pub async fn show_full_application(app: AppHandle) -> Result<()> {
    crate::window::show_full(&app);
    Ok(())
}

#[tauri::command]
pub async fn hide_full_application(app: AppHandle) -> Result<()> {
    crate::window::hide_full(&app);
    Ok(())
}

/// Pins the quick palette so clicking away no longer dismisses it.
///
/// The flag is stored natively because the light-dismiss decision is taken in
/// the Rust `Focused(false)` handler, at which point React state is unreachable.
#[tauri::command]
pub async fn set_quick_pinned(app: AppHandle, value: bool) -> Result<bool> {
    crate::window::set_quick_pinned(&app, value);
    Ok(value)
}

#[tauri::command]
pub async fn set_always_on_top(window: tauri::WebviewWindow, value: bool) -> Result<bool> {
    // The quick palette is always topmost by contract; only the full
    // application exposes a user-controlled always-on-top toggle.
    if crate::window::mode_for_label(window.label()) == crate::window::WindowMode::Quick {
        return Ok(true);
    }
    window
        .set_always_on_top(value)
        .map_err(|error| Error::Other(error.to_string()))?;
    Ok(value)
}

/// Applies a preview-visibility change to the window that requested it.
///
/// Quick and full keep *separate* persisted preferences and separate
/// dimensions. A single shared `showPreview` used to resize whichever window
/// happened to be mounted, so toggling the preview in the flyout also resized
/// the desktop application.
#[tauri::command]
pub async fn set_preview_visible(
    app: AppHandle,
    window: tauri::WebviewWindow,
    state: tauri::State<'_, AppState>,
    value: bool,
) -> Result<bool> {
    match crate::window::mode_for_label(window.label()) {
        crate::window::WindowMode::Quick => {
            persist_quick_preview(&app, &state, value)?;
            let foreground = *state.foreground.lock();
            crate::window::layout_quick(&window, value, foreground);
        }
        crate::window::WindowMode::Full => resize_full_for_preview(&window, value)?,
        crate::window::WindowMode::Settings => {}
    }
    Ok(value)
}

/// Persists the quick palette's own compact/expanded choice.
fn persist_quick_preview(
    app: &AppHandle,
    state: &tauri::State<'_, AppState>,
    value: bool,
) -> Result<()> {
    let mut next = state.settings.read().clone();
    if next.quick_preview_expanded == value {
        return Ok(());
    }
    next.quick_preview_expanded = value;
    state.db.save_settings(&next)?;
    *state.settings.write() = next.clone();
    let _ = app.emit("settings-updated", &next);
    Ok(())
}

/// Keeps the full application freely resizable. Preview visibility is a user
/// preference; the frontend temporarily collapses the pane below 780px without
/// rewriting that preference or forcing the window back to a desktop size.
fn resize_full_for_preview(window: &tauri::WebviewWindow, _value: bool) -> Result<()> {
    use tauri::{LogicalSize, Size};

    window
        .set_min_size(Some(Size::Logical(LogicalSize::new(640.0, 460.0))))
        .map_err(|error| Error::Other(error.to_string()))
}

#[tauri::command]
pub async fn sync_state(state: tauri::State<'_, AppState>) -> Result<SyncState> {
    let settings = state.settings.read().clone();
    Ok(state.sync.state(&settings))
}

#[tauri::command]
pub async fn sync_history_now(app: AppHandle, state: tauri::State<'_, AppState>) -> Result<usize> {
    let queued = state.sync.sync_history_now();
    let _ = app.emit("sync-peers-updated", ());
    Ok(queued)
}

#[tauri::command]
pub async fn regenerate_pairing_code(
    app: AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<Settings> {
    let previous = state.settings.read().clone();
    let mut next = previous.clone();
    next.sync_pairing_code = format!(
        "{:06}",
        (crate::models::now_ms().unsigned_abs() ^ u64::from(std::process::id())) % 1_000_000
    );
    state.db.save_settings(&next)?;
    *state.settings.write() = next.clone();
    state.sync.settings_changed(&previous, &next);
    apply_runtime_settings(&app, &next)?;
    let _ = app.emit("settings-updated", &next);
    let _ = app.emit("sync-peers-updated", ());
    Ok(next)
}

#[tauri::command]
pub async fn quit_app(app: AppHandle) -> Result<()> {
    app.exit(0);
    Ok(())
}

// ---- helpers -------------------------------------------------------------

fn cleanup_asset_paths(storage_root: &std::path::Path, orphans: Vec<String>) {
    for path in orphans {
        let p = PathBuf::from(&path);
        if let Err(err) = crate::storage::remove_managed_asset(storage_root, &p) {
            log::debug!("could not remove orphan {path}: {err}");
        }
    }
}

fn rollback_storage_target(app: &AppHandle, target: &std::path::Path) {
    revoke_storage_target_scope(app, target);
    if let Err(error) = crate::storage::remove_managed_directories(target) {
        log::warn!("failed storage target cleanup was incomplete: {error}");
    }
}

fn allow_storage_target_scope(app: &AppHandle, target: &std::path::Path) -> Result<()> {
    for managed_root in crate::storage::managed_asset_roots(target) {
        app.asset_protocol_scope()
            .allow_directory(managed_root, true)
            .map_err(|error| Error::Other(error.to_string()))?;
    }
    Ok(())
}

fn revoke_storage_target_scope(app: &AppHandle, target: &std::path::Path) {
    for managed_root in crate::storage::managed_asset_roots(target) {
        if let Err(error) = app
            .asset_protocol_scope()
            .forbid_directory(managed_root, true)
        {
            log::warn!("failed storage target scope could not be removed: {error}");
        }
    }
}

fn enforce_history_policy(state: &tauri::State<'_, AppState>) -> Result<()> {
    let _storage_guard = state.storage_operation.read();
    let settings = state.settings.read().clone();
    let storage_root = state.storage_root.read().clone();
    let orphans = state
        .db
        .prune(settings.max_items, settings.retention_days)?;
    cleanup_asset_paths(&storage_root, orphans);
    Ok(())
}

/// Applies retention immediately during startup, before clipboard capture can
/// enqueue background asset work.
pub fn enforce_history_policy_on_startup(app: &App) -> Result<()> {
    let state: tauri::State<AppState> = app.state();
    enforce_history_policy(&state)
}

/// Installs both saved global shortcuts without making startup depend on them.
///
/// A stale, unsupported, or OS-conflicting pair is replaced with the first
/// available safe fallback and persisted so Settings keeps telling the truth
/// about what is actually bound.
pub fn install_hotkeys(app: &App) {
    let state: tauri::State<AppState> = app.state();
    let (saved_quick, saved_full) = {
        let settings = state.settings.read();
        (settings.hotkey.clone(), settings.full_window_hotkey.clone())
    };
    if register_hotkeys(app.handle(), &saved_quick, &saved_full).is_ok() {
        return;
    }

    log::warn!("saved global shortcuts are unavailable; trying safe fallbacks");
    const FALLBACKS: [(&str, &str); 3] = [
        ("Ctrl+Shift+V", "Ctrl+Alt+Shift+V"),
        ("Ctrl+Alt+V", "Ctrl+Alt+Shift+C"),
        ("Ctrl+Shift+C", "Ctrl+Alt+Shift+D"),
    ];
    for (quick, full) in FALLBACKS {
        if quick == saved_quick && full == saved_full {
            continue;
        }
        if register_hotkeys(app.handle(), quick, full).is_ok() {
            let mut settings = state.settings.read().clone();
            settings.hotkey = quick.to_string();
            settings.full_window_hotkey = full.to_string();
            if let Err(error) = state.db.save_settings(&settings) {
                log::error!("could not persist fallback global shortcuts: {error}");
            }
            *state.settings.write() = settings.clone();
            let _ = app.emit("settings-updated", &settings);
            return;
        }
    }
    log::error!("no safe global shortcuts could be registered; use the tray icon to open Clipmo");
}

pub fn install_clipboard_listener(app: &App) -> Result<()> {
    let state: tauri::State<AppState> = app.state();
    let (snapshot_tx, snapshot_rx) = mpsc::sync_channel::<SnapshotJob>(16);
    let snapshot_db = Arc::clone(&state.db);
    let snapshot_app = app.handle().clone();
    let snapshot_storage_root = Arc::clone(&state.storage_root);
    let snapshot_storage_operation = Arc::clone(&state.storage_operation);
    std::thread::Builder::new()
        .name("file-snapshot".into())
        .spawn(move || {
            while let Ok(job) = snapshot_rx.recv() {
                let _storage_guard = snapshot_storage_operation.read();
                if snapshot_db.get(job.id).ok().flatten().is_none() {
                    continue;
                }
                let storage_root = snapshot_storage_root.read().clone();
                match crate::storage::snapshot_files(
                    &storage_root,
                    &job.hash,
                    &job.originals,
                    job.max_bytes,
                ) {
                    Ok(assets) => match snapshot_db.set_file_assets(job.id, &assets) {
                        Ok(orphans) => {
                            cleanup_asset_paths(&storage_root, orphans);
                            if let Ok(item) = snapshot_db.get_required(job.id) {
                                let _ = snapshot_app.emit("clip-updated", &item);
                            }
                        }
                        Err(error) => {
                            let group = crate::storage::file_root(&storage_root).join(&job.hash);
                            if group.exists() {
                                cleanup_asset_paths(
                                    &storage_root,
                                    vec![group.to_string_lossy().into_owned()],
                                );
                            }
                            log::error!("file snapshot DB update failed: {error}");
                        }
                    },
                    Err(error) => {
                        let group = crate::storage::file_root(&storage_root).join(&job.hash);
                        if group.exists() {
                            cleanup_asset_paths(
                                &storage_root,
                                vec![group.to_string_lossy().into_owned()],
                            );
                        }
                        log::error!("file snapshot failed: {error}");
                    }
                }
            }
        })
        .map_err(|error| Error::Other(format!("snapshot worker start failed: {error}")))?;

    let sink = Arc::new(TauriSink {
        db: Arc::clone(&state.db),
        app: app.handle().clone(),
        storage_root: Arc::clone(&state.storage_root),
        storage_operation: Arc::clone(&state.storage_operation),
        settings: Arc::clone(&state.settings),
        sync: state.sync.clone(),
        snapshot_tx,
    });
    listener::start_listener(sink).map_err(|e| Error::Other(format!("listener start failed: {e}")))
}

/// Bridge from the listener thread to the DB and the webview.
struct TauriSink {
    db: Arc<Db>,
    app: AppHandle,
    storage_root: Arc<parking_lot::RwLock<PathBuf>>,
    storage_operation: Arc<parking_lot::RwLock<()>>,
    settings: Arc<parking_lot::RwLock<Settings>>,
    sync: crate::sync::SyncService,
    snapshot_tx: mpsc::SyncSender<SnapshotJob>,
}

struct SnapshotJob {
    id: i64,
    hash: String,
    originals: Vec<String>,
    max_bytes: u64,
}

impl CaptureSink for TauriSink {
    fn handle(&self, mut event: ClipEvent) {
        let settings = self.settings.read().clone();
        if (event.kind == ItemKind::Image && !settings.capture_images)
            || (event.kind == ItemKind::Files && !settings.capture_files)
        {
            return;
        }
        #[cfg(debug_assertions)]
        log::debug!("capture_filter kind={:?}", event.kind);
        if event.kind == ItemKind::Files {
            let original_files = event.files.clone();
            let configured: &[String] = match settings.file_filter_mode {
                crate::models::FileFilterMode::Include => &settings.file_include_extensions,
                crate::models::FileFilterMode::Exclude => &settings.file_exclude_extensions,
                crate::models::FileFilterMode::All => &[],
            };
            event.files = crate::capture_policy::filter_local_files(
                &original_files,
                settings.file_filter_mode,
                configured,
            );
            #[cfg(debug_assertions)]
            {
                let rejected: Vec<_> = original_files
                    .iter()
                    .filter(|path| !event.files.contains(path))
                    .collect();
                let original_names: Vec<_> = original_files
                    .iter()
                    .filter_map(|path| PathBuf::from(path).file_name().map(|name| name.to_owned()))
                    .collect();
                let accepted_names: Vec<_> = event
                    .files
                    .iter()
                    .filter_map(|path| PathBuf::from(path).file_name().map(|name| name.to_owned()))
                    .collect();
                let rejected_names: Vec<_> = rejected
                    .iter()
                    .filter_map(|path| PathBuf::from(path).file_name().map(|name| name.to_owned()))
                    .collect();
                log::debug!(
                    "capture_filter kind=files original_names={:?} mode={:?} extensions={:?} accepted_names={:?} rejected_names={:?}",
                    original_names,
                    settings.file_filter_mode,
                    crate::capture_policy::normalize_extensions(configured),
                    accepted_names,
                    rejected_names
                );
            }
            if event.files.is_empty() {
                return;
            }
            let paths: Vec<PathBuf> = event.files.iter().map(PathBuf::from).collect();
            event.content = event.files.join("\n");
            event.size_bytes = event.content.len().min(i64::MAX as usize) as i64;
            event.content_hash = crate::clipboard::hash_files(&paths);
            let first = paths
                .first()
                .and_then(|path| path.file_name())
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| "Files".into());
            event.preview = match paths.len().saturating_sub(1) {
                0 => first,
                more => format!("{first} + {more} more"),
            };
        }
        if event.source.as_ref().is_some_and(|source| {
            crate::win::source::is_current_process(&source.exe_path)
                || settings
                    .ignored_apps
                    .iter()
                    .any(|ignored| crate::capture_policy::source_matches_ignored(source, ignored))
        }) {
            #[cfg(debug_assertions)]
            log::debug!("capture_filter source_rejected=true");
            return;
        }

        let _storage_guard = self.storage_operation.read();
        let storage_root = self.storage_root.read().clone();
        match persist(&self.db, &storage_root, &event, &settings) {
            Ok(Persisted { item, is_new }) => {
                match self.db.prune(settings.max_items, settings.retention_days) {
                    Ok(orphans) => cleanup_asset_paths(&storage_root, orphans),
                    Err(error) => log::error!("automatic history cleanup failed: {error}"),
                }
                if is_new {
                    let _ = self.app.emit("clip-updated", &item);
                    self.sync.enqueue_item(&item);
                } else {
                    let _ = self.app.emit("clip-touched", &event.content_hash);
                }

                let retryable_assets = item.file_assets.is_empty()
                    || item.file_assets.iter().all(|asset| {
                        matches!(
                            asset.status,
                            StoredFileStatus::Failed | StoredFileStatus::Skipped
                        )
                    });
                if event.kind == ItemKind::Files
                    && settings.store_file_snapshots
                    && !event.files.is_empty()
                    && (is_new || retryable_assets)
                {
                    let job = SnapshotJob {
                        id: item.id,
                        hash: event.content_hash.clone(),
                        originals: event.files.clone(),
                        max_bytes: u64::from(settings.max_snapshot_size_mb) * 1024 * 1024,
                    };
                    match self.snapshot_tx.try_send(job) {
                        Ok(()) => {}
                        Err(mpsc::TrySendError::Full(job)) => {
                            self.mark_snapshot_failed(
                                job,
                                &storage_root,
                                "Snapshot queue was busy; copy the files again to retry",
                            );
                        }
                        Err(mpsc::TrySendError::Disconnected(job)) => {
                            self.mark_snapshot_failed(
                                job,
                                &storage_root,
                                "Snapshot worker is unavailable",
                            );
                        }
                    }
                }
            }
            Err(err) => log::error!("failed to persist clipboard event: {err}"),
        }
    }
}

impl TauriSink {
    fn mark_snapshot_failed(
        &self,
        job: SnapshotJob,
        storage_root: &std::path::Path,
        message: &str,
    ) {
        let assets: Vec<StoredFile> = job
            .originals
            .iter()
            .map(|path| StoredFile {
                original_path: path.clone(),
                stored_path: None,
                size_bytes: 0,
                is_directory: PathBuf::from(path).is_dir(),
                status: StoredFileStatus::Failed,
                message: Some(message.to_string()),
                thumb_path: None,
            })
            .collect();
        match self.db.set_file_assets(job.id, &assets) {
            Ok(orphans) => cleanup_asset_paths(storage_root, orphans),
            Err(error) => log::error!("could not record snapshot queue failure: {error}"),
        }
    }
}

struct Persisted {
    item: ClipItem,
    is_new: bool,
}

fn persist(
    db: &Db,
    storage_root: &std::path::Path,
    event: &ClipEvent,
    settings: &Settings,
) -> Result<Persisted> {
    let (image_meta, size_bytes) = if event.kind == ItemKind::Image {
        let bytes = event
            .image_bytes
            .as_deref()
            .ok_or_else(|| Error::Other("captured image bytes were missing".into()))?;
        let hash = &event.content_hash;
        if let Some(existing) = db.get_by_hash(hash)?.filter(|item| item.image.is_some()) {
            (existing.image, existing.size_bytes)
        } else {
            let image = image::load_from_memory_with_format(bytes, image::ImageFormat::Png)
                .map_err(Error::Image)?;
            let (extension, encoded) = encode_stored_image(&image, bytes, settings)?;
            let img_path =
                crate::storage::image_root(storage_root).join(format!("{hash}.{extension}"));
            let thumb_path = crate::storage::thumb_root(storage_root).join(format!("{hash}.png"));
            std::fs::write(&img_path, &encoded)?;
            write_thumbnail(bytes, &thumb_path)?;
            let (w, h) = (image.width(), image.height());
            (
                Some(ImageMeta {
                    path: img_path.to_string_lossy().to_string(),
                    thumb_path: thumb_path.to_string_lossy().to_string(),
                    width: w,
                    height: h,
                }),
                encoded.len().min(i64::MAX as usize) as i64,
            )
        }
    } else {
        (None, event.size_bytes)
    };

    let file_assets = if event.kind == ItemKind::Files && settings.store_file_snapshots {
        event
            .files
            .iter()
            .map(|path| StoredFile {
                original_path: path.clone(),
                stored_path: None,
                size_bytes: 0,
                is_directory: PathBuf::from(path).is_dir(),
                status: StoredFileStatus::Pending,
                message: None,
                thumb_path: None,
            })
            .collect()
    } else {
        Vec::new()
    };

    let new = crate::models::NewItem {
        kind: event.kind,
        preview: event.preview.clone(),
        content: event.content.clone(),
        html: event.html.clone(),
        rtf: event.rtf.clone(),
        image: image_meta.clone(),
        files: event.files.clone(),
        file_assets,
        size_bytes,
        content_hash: event.content_hash.clone(),
        source: event.source.clone(),
        device: None,
        sync_status: crate::models::SyncStatus::Local,
    };

    let upsert = db.upsert(&new)?;
    let item = db.get_required(upsert.id())?;
    Ok(Persisted {
        item,
        is_new: upsert.is_new(),
    })
}

fn encode_stored_image(
    image: &image::DynamicImage,
    original_png: &[u8],
    settings: &Settings,
) -> Result<(&'static str, Vec<u8>)> {
    use image::ImageEncoder;
    use std::io::Cursor;

    let quality = match settings.image_compression {
        ImageCompression::None => 100,
        ImageCompression::Normal => 82,
        ImageCompression::Best => 68,
        ImageCompression::Manual => settings.image_quality.clamp(1, 100),
    };
    match settings.image_format {
        ImageFormat::Original => Ok(("png", original_png.to_vec())),
        ImageFormat::Png => {
            let rgba = image.to_rgba8();
            let mut output = Vec::new();
            let compression = match settings.image_compression {
                ImageCompression::None => image::codecs::png::CompressionType::Fast,
                ImageCompression::Best => image::codecs::png::CompressionType::Best,
                _ => image::codecs::png::CompressionType::Default,
            };
            image::codecs::png::PngEncoder::new_with_quality(
                &mut output,
                compression,
                image::codecs::png::FilterType::Adaptive,
            )
            .write_image(
                &rgba,
                rgba.width(),
                rgba.height(),
                image::ExtendedColorType::Rgba8,
            )
            .map_err(Error::Image)?;
            Ok(("png", output))
        }
        ImageFormat::Jpeg => {
            let rgb = image.to_rgb8();
            let mut output = Vec::new();
            image::codecs::jpeg::JpegEncoder::new_with_quality(&mut output, quality)
                .encode_image(&image::DynamicImage::ImageRgb8(rgb))
                .map_err(Error::Image)?;
            Ok(("jpg", output))
        }
        ImageFormat::Webp => {
            let rgba = image.to_rgba8();
            let mut output = Cursor::new(Vec::new());
            image::codecs::webp::WebPEncoder::new_lossless(&mut output)
                .encode(
                    &rgba,
                    rgba.width(),
                    rgba.height(),
                    image::ExtendedColorType::Rgba8,
                )
                .map_err(Error::Image)?;
            Ok(("webp", output.into_inner()))
        }
    }
}

fn write_thumbnail(bytes: &[u8], dest: &std::path::Path) -> Result<()> {
    let img = image::load_from_memory_with_format(bytes, image::ImageFormat::Png)
        .map_err(Error::Image)?;
    let thumb = img.thumbnail(256, 256);
    thumb
        .save_with_format(dest, image::ImageFormat::Png)
        .map_err(Error::Image)
}

/// Pushes the runtime parts of the settings (hotkey, backdrop, theme) to the
/// running app. Called from `save_settings`.
pub fn apply_runtime_settings(app: &AppHandle, settings: &Settings) -> Result<()> {
    crate::native_appearance::apply_all(app, settings);
    Ok(())
}

fn autostart_enabled(app: &AppHandle) -> Result<bool> {
    app.autolaunch().is_enabled().map_err(|error| {
        Error::Other(format!(
            "Windows launch-at-login state could not be read: {error}"
        ))
    })
}

fn set_autostart_enabled(app: &AppHandle, enabled: bool) -> Result<()> {
    let registration = app.autolaunch();
    let result = if enabled {
        registration.enable()
    } else {
        registration.disable()
    };
    result.map_err(|error| {
        Error::Other(format!(
            "Windows launch-at-login could not be updated: {error}"
        ))
    })?;

    let registered = autostart_enabled(app)?;
    if registered != enabled {
        return Err(Error::Other(
            "Windows did not keep the requested launch-at-login setting".into(),
        ));
    }
    Ok(())
}

fn validate_filter_shortcuts(settings: &Settings) -> Result<()> {
    const LABELS: [&str; 9] = [
        "Toggle navigation",
        "All history",
        "Favorites",
        "Text filter",
        "Image filter",
        "Link filter",
        "File filter",
        "Email filter",
        "Color filter",
    ];
    if settings.filter_shortcuts.len() != LABELS.len() {
        return Err(Error::Other(format!(
            "Keyboard shortcuts must contain exactly {} filter actions",
            LABELS.len()
        )));
    }
    let (quick, full) =
        crate::hotkey::validate_distinct(&settings.hotkey, &settings.full_window_hotkey)?;
    let mut assigned =
        std::collections::HashMap::from([(quick, "Quick clipboard"), (full, "Open full Clipmo")]);
    for index in 0..=9 {
        let combination = format!("Ctrl+{index}");
        assigned.insert(
            crate::hotkey::parse(&combination)?,
            "Device quick switching",
        );
    }
    for (label, combination) in LABELS.into_iter().zip(&settings.filter_shortcuts) {
        let parsed = crate::hotkey::parse(combination)
            .map_err(|error| Error::Other(format!("{label}: {error}")))?;
        if let Some(existing) = assigned.insert(parsed, label) {
            return Err(Error::Other(format!(
                "“{label}” and “{existing}” cannot use the same shortcut"
            )));
        }
    }
    Ok(())
}

/// Registers both global actions atomically.
///
/// Either both accelerators end up bound, or nothing changes: a partial apply
/// would leave the user with one working shortcut and one dead one after a
/// failed save. The previous registrations are only released once the new ones
/// have been accepted by the OS.
fn register_hotkeys(app: &AppHandle, quick_combo: &str, full_combo: &str) -> Result<()> {
    let (quick, full) = crate::hotkey::validate_distinct(quick_combo, full_combo)?;
    let state: tauri::State<AppState> = app.state();
    let mut active = state.hotkeys.lock();
    if active.quick == Some(quick) && active.full == Some(full) {
        return Ok(());
    }

    let manager = app.global_shortcut();
    manager
        .on_shortcut(quick, move |app, _shortcut, event| {
            if matches!(event.state(), ShortcutState::Pressed) {
                crate::window::toggle_quick(app);
            }
        })
        .map_err(|error| {
            Error::Other(format!(
                "{} shortcut could not be registered: {error}",
                crate::hotkey::HotkeyAction::QuickPalette.label()
            ))
        })?;

    if let Err(error) = manager.on_shortcut(full, move |app, _shortcut, event| {
        if matches!(event.state(), ShortcutState::Pressed) {
            crate::window::show_full(app);
        }
    }) {
        // Roll the half-applied change back so the caller can restore the
        // previously working pair without leaking a stray registration.
        if let Err(cleanup) = manager.unregister(quick) {
            log::error!("could not roll back the quick palette shortcut: {cleanup}");
        }
        return Err(Error::Other(format!(
            "{} shortcut could not be registered: {error}",
            crate::hotkey::HotkeyAction::FullWindow.label()
        )));
    }

    for previous in [active.quick, active.full].into_iter().flatten() {
        if previous == quick || previous == full {
            continue;
        }
        if let Err(error) = manager.unregister(previous) {
            log::warn!("a previous global shortcut could not be released: {error}");
        }
    }
    active.quick = Some(quick);
    active.full = Some(full);
    Ok(())
}

pub fn show_settings_window(app: &AppHandle) -> std::result::Result<(), String> {
    if let Some(existing) = app.get_webview_window("settings") {
        let state: tauri::State<AppState> = app.state();
        let settings = state.settings.read().clone();
        let system = crate::win::appearance::read();
        crate::native_appearance::apply_window(&existing, &settings, &system);
        if existing.is_minimized().unwrap_or(false) {
            existing.unminimize().map_err(|error| error.to_string())?;
        }
        existing.show().map_err(|error| error.to_string())?;
        existing.set_focus().map_err(|error| error.to_string())?;
        return Ok(());
    }
    let window =
        WebviewWindowBuilder::new(app, "settings", WebviewUrl::App("settings.html".into()))
            .title("Clipmo settings")
            .inner_size(800.0, 680.0)
            .min_inner_size(680.0, 560.0)
            .resizable(true)
            .decorations(true)
            .transparent(true)
            .skip_taskbar(true)
            .center()
            .visible(false)
            .build()
            .map_err(|e| e.to_string())?;
    let state: tauri::State<AppState> = app.state();
    let settings = state.settings.read().clone();
    let system = crate::win::appearance::read();
    crate::native_appearance::apply_window(&window, &settings, &system);
    window.show().map_err(|e| e.to_string())?;
    window.set_focus().map_err(|e| e.to_string())
}
