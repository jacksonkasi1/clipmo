//! Deterministic Win32 style enforcement and reporting for the quick flyout.

use tauri::WebviewWindow;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{
    GetWindowLongPtrW, SetWindowLongPtrW, SetWindowPos, GWL_EXSTYLE, GWL_STYLE,
    SET_WINDOW_POS_FLAGS, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOOWNERZORDER,
    SWP_NOSIZE, SWP_NOZORDER, WS_CAPTION, WS_EX_APPWINDOW, WS_EX_TOOLWINDOW, WS_MAXIMIZEBOX,
    WS_MINIMIZEBOX, WS_SYSMENU, WS_THICKFRAME,
};

/// A single observation of the native window styles, used for both enforcement
/// and packaged smoke diagnostics.
#[derive(Clone, Copy, Debug)]
pub struct StyleSnapshot {
    pub hwnd: isize,
    pub style: isize,
    pub ex_style: isize,
}

impl std::fmt::Display for StyleSnapshot {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "hwnd=0x{:X} style=0x{:08X} exStyle=0x{:08X}",
            self.hwnd, self.style as u32, self.ex_style as u32
        )
    }
}

fn snapshot(hwnd: HWND) -> StyleSnapshot {
    unsafe {
        StyleSnapshot {
            hwnd: hwnd.0 as isize,
            style: GetWindowLongPtrW(hwnd, GWL_STYLE),
            ex_style: GetWindowLongPtrW(hwnd, GWL_EXSTYLE),
        }
    }
}

/// Reads the current native styles without changing them.
pub fn read_styles(window: &WebviewWindow) -> Result<StyleSnapshot, String> {
    let hwnd = window.hwnd().map_err(|error| error.to_string())?;
    Ok(snapshot(hwnd))
}

/// Removes application-window chrome after backdrop setup and forces Windows to
/// recalculate the non-client frame. Tauri's high-level setters are still used,
/// but WebView2/Acrylic can restore the original configured styles while the
/// hidden window warms up, so the final HWND is authoritative. Returns the
/// styles observed immediately after enforcement.
pub fn enforce_quick_flyout(window: &WebviewWindow) -> Result<StyleSnapshot, String> {
    // On Windows Tauri deliberately adds a white frame when shadow is enabled
    // for an undecorated window. Disable it both in configuration and here so a
    // hidden warm webview cannot restore it during Acrylic or focus changes.
    window
        .set_shadow(false)
        .map_err(|error| error.to_string())?;

    let hwnd = window.hwnd().map_err(|error| error.to_string())?;
    unsafe {
        let style = GetWindowLongPtrW(hwnd, GWL_STYLE);
        let forbidden =
            (WS_CAPTION | WS_THICKFRAME | WS_SYSMENU | WS_MINIMIZEBOX | WS_MAXIMIZEBOX).0;
        SetWindowLongPtrW(hwnd, GWL_STYLE, style & !(forbidden as isize));

        let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        SetWindowLongPtrW(
            hwnd,
            GWL_EXSTYLE,
            (ex_style & !(WS_EX_APPWINDOW.0 as isize)) | WS_EX_TOOLWINDOW.0 as isize,
        );

        SetWindowPos(
            hwnd,
            None,
            0,
            0,
            0,
            0,
            SET_WINDOW_POS_FLAGS(
                SWP_FRAMECHANGED.0
                    | SWP_NOMOVE.0
                    | SWP_NOSIZE.0
                    | SWP_NOZORDER.0
                    | SWP_NOOWNERZORDER.0
                    | SWP_NOACTIVATE.0,
            ),
        )
        .map_err(|error| error.to_string())?;
    }

    // `SWP_FRAMECHANGED` can reset DWM's border policy. Reapply it only after
    // the final frame calculation so the flyout keeps small rounded corners
    // without the white non-client edge.
    crate::platform::backdrop::apply_quick_frame(window);
    Ok(snapshot(hwnd))
}
