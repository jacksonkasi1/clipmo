//! Synthesises a Ctrl+V keystroke aimed at the previous foreground window.
//!
//! Flow:
//!
//! 1. The user hits Enter or double-clicks on an item. Before we steal focus,
//!    the hotkey/window handler stores the foreground HWND it observed.
//! 2. We release any modifier keys we might still be holding (otherwise the
//!    modifier state would arrive at the target as `Shift+Ctrl+V`).
//! 3. We hand focus back to the previous window, attach our input thread to
//!    its thread so the keystroke is delivered, then `SendInput` sequential
//!    events with micro-delays for Ctrl+V using virtual keys and hardware scan codes.
//!
//! All `unsafe` blocks are scoped to the calls that genuinely need them; the
//! rest of the function is just arithmetic and handle juggling.

use windows::Win32::Foundation::{HWND, POINT};
use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    MapVirtualKeyW, SendInput, INPUT, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, MAPVK_VK_TO_VSC,
    VIRTUAL_KEY, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT, VK_V,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AllowSetForegroundWindow, BringWindowToTop, GetForegroundWindow, GetWindowThreadProcessId,
    IsIconic, IsWindow, SetForegroundWindow, ShowWindow, ASFW_ANY, SW_RESTORE,
};

/// Sends Ctrl+V to the foreground window captured before the popup opened.
pub fn paste_to(target: isize) -> bool {
    paste_to_target(target).is_some()
}

/// Returns the actual receiving window, including when the saved target expired.
pub fn paste_to_target(target: isize) -> Option<isize> {
    unsafe {
        let target_hwnd = if target != 0 && IsWindow(Some(HWND(target as *mut _))).as_bool() {
            HWND(target as *mut _)
        } else {
            // If no previous target was recorded, wait briefly for the OS to restore focus
            // after Clipmo hides, and target the currently active foreground window.
            std::thread::sleep(std::time::Duration::from_millis(30));
            let fg = GetForegroundWindow();
            if fg.0.is_null() || !IsWindow(Some(fg)).as_bool() {
                return None;
            }
            fg
        };

        // Drop any modifiers the user may have been holding when they pressed
        // the hotkey so the synthetic V is a bare `Ctrl+V`.
        release_modifiers();

        // `AllowSetForegroundWindow` is required because, by design, a
        // background process cannot move focus.
        let _ = AllowSetForegroundWindow(ASFW_ANY);

        // Attach the input threads so the keystroke is delivered even if the
        // shell briefly intercepted the focus transition.
        let target_tid = window_thread_id(target_hwnd);
        let our_tid = current_thread_id();

        if target_tid != 0 && our_tid != 0 && target_tid != our_tid {
            let _ = AttachThreadInput(our_tid, target_tid, true);
        }

        // SW_RESTORE also turns a maximized window into a normal/half-sized
        // window. Only restore an actually minimized target.
        if IsIconic(target_hwnd).as_bool() {
            let _ = ShowWindow(target_hwnd, SW_RESTORE);
        }
        let _ = BringWindowToTop(target_hwnd);
        let requested = SetForegroundWindow(target_hwnd).as_bool();
        let focused = requested && wait_for_foreground(target_hwnd);

        // Give the target window a brief moment to settle focus and activate its edit control.
        std::thread::sleep(std::time::Duration::from_millis(25));

        let sent = focused && send_ctrl_v();

        if target_tid != 0 && our_tid != 0 && target_tid != our_tid {
            let _ = AttachThreadInput(our_tid, target_tid, false);
        }

        sent.then_some(target_hwnd.0 as isize)
    }
}

fn wait_for_foreground(target: HWND) -> bool {
    for _ in 0..20 {
        if unsafe { GetForegroundWindow() } == target {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    false
}

fn foreground_target() -> isize {
    unsafe { GetForegroundWindow().0 as isize }
}

pub fn is_foreground(target: isize) -> bool {
    target != 0 && foreground_target() == target
}

/// Continue a batch only in its original target, without restoring focus.
pub fn paste_if_foreground(target: isize) -> bool {
    is_foreground(target) && send_ctrl_v()
}

unsafe fn release_modifiers() {
    let modifiers = [VK_CONTROL, VK_SHIFT, VK_MENU, VK_LWIN, VK_RWIN];
    for vk in modifiers {
        if is_key_down(vk) {
            send_key_event(vk, true);
        }
    }
}

unsafe fn is_key_down(vk: VIRTUAL_KEY) -> bool {
    use windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
    (GetAsyncKeyState(vk.0 as i32) as u16 & 0x8000) != 0
}

/// Sends a single key event (up or down) for the given virtual-key code,
/// including its hardware scan code for full target application compatibility.
fn send_key_event(vk: VIRTUAL_KEY, up: bool) -> bool {
    let scan_code = unsafe { MapVirtualKeyW(vk.0 as u32, MAPVK_VK_TO_VSC) as u16 };
    let mut flags = windows::Win32::UI::Input::KeyboardAndMouse::KEYBD_EVENT_FLAGS(0);
    if up {
        flags |= KEYEVENTF_KEYUP;
    }

    let input = INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: scan_code,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };

    unsafe { SendInput(&[input], std::mem::size_of::<INPUT>() as i32) == 1 }
}

/// Sends Ctrl+V via `SendInput` with sequential timing delays so the target
/// application's message loop reliably registers the Ctrl modifier state.
fn send_ctrl_v() -> bool {
    // 1. Press Ctrl
    let mut sent = send_key_event(VK_CONTROL, false);
    std::thread::sleep(std::time::Duration::from_millis(15));

    // 2. Press V
    sent &= send_key_event(VK_V, false);
    std::thread::sleep(std::time::Duration::from_millis(15));

    // 3. Release V
    sent &= send_key_event(VK_V, true);
    std::thread::sleep(std::time::Duration::from_millis(15));

    // 4. Release Ctrl
    sent &= send_key_event(VK_CONTROL, true);

    sent
}

fn window_thread_id(hwnd: HWND) -> u32 {
    unsafe { GetWindowThreadProcessId(hwnd, None) }
}

fn current_thread_id() -> u32 {
    unsafe { GetCurrentThreadId() }
}

/// Resets the mouse cursor to (0, 0) of the target window. Unused for now but
/// kept because the next iteration of paste may need to focus a control before
/// sending the keystroke.
#[allow(dead_code)]
pub fn focus_caret(target: isize) {
    if target == 0 {
        return;
    }
    let hwnd = HWND(target as *mut _);
    unsafe {
        let _ = SetForegroundWindow(hwnd);
        // The caret position would be retrieved via GetGUIThreadInfo; out of
        // scope for the v1 paste path.
        let _ = POINT { x: 0, y: 0 };
    }
}

#[allow(dead_code)]
fn _ensure_foreground_imported() -> Option<HWND> {
    unsafe { Some(GetForegroundWindow()) }
}
