//! Best-effort Windows application discovery for the ignored-app picker.

use std::collections::{BTreeMap, BTreeSet};
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use crate::models::{ApplicationInfo, IgnoredApp};

const CACHE_TTL: Duration = Duration::from_secs(30);
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
/// Timestamped snapshot of the installed-application scan.
type InstalledSnapshot = Option<(Instant, Vec<ApplicationInfo>)>;

static INSTALLED_CACHE: OnceLock<Mutex<InstalledSnapshot>> = OnceLock::new();

pub fn resolve(executable_path: &str) -> Option<IgnoredApp> {
    let path = PathBuf::from(executable_path.trim().trim_matches('"'));
    if path.as_os_str().is_empty() {
        return None;
    }
    Some(identity_for_path(&path))
}

pub fn running() -> Vec<ApplicationInfo> {
    let system = sysinfo::System::new_all();
    let visible_processes = visible_window_processes();
    let mut apps = BTreeMap::new();
    for (pid, process) in system.processes() {
        if !visible_processes.contains(&pid.as_u32()) {
            continue;
        }
        let Some(path) = process.exe() else { continue };
        if path.as_os_str().is_empty() || !path.exists() {
            continue;
        }
        let identity = identity_for_path(path);
        apps.entry(identity.id.clone()).or_insert(ApplicationInfo {
            identity,
            publisher: None,
            running: true,
            installed: false,
            recently_used: None,
        });
    }
    apps.into_values().collect()
}

pub fn installed(refresh: bool) -> Vec<ApplicationInfo> {
    let cache = INSTALLED_CACHE.get_or_init(|| Mutex::new(None));
    if !refresh {
        if let Ok(guard) = cache.lock() {
            if let Some((created, apps)) = guard.as_ref() {
                if created.elapsed() < CACHE_TTL {
                    return apps.clone();
                }
            }
        }
    }

    let mut apps = BTreeMap::new();
    discover_uninstall_registry(&mut apps);
    discover_start_menu_executables(&mut apps);
    let result: Vec<_> = apps.into_values().collect();
    if let Ok(mut guard) = cache.lock() {
        *guard = Some((Instant::now(), result.clone()));
    }
    result
}

pub fn extract_icon(executable_path: &str) -> Option<String> {
    use sha2::{Digest, Sha256};

    let executable = Path::new(executable_path);
    if !executable.is_file() {
        return None;
    }
    let cache_root = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)?
        .join("Clipdeck")
        .join("icon-cache");
    std::fs::create_dir_all(&cache_root).ok()?;
    let digest = Sha256::digest(super::source::normalize_path(executable).as_bytes());
    let icon_path = cache_root.join(format!("{digest:x}.png"));
    if icon_path.is_file() {
        return Some(icon_path.to_string_lossy().into_owned());
    }

    let script = r#"Add-Type -AssemblyName System.Drawing; $icon = [System.Drawing.Icon]::ExtractAssociatedIcon($args[0]); if ($null -eq $icon) { exit 2 }; $bitmap = $icon.ToBitmap(); $bitmap.Save($args[1], [System.Drawing.Imaging.ImageFormat]::Png); $bitmap.Dispose(); $icon.Dispose()"#;
    let status = hidden_powershell(script)
        .arg(executable)
        .arg(&icon_path)
        .status()
        .ok()?;
    (status.success() && icon_path.is_file()).then(|| icon_path.to_string_lossy().into_owned())
}

fn identity_for_path(path: &Path) -> IgnoredApp {
    let normalized = super::source::normalize_path(path);
    let executable_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_string();
    let display_name = path
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or(&executable_name)
        .to_string();
    IgnoredApp {
        id: format!("exe:{normalized}"),
        display_name,
        executable_path: path.to_string_lossy().into_owned(),
        executable_name,
        app_user_model_id: None,
        package_family_name: None,
        icon_path: None,
    }
}

fn insert_path(
    apps: &mut BTreeMap<String, ApplicationInfo>,
    path: PathBuf,
    display_name: Option<String>,
    publisher: Option<String>,
) {
    if !path.is_file()
        || path
            .extension()
            .is_none_or(|ext| !ext.eq_ignore_ascii_case("exe"))
        || is_helper_executable(&path)
    {
        return;
    }
    let mut identity = identity_for_path(&path);
    if let Some(name) = display_name.filter(|name| !name.trim().is_empty()) {
        identity.display_name = name;
    }
    apps.entry(identity.id.clone())
        .and_modify(|app| {
            app.installed = true;
            if publisher.is_some() {
                app.publisher = publisher.clone();
            }
            if !identity.display_name.is_empty() {
                app.identity.display_name = identity.display_name.clone();
            }
        })
        .or_insert(ApplicationInfo {
            identity,
            publisher,
            running: false,
            installed: true,
            recently_used: None,
        });
}

