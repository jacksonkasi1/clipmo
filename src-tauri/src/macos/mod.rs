//! Native macOS services. The saved paste target is a PID, never a window pointer.
use std::ffi::{c_char, CStr, CString};

unsafe extern "C" {
    fn clipmo_free(value: *mut c_char);
    pub(super) fn clipmo_change_count(name: *const c_char) -> i64;
    pub(super) fn clipmo_read(name: *const c_char) -> *mut c_char;
    pub(super) fn clipmo_write(name: *const c_char, payload: *const c_char) -> i32;
    #[cfg(not(test))]
    fn clipmo_foreground() -> i32;
    #[cfg(not(test))]
    fn clipmo_accessibility(prompt: i32) -> i32;
    #[cfg(not(test))]
    fn clipmo_activate(pid: i32) -> i32;
    #[cfg(not(test))]
    fn clipmo_paste(pid: i32) -> i32;
    #[cfg(not(test))]
    fn clipmo_apps(installed: i32) -> *mut c_char;
    #[cfg(not(test))]
    fn clipmo_dark() -> i32;
    #[cfg(not(test))]
    fn clipmo_backdrop_snapshot(window: *mut std::ffi::c_void) -> *mut c_char;
}

/// The native bridge returns UTF-8 JSON with independent malloc ownership.
pub(super) unsafe fn take_json<T: serde::de::DeserializeOwned>(ptr: *mut c_char) -> Option<T> {
    if ptr.is_null() {
        return None;
    }
    let result = serde_json::from_slice(CStr::from_ptr(ptr).to_bytes()).ok();
    clipmo_free(ptr);
    result
}

pub(super) fn write(payload: &serde_json::Value) -> crate::error::Result<()> {
    let data = CString::new(payload.to_string())
        .map_err(|e| crate::error::Error::Clipboard(e.to_string()))?;
    if unsafe { clipmo_write(std::ptr::null(), data.as_ptr()) } == 0 {
        return Err(crate::error::Error::Clipboard(
            "macOS could not write the clipboard".into(),
        ));
    }
    Ok(())
}

#[cfg(not(test))]
pub mod source {
    pub fn current_foreground() -> isize {
        unsafe { super::clipmo_foreground() as isize }
    }
    pub fn foreground_paste_target() -> Option<isize> {
        let pid = current_foreground();
        (pid > 0 && pid != std::process::id() as isize).then_some(pid)
    }
    pub fn is_current_process(path: &str) -> bool {
        std::env::current_exe().is_ok_and(|own| own == std::path::Path::new(path))
    }
}

