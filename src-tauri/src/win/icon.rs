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
    let status = Command::new("powershell.exe")
        .creation_flags(CREATE_NO_WINDOW)
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .arg(executable)
        .arg(&icon_path)
        .status()
        .ok()?;

    (status.success() && icon_path.is_file()).then(|| icon_path.to_string_lossy().into_owned())
}
