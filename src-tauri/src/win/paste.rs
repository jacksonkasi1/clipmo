//! Synthesises a Ctrl+V keystroke aimed at the previous foreground window.
//!
//! Flow:
//!
//! 1. The user hits Enter on an item. Before we steal focus, the hotkey handler
//!    stores the foreground HWND it observed.
//! 2. We release any modifier keys we might still be holding (otherwise the
//!    modifier state would arrive at the target as `Shift+Ctrl+V`).
//! 3. We hand focus back to the previous window, attach our input thread to
//!    its thread so the keystroke is delivered, then `SendInput` a single
//!    press/release pair for the V key using scan codes (the synthetic flag
//!    that lets some apps distinguish macro paste is left at 0).
//!
//! All `unsafe` blocks are scoped to the calls that genuinely need them; the
//! rest of the function is just arithmetic and handle juggling.

use windows::Win32::Foundation::{HWND, POINT};
use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, KEYEVENTF_SCANCODE, VIRTUAL_KEY,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AllowSetForegroundWindow, BringWindowToTop, GetForegroundWindow, GetWindowThreadProcessId,
    IsIconic, IsWindow, SetForegroundWindow, ShowWindow, ASFW_ANY, SW_RESTORE,
};

/// Sends Ctrl+V to the foreground window captured before the popup opened.
pub fn paste_to(target: isize) -> bool {
    if target == 0 {
        return false;
    }
    let target = HWND(target as *mut _);

    unsafe {
        if !IsWindow(Some(target)).as_bool() {
            return false;
        }

        // Drop any modifiers the user may have been holding when they pressed
        // the hotkey so the synthetic V is a bare `Ctrl+V`.
        release_modifiers();

        // `AllowSetForegroundWindow` is required because, by design, a
        // background process cannot move focus.
        let _ = AllowSetForegroundWindow(ASFW_ANY);

        // Attach the input threads so the keystroke is delivered even if the
        // shell briefly intercepted the focus transition.
        let target_tid = window_thread_id(target);
        let our_tid = current_thread_id();

        if target_tid != 0 && our_tid != 0 && target_tid != our_tid {
            let _ = AttachThreadInput(our_tid, target_tid, true);
        }

        // SW_RESTORE also turns a maximized window into a normal/half-sized
        // window. Only restore an actually minimized target.
        if IsIconic(target).as_bool() {
            let _ = ShowWindow(target, SW_RESTORE);
        }
        let _ = BringWindowToTop(target);
        let requested = SetForegroundWindow(target).as_bool();
        let focused = requested && wait_for_foreground(target);
        let sent = focused && send_ctrl_v();

        if target_tid != 0 && our_tid != 0 && target_tid != our_tid {
            let _ = AttachThreadInput(our_tid, target_tid, false);
        }

        sent
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

unsafe fn release_modifiers() {
    let modifiers = [
        VIRTUAL_KEY(0x11), // VK_CONTROL
        VIRTUAL_KEY(0x10), // VK_SHIFT
        VIRTUAL_KEY(0x12), // VK_MENU (Alt)
        VIRTUAL_KEY(0x5B), // VK_LWIN
        VIRTUAL_KEY(0x5C), // VK_RWIN
    ];
    for vk in modifiers {
        if is_key_down(vk) {
            send_key(vk, true);
        }
    }
}

unsafe fn is_key_down(vk: VIRTUAL_KEY) -> bool {
    use windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
    (GetAsyncKeyState(vk.0 as i32) as u16 & 0x8000) != 0
}

/// Sends a single key event (up or down) for the given virtual-key code.
///
/// Used to release modifier keys (Ctrl/Shift/Alt/Win) before we push the real
/// `Ctrl+V`, so the target doesn't receive stray chord input.
fn send_key(vk: VIRTUAL_KEY, up: bool) {
    let mut flags = windows::Win32::UI::Input::KeyboardAndMouse::KEYBD_EVENT_FLAGS(0);
    if up {
        flags |= KEYEVENTF_KEYUP;
    }

    let input = INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };

    unsafe {
        SendInput(&[input], std::mem::size_of::<INPUT>() as i32);
    }
}

/// Sends Ctrl+V via `SendInput`. Scan codes are used so the keystroke reaches
/// apps that have ScanCodeMap overrides.
fn send_ctrl_v() -> bool {
    let ctrl = 0x1D;
    let v = 0x2F;

    let inputs = [
        build_input(ctrl, false),
        build_input(v, false),
        build_input(v, true),
        build_input(ctrl, true),
    ];

    unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) == inputs.len() as u32 }
}

fn build_input(scan_code: u16, up: bool) -> INPUT {
    let mut flags = KEYEVENTF_SCANCODE;
    if up {
        flags |= KEYEVENTF_KEYUP;
    }

    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(0),
                wScan: scan_code,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
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