#[cfg(not(test))]
pub mod paste {
    pub fn ensure_permission() -> crate::error::Result<()> {
        if unsafe { super::clipmo_accessibility(1) } != 0 {
            return Ok(());
        }
        Err(crate::error::Error::Other("Allow Clipmo in System Settings → Privacy & Security → Accessibility, then retry. The item is still available to copy and paste manually with Command+V.".into()))
    }
    pub fn paste_to(target: isize) -> bool {
        paste_to_target(target).is_some()
    }
    pub fn paste_to_target(target: isize) -> Option<isize> {
        let target = if target > 0 {
            target
        } else {
            super::source::foreground_paste_target()?
        };
        if ensure_permission().is_err() || unsafe { super::clipmo_activate(target as i32) } == 0 {
            return None;
        }
        for _ in 0..30 {
            if is_foreground(target) {
                std::thread::sleep(std::time::Duration::from_millis(30));
                return paste_if_foreground(target).then_some(target);
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        None
    }
    pub fn is_foreground(target: isize) -> bool {
        target > 0
            && target != std::process::id() as isize
            && super::source::current_foreground() == target
    }
    pub fn paste_if_foreground(target: isize) -> bool {
        is_foreground(target) && unsafe { super::clipmo_paste(target as i32) != 0 }
    }
}

#[cfg(not(test))]
pub mod apps {
    use crate::models::{ApplicationInfo, IgnoredApp};
    pub fn running() -> Vec<ApplicationInfo> {
        unsafe { super::take_json(super::clipmo_apps(0)).unwrap_or_default() }
    }
    pub fn installed(_refresh: bool) -> Vec<ApplicationInfo> {
        unsafe { super::take_json(super::clipmo_apps(1)).unwrap_or_default() }
    }
    pub fn resolve(path: &str) -> Option<IgnoredApp> {
        installed(false)
            .into_iter()
            .find(|app| app.identity.executable_path == path)
            .map(|app| app.identity)
            .or_else(|| {
                std::path::Path::new(path)
                    .is_file()
                    .then(|| IgnoredApp::from_legacy(path))
            })
    }
}

#[cfg(not(test))]
pub mod icon {
    // Generic app glyphs remain available when no cached icon exists.
    pub fn cached(_path: &str) -> Option<String> {
        None
    }
    pub fn extract(_path: &str) -> Option<String> {
        None
    }
}

#[cfg(not(test))]
pub mod appearance {
    pub fn read() -> crate::models::SystemAppearance {
        crate::models::SystemAppearance {
            accent: "#007AFF".into(),
            dark: unsafe { super::clipmo_dark() != 0 },
        }
    }
}
#[cfg(not(test))]
pub mod backdrop {
    use crate::models::Backdrop;
    use tauri::Emitter;

    pub fn apply(window: &tauri::WebviewWindow, backdrop: Backdrop, _dark: bool) -> Backdrop {
        let target = window.clone();
        // AppKit requires the main thread, including changes from Settings IPC.
        // Clear first so theme/focus changes cannot stack visual effect views.
        if let Err(error) = window.run_on_main_thread(move || {
            use window_vibrancy::{
                apply_vibrancy, clear_vibrancy, NSVisualEffectMaterial, NSVisualEffectState,
            };
            let result = clear_vibrancy(&target).and_then(|_| {
                if backdrop == Backdrop::Solid {
                    Ok(())
                } else {
                    let material = if target.label() == "quick" {
                        NSVisualEffectMaterial::Popover
                    } else {
                        NSVisualEffectMaterial::Sidebar
                    };
                    apply_vibrancy(&target, material, Some(NSVisualEffectState::Active), None)
                }
            });
            let effective = match result {
                Ok(()) => backdrop,
                Err(error) => {
                    log::warn!("could not apply macOS vibrancy: {error}");
                    Backdrop::Solid
                }
            };
            let _ = target.emit("clipdeck:backdrop", effective);
            if let (Ok(base), Ok(native)) =
                (std::env::var("CLIPDECK_READY_FILE"), target.ns_window())
            {
                let snapshot: Option<serde_json::Value> =
                    unsafe { super::take_json(super::clipmo_backdrop_snapshot(native)) };
                if let Some(mut snapshot) = snapshot {
                    snapshot["processId"] = serde_json::json!(std::process::id());
                    snapshot["backdrop"] = serde_json::json!(effective);
                    let path = std::path::PathBuf::from(base)
                        .with_extension(format!("vibrancy.{}.json", target.label()));
                    let temporary = path.with_extension("tmp");
                    if std::fs::write(&temporary, snapshot.to_string()).is_ok() {
                        let _ = std::fs::rename(temporary, path);
                    }
                }
            }
        }) {
            log::warn!("could not schedule macOS vibrancy: {error}");
            let _ = window.emit("clipdeck:backdrop", Backdrop::Solid);
            return Backdrop::Solid;
        }
        backdrop
    }
}
#[cfg(not(test))]
pub mod monitor {
    pub struct WorkArea {
        pub x: i32,
        pub y: i32,
        pub width: i32,
        pub height: i32,
    }
    pub fn resolve(_foreground: isize) -> Option<WorkArea> {
        None
    }
}
