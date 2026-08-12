//! Cached application-icon extraction that never creates a visible console.

use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use sha2::{Digest, Sha256};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub fn extract(executable_path: &str) -> Option<String> {
    let executable = Path::new(executable_path);
    if !executable.is_file() {
        return None;
    }

    if let Some(cached) = cached(executable_path) {
        return Some(cached);
    }

    let cache_root = cache_root()?;
    std::fs::create_dir_all(&cache_root).ok()?;
    let icon_path = cache_path(executable, &cache_root);

    let script = r#"Add-Type -AssemblyName System.Drawing; $icon = [System.Drawing.Icon]::ExtractAssociatedIcon($args[0]); if ($null -eq $icon) { exit 2 }; $bitmap = $icon.ToBitmap(); $bitmap.Save($args[1], [System.Drawing.Imaging.ImageFormat]::Png); $bitmap.Dispose(); $icon.Dispose()"#;
    let mut child = Command::new("powershell.exe")
        .creation_flags(CREATE_NO_WINDOW)
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .arg(executable)
        .arg(&icon_path)
        .spawn()
        .ok()?;

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    let status = loop {
        if let Some(status) = child.try_wait().ok()? {
            break status;
        }
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    };

    (status.success() && icon_path.is_file()).then(|| icon_path.to_string_lossy().into_owned())
}

/// Returns an already generated icon without launching an external process.
/// Automatic history/sidebar refreshes must use this non-blocking path.
pub fn cached(executable_path: &str) -> Option<String> {
    let executable = Path::new(executable_path);
    let icon_path = cache_path(executable, &cache_root()?);
    icon_path
        .is_file()
        .then(|| icon_path.to_string_lossy().into_owned())
}

fn cache_root() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .map(|path| path.join("Clipdeck").join("icon-cache"))
}

fn cache_path(executable: &Path, cache_root: &Path) -> PathBuf {
    let digest = Sha256::digest(super::source::normalize_path(executable).as_bytes());
    cache_root.join(format!("{digest:x}.png"))
}