fn discover_uninstall_registry(apps: &mut BTreeMap<String, ApplicationInfo>) {
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
    use winreg::RegKey;

    const KEYS: [&str; 2] = [
        r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
        r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall",
    ];
    for hive in [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE] {
        let root = RegKey::predef(hive);
        for key_name in KEYS {
            let Ok(key) = root.open_subkey(key_name) else {
                continue;
            };
            for child_name in key.enum_keys().flatten() {
                let Ok(child) = key.open_subkey(child_name) else {
                    continue;
                };
                let display_name: Option<String> = child.get_value("DisplayName").ok();
                let publisher: Option<String> = child.get_value("Publisher").ok();
                let Some(raw): Option<String> = child.get_value("DisplayIcon").ok() else {
                    continue;
                };
                let Some(path) = display_icon_executable(&raw) else {
                    continue;
                };
                insert_path(apps, path, display_name, publisher);
            }
        }
    }
}

fn display_icon_executable(raw: &str) -> Option<PathBuf> {
    let value = raw.trim();
    let path = if let Some(quoted) = value.strip_prefix('"') {
        quoted.split('"').next().unwrap_or_default()
    } else {
        value.split(',').next().unwrap_or_default().trim()
    };
    (!path.is_empty()).then(|| PathBuf::from(path))
}

fn discover_start_menu_executables(apps: &mut BTreeMap<String, ApplicationInfo>) {
    let script = r#"$shell = New-Object -ComObject WScript.Shell; Get-ChildItem @($env:APPDATA + '\Microsoft\Windows\Start Menu\Programs', $env:PROGRAMDATA + '\Microsoft\Windows\Start Menu\Programs') -Filter *.lnk -Recurse -ErrorAction SilentlyContinue | ForEach-Object { $shortcut = $shell.CreateShortcut($_.FullName); if ($shortcut.TargetPath -like '*.exe') { [pscustomobject]@{ Name = $_.BaseName; Path = $shortcut.TargetPath } } } | ConvertTo-Json -Compress"#;
    if let Ok(output) = hidden_powershell(script).output() {
        if output.status.success() {
            if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&output.stdout) {
                let entries: Vec<&serde_json::Value> = match &value {
                    serde_json::Value::Array(values) => values.iter().collect(),
                    serde_json::Value::Object(_) => vec![&value],
                    _ => Vec::new(),
                };
                for entry in entries {
                    if let Some(path) = entry.get("Path").and_then(serde_json::Value::as_str) {
                        insert_path(
                            apps,
                            PathBuf::from(path),
                            entry
                                .get("Name")
                                .and_then(serde_json::Value::as_str)
                                .map(str::to_string),
                            None,
                        );
                    }
                }
            }
        }
    }
}

fn hidden_powershell(script: &str) -> Command {
    let mut command = Command::new("powershell.exe");
    command.creation_flags(CREATE_NO_WINDOW).args([
        "-NoProfile",
        "-NonInteractive",
        "-Command",
        script,
    ]);
    command
}

fn is_helper_executable(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    name.starts_with("unins")
        || matches!(
            name.as_str(),
            "setup.exe"
                | "uninstall.exe"
                | "uninstaller.exe"
                | "update.exe"
                | "updater.exe"
                | "installer.exe"
                | "crashpad_handler.exe"
                | "squirrel.exe"
                | "maintenancetool.exe"
        )
}

fn visible_window_processes() -> BTreeSet<u32> {
    use windows::core::BOOL;
    use windows::Win32::Foundation::{HWND, LPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowTextLengthW, GetWindowThreadProcessId, IsWindowVisible,
    };

    unsafe extern "system" fn collect(hwnd: HWND, parameter: LPARAM) -> BOOL {
        if IsWindowVisible(hwnd).as_bool() && GetWindowTextLengthW(hwnd) > 0 {
            let processes = &mut *(parameter.0 as *mut BTreeSet<u32>);
            let mut pid = 0;
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
            if pid != 0 {
                processes.insert(pid);
            }
        }
        BOOL(1)
    }

    let mut processes = BTreeSet::new();
    let parameter =
        windows::Win32::Foundation::LPARAM((&mut processes as *mut BTreeSet<u32>) as isize);
    let _ = unsafe { EnumWindows(Some(collect), parameter) };
    processes
}
