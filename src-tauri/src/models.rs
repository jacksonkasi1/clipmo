//! Data types shared between the Rust core and the web frontend.
//!
//! All types are `camelCase` on the wire to match TypeScript conventions; the
//! mirrored definitions live in `src/lib/types.ts`.

use serde::{Deserialize, Serialize};

/// The high-level category of a clipboard entry.
///
/// This drives both the icon shown in the list and the filter tabs. It is
/// derived once at capture time (see `clipboard::classify`) so that filtering
/// never has to re-parse content.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ItemKind {
    #[default]
    Text,
    Link,
    Email,
    Color,
    Image,
    Files,
}

impl ItemKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Link => "link",
            Self::Email => "email",
            Self::Color => "color",
            Self::Image => "image",
            Self::Files => "files",
        }
    }

    pub fn from_db_value(value: &str) -> Self {
        match value {
            "link" => Self::Link,
            "email" => Self::Email,
            "color" => Self::Color,
            "image" => Self::Image,
            "files" => Self::Files,
            _ => Self::Text,
        }
    }
}

/// Metadata about a captured image. The bytes themselves live on disk so that
/// listing the history never loads image data.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageMeta {
    /// Absolute path to the full-resolution PNG.
    pub path: String,
    /// Absolute path to the downscaled preview PNG.
    pub thumb_path: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StoredFileStatus {
    #[default]
    Pending,
    Ready,
    Skipped,
    Failed,
}

/// Durable snapshot metadata for a copied file or folder. The original path is
/// display-only; history cleanup never mutates the original filesystem item.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredFile {
    pub original_path: String,
    pub stored_path: Option<String>,
    pub size_bytes: u64,
    pub is_directory: bool,
    pub status: StoredFileStatus,
    pub message: Option<String>,
    /// Absolute path to a generated thumbnail when the original is an image.
    /// `None` for non-image files or when generation has not yet succeeded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thumb_path: Option<String>,
}

/// The application that owned the clipboard when an entry was captured.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceApp {
    /// Friendly name, e.g. "Visual Studio Code".
    pub name: String,
    /// Full path to the executable.
    pub exe_path: String,
    /// Absolute path to the extracted 32x32 PNG icon, if extraction succeeded.
    pub icon_path: Option<String>,
}

/// Stable identity used by capture exclusions and application pickers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IgnoredApp {
    pub id: String,
    pub display_name: String,
    pub executable_path: String,
    pub executable_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app_user_model_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package_family_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon_path: Option<String>,
}

impl IgnoredApp {
    pub fn from_legacy(value: &str) -> Self {
        let value = value.trim();
        let path = std::path::Path::new(value);
        let executable_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(value)
            .to_string();
        let display_name = path
            .file_stem()
            .and_then(|name| name.to_str())
            .unwrap_or(value)
            .to_string();
        let normalized = value.replace('/', "\\").to_lowercase();
        let id = format!("exe:{}", normalized.trim());
        Self {
            id,
            display_name,
            executable_path: if path.components().count() > 1 {
                value.to_string()
            } else {
                String::new()
            },
            executable_name,
            app_user_model_id: None,
            package_family_name: None,
            icon_path: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplicationInfo {
    #[serde(flatten)]
    pub identity: IgnoredApp,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publisher: Option<String>,
    pub running: bool,
    pub installed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recently_used: Option<bool>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PlatformKind {
    #[default]
    Windows,
    Macos,
    Linux,
    Android,
    Ios,
    Unknown,
}

impl PlatformKind {
    pub fn current() -> Self {
        if cfg!(target_os = "windows") {
            Self::Windows
        } else if cfg!(target_os = "macos") {
            Self::Macos
        } else if cfg!(target_os = "linux") {
            Self::Linux
        } else if cfg!(target_os = "android") {
            Self::Android
        } else if cfg!(target_os = "ios") {
            Self::Ios
        } else {
            Self::Unknown
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Windows => "windows",
            Self::Macos => "macos",
            Self::Linux => "linux",
            Self::Android => "android",
            Self::Ios => "ios",
            Self::Unknown => "unknown",
        }
    }

    pub fn from_db_value(value: &str) -> Self {
        match value {
            "windows" => Self::Windows,
            "macos" => Self::Macos,
            "linux" => Self::Linux,
            "android" => Self::Android,
            "ios" => Self::Ios,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceIdentity {
    pub id: String,
    pub name: String,
    pub platform: PlatformKind,
    pub color: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SyncStatus {
    #[default]
    Local,
    Synced,
    Pending,
    Offline,
}

impl SyncStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Synced => "synced",
            Self::Pending => "pending",
            Self::Offline => "offline",
        }
    }

    pub fn from_db_value(value: &str) -> Self {
        match value {
            "synced" => Self::Synced,
            "pending" => Self::Pending,
            "offline" => Self::Offline,
            _ => Self::Local,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncPeer {
    pub device: DeviceIdentity,
    pub last_seen_at: i64,
    pub status: SyncStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncState {
    pub enabled: bool,
    pub device: DeviceIdentity,
    pub pairing_code: String,
    pub peers: Vec<SyncPeer>,
}

/// A single clipboard history entry as sent to the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipItem {
    pub id: i64,
    pub kind: ItemKind,
    /// Single-line label rendered in the list (already trimmed and truncated).
    pub preview: String,
    /// Full plain-text payload. Empty for image entries.
    pub content: String,
    /// Whether a rich HTML flavour was captured alongside the plain text.
    pub has_html: bool,
    /// Whether an RTF flavour was captured alongside the plain text.
    pub has_rtf: bool,
    pub image: Option<ImageMeta>,
    pub files: Vec<String>,
    pub file_assets: Vec<StoredFile>,
    pub size_bytes: i64,
    pub source: Option<SourceApp>,
    /// User-defined local labels used for organization and search.
    #[serde(default)]
    pub tags: Vec<String>,
    pub favorite: bool,
    pub copy_count: i64,
    pub device: DeviceIdentity,
    pub sync_status: SyncStatus,
    /// Unix milliseconds.
    pub first_copied_at: i64,
    /// Unix milliseconds.
    pub last_copied_at: i64,
}

/// Filter + paging parameters for a history query.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListQuery {
    /// Free-text search. When present, matched via SQLite FTS5 prefix search.
    #[serde(default)]
    pub search: Option<String>,
    /// Restrict to one or more categories. Empty = no kind filter.
    #[serde(default)]
    pub kinds: Vec<ItemKind>,
    /// Restrict results to clipboard entries originating from these devices.
    /// Empty = all devices. The special local id is stored as `local`.
    #[serde(default)]
    pub device_ids: Vec<String>,
    /// Restrict results to entries carrying any of these normalized tags.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Restrict results to entries captured from these executable paths.
    #[serde(default)]
    pub source_exes: Vec<String>,
    /// Only return starred entries.
    #[serde(default)]
    pub favorites_only: bool,
    #[serde(default)]
    pub limit: Option<u32>,
    #[serde(default)]
    pub offset: Option<u32>,
}

/// Which rich flavour to place on the clipboard when copying an entry back.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PasteFlavor {
    /// Restore every flavour that was captured (HTML/RTF/image/files).
    #[default]
    Original,
    /// Force plain text, discarding formatting.
    PlainText,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileFilterMode {
    All,
    Include,
    #[default]
    Exclude,
}

fn default_included_extensions() -> Vec<String> {
    [".txt", ".pdf"].into_iter().map(str::to_string).collect()
}

fn default_excluded_extensions() -> Vec<String> {
    [
        ".exe", ".bat", ".cmd", ".msi", ".scr", ".com", ".dll", ".sys",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImageFormat {
    #[default]
    Original,
    Png,
    Jpeg,
    Webp,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImageCompression {
    None,
    #[default]
    Normal,
    Best,
    Manual,
}

/// Default accelerator for the decorated application window. Deliberately
/// distinct from the quick palette so the two actions never collide.
fn default_full_window_hotkey() -> String {
    "Ctrl+Alt+Shift+V".to_string()
}

fn default_image_quality() -> u8 {
    80
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FilterScope {
    pub kind: FilterScopeKind,
    pub value: String,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FilterScopeKind {
    Tag,
    Device,
    Source,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BulkFilterAction {
    FavoriteAll,
    DeleteNonFavorites,
    DeleteAll,
}

fn default_filter_shortcuts() -> Vec<String> {
    [
        "Ctrl+B", "Alt+1", "Alt+2", "Alt+3", "Alt+4", "Alt+5", "Alt+6", "Alt+7", "Alt+8",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

/// User-facing configuration, persisted in the `settings` table.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    /// Settings schema version used for one-time behavioral migrations.
    #[serde(default = "default_settings_version")]
    pub settings_version: u32,
    /// Global hotkey that toggles the frameless quick clipboard palette, in
    /// Tauri accelerator syntax, e.g. `Ctrl+Shift+V`.
    pub hotkey: String,
    /// Global hotkey that opens the full, decorated Clipdeck application
    /// window. Must never equal [`Settings::hotkey`].
    #[serde(default = "default_full_window_hotkey")]
    pub full_window_hotkey: String,
    /// In-app navigation/category shortcuts. Ordering is shared with the
    /// frontend FILTER_SHORTCUTS definition.
    #[serde(default = "default_filter_shortcuts")]
    pub filter_shortcuts: Vec<String>,
    /// Maximum number of non-favorite entries retained. 0 disables pruning.
    pub max_items: u32,
    /// Delete non-favorite entries older than this many days. 0 disables.
    pub retention_days: u32,
    /// Capture images from the clipboard.
    pub capture_images: bool,
    /// Capture file/folder copies.
    pub capture_files: bool,
    /// Save durable snapshots for file/folder clipboard entries.
    #[serde(default = "default_true")]
    pub store_file_snapshots: bool,
    /// Maximum bytes stored for one clipboard file or folder group.
    #[serde(default = "default_snapshot_limit_mb")]
    pub max_snapshot_size_mb: u32,
    /// Optional include/exclude policy for local file clipboard capture.
    #[serde(default)]
    pub file_filter_mode: FileFilterMode,
    /// Lowercase extensions such as `.txt` used by Include mode.
    #[serde(default = "default_included_extensions")]
    pub file_include_extensions: Vec<String>,
    /// Lowercase extensions such as `.exe` used by Exclude mode.
    #[serde(default = "default_excluded_extensions")]
    pub file_exclude_extensions: Vec<String>,
    /// Managed image format. Clipboard restoration remains visually equivalent.
    #[serde(default)]
    pub image_format: ImageFormat,
    #[serde(default)]
    pub image_compression: ImageCompression,
    #[serde(default = "default_image_quality")]
    pub image_quality: u8,
    /// Optional managed-content root. `None` uses Windows app data.
    #[serde(default)]
    pub storage_path: Option<String>,
    /// Skip entries whose stable source identity matches one of these apps.
    #[serde(default, deserialize_with = "deserialize_ignored_apps")]
    pub ignored_apps: Vec<IgnoredApp>,
    /// Window backdrop material.
    pub backdrop: Backdrop,
    /// Theme preference.
    pub theme: ThemeMode,
    /// Paste immediately into the previously focused app on Enter.
    pub paste_on_enter: bool,
    /// Launch Clipdeck when Windows starts.
    pub launch_at_login: bool,
    /// Show the preview pane in the **full application** window.
    ///
    /// The quick palette has its own independent preference below; a single
    /// shared flag would let one window resize the other.
    pub show_preview: bool,
    /// Quick palette compact (`false`) versus expanded (`true`) layout.
    ///
    /// Fresh installs start compact so the flyout stays list-only.
    #[serde(default)]
    pub quick_preview_expanded: bool,
    /// Share clipboard history with trusted devices discovered on the local network.
    #[serde(default)]
    pub sync_enabled: bool,
    /// Stable local device identifier used by LAN sync.
    #[serde(default = "default_device_id")]
    pub sync_device_id: String,
    /// User-visible local device name.
    #[serde(default = "default_device_name")]
    pub sync_device_name: String,
    /// Color used as the local device badge.
    #[serde(default = "default_device_color")]
    pub sync_device_color: String,
    /// Short pairing code required before two devices exchange history.
    #[serde(default = "default_pairing_code")]
    pub sync_pairing_code: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            settings_version: 3,
            // Win+V is reserved by the OS shell and cannot be intercepted by a
            // user process, so we default to the de-facto convention used by
            // third-party clipboard managers on Windows.
            hotkey: "Ctrl+Shift+V".to_string(),
            full_window_hotkey: default_full_window_hotkey(),
            filter_shortcuts: default_filter_shortcuts(),
            max_items: 10_000,
            retention_days: 0,
            capture_images: true,
            capture_files: true,
            store_file_snapshots: true,
            max_snapshot_size_mb: 512,
            file_filter_mode: FileFilterMode::Exclude,
            file_include_extensions: default_included_extensions(),
            file_exclude_extensions: default_excluded_extensions(),
            image_format: ImageFormat::Original,
            image_compression: ImageCompression::Normal,
            image_quality: default_image_quality(),
            storage_path: None,
            ignored_apps: Vec::new(),
            backdrop: Backdrop::Acrylic,
            theme: ThemeMode::System,
            paste_on_enter: true,
            launch_at_login: false,
            show_preview: false,
            quick_preview_expanded: false,
            sync_enabled: false,
            sync_device_id: default_device_id(),
            sync_device_name: default_device_name(),
            sync_device_color: default_device_color(),
            sync_pairing_code: default_pairing_code(),
        }
    }
}

impl Settings {
    pub fn device_identity(&self) -> DeviceIdentity {
        DeviceIdentity {
            id: self.sync_device_id.clone(),
            name: self.sync_device_name.clone(),
            platform: PlatformKind::current(),
            color: self.sync_device_color.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Backdrop {
    /// Desktop Acrylic — matches the Windows 11 transient flyout material.
    Acrylic,
    /// Mica — tinted desktop wallpaper, cheaper to composite.
    Mica,
    /// Opaque Fluent neutral surface. Always available.
    Solid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    System,
    Light,
    Dark,
}

/// OS-derived appearance information pushed to the frontend so the web layer can
/// match the current Windows personalisation settings.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemAppearance {
    /// Accent colour as `#RRGGBB`.
    pub accent: String,
    /// True when Windows is set to a dark app theme.
    pub dark: bool,
}

/// Aggregate counters surfaced to the UI for the bottom status line.
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Counts {
    /// Total entries currently stored.
    pub total: i64,
    /// Entries marked as favourite.
    pub favorites: i64,
    /// Entries pinned by the user (alias for favourites; reserved for the
    /// pin feature added in a later iteration).
    pub pinned: i64,
    pub text: i64,
    pub images: i64,
    pub files: i64,
    pub links: i64,
    pub colors: i64,
    pub emails: i64,
    pub storage_bytes: i64,
}

/// The payload the listener hands to the persistence layer for each new
/// clipboard change. Defined here so the wire contract and the DB insert
/// shape stay aligned.
#[derive(Debug, Clone, Default)]
pub struct NewItem {
    pub kind: ItemKind,
    pub preview: String,
    pub content: String,
    /// Original rich clipboard flavours. These are kept separately from plain
    /// text so copying an entry back can faithfully restore formatting.
    pub html: Option<String>,
    pub rtf: Option<String>,
    pub image: Option<ImageMeta>,
    pub files: Vec<String>,
    pub file_assets: Vec<StoredFile>,
    pub size_bytes: i64,
    pub content_hash: String,
    pub source: Option<SourceApp>,
    pub device: Option<DeviceIdentity>,
    pub sync_status: SyncStatus,
}

fn default_true() -> bool {
    true
}

fn default_snapshot_limit_mb() -> u32 {
    512
}

fn default_settings_version() -> u32 {
    3
}

fn deserialize_ignored_apps<'de, D>(deserializer: D) -> Result<Vec<IgnoredApp>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum StoredIdentity {
        Identity(IgnoredApp),
        Legacy(String),
    }

    let stored = Vec::<StoredIdentity>::deserialize(deserializer)?;
    Ok(stored
        .into_iter()
        .filter_map(|value| match value {
            StoredIdentity::Identity(identity) => Some(identity),
            StoredIdentity::Legacy(value) if !value.trim().is_empty() => {
                Some(IgnoredApp::from_legacy(&value))
            }
            StoredIdentity::Legacy(_) => None,
        })
        .collect())
}

#[cfg(test)]
mod settings_tests {
    use super::*;

    #[test]
    fn default_settings_use_safe_file_exclusions() {
        let settings = Settings::default();
        assert_eq!(settings.file_filter_mode, FileFilterMode::Exclude);
        assert!(settings
            .file_exclude_extensions
            .contains(&".exe".to_string()));
    }

    #[test]
    fn legacy_ignored_app_strings_migrate_to_stable_identities() {
        let settings: Settings = serde_json::from_value(serde_json::json!({
            "ignoredApps": [r"C:\\Apps\\Notepad.exe", "clipdeck.exe"]
        }))
        .unwrap();
        assert_eq!(settings.ignored_apps.len(), 2);
        assert_eq!(settings.ignored_apps[0].executable_name, "Notepad.exe");
        assert!(settings.ignored_apps[0].id.starts_with("exe:"));
    }
}

fn default_device_id() -> String {
    let now = now_ms();
    let pid = std::process::id();
    format!("clipdeck-{now:x}-{pid:x}")
}

fn default_device_name() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .ok()
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| "This device".into())
}

fn default_device_color() -> String {
    "#28b7e8".into()
}

fn default_pairing_code() -> String {
    let seed = now_ms().unsigned_abs() ^ u64::from(std::process::id());
    format!("{:06}", seed % 1_000_000)
}

/// Current unix timestamp in milliseconds.
pub fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_deserialization_merges_missing_defaults() {
        let settings: Settings = serde_json::from_value(serde_json::json!({
            "hotkey": "Alt+V",
            "ignoredApps": []
        }))
        .unwrap();
        assert_eq!(settings.hotkey, "Alt+V");
        assert_eq!(settings.file_filter_mode, FileFilterMode::Exclude);
        assert_eq!(
            settings.file_exclude_extensions,
            default_excluded_extensions()
        );
    }

    #[test]
    fn legacy_ignored_app_strings_migrate_to_stable_identities() {
        let settings: Settings = serde_json::from_value(serde_json::json!({
            "ignoredApps": ["C:/Apps/Editor.EXE", "browser.exe"]
        }))
        .unwrap();
        assert_eq!(settings.ignored_apps.len(), 2);
        assert_eq!(settings.ignored_apps[0].executable_name, "Editor.EXE");
        assert!(settings.ignored_apps[0].id.starts_with("exe:"));
        assert_eq!(settings.ignored_apps[1].executable_path, "");
    }

    #[test]
    fn ignored_app_wire_contract_is_camel_case() {
        let app = IgnoredApp::from_legacy("C:/Apps/Editor.exe");
        let value = serde_json::to_value(app).unwrap();
        assert!(value.get("displayName").is_some());
        assert!(value.get("executablePath").is_some());
        assert!(value.get("executableName").is_some());
    }
}
