//! Listens for clipboard changes and dispatches parsed content to a sink.
//!
//! Architecture:
//!
//! * A dedicated OS thread creates a hidden top-level window with
//!   `WS_EX_TOOLWINDOW`. The window is 0×0, positioned at `(-32000, -32000)`
//!   (the standard "minimised off-screen" coordinates) and never made visible.
//! * `AddClipboardFormatListener` makes the shell broadcast
//!   `WM_CLIPBOARDUPDATE` to us on every clipboard change.
//! * The thread pumps a normal `GetMessageW`/`DispatchMessageW` loop.
//! * Each notification triggers a `ClipEvent` delivered to the user-supplied
//!   [`CaptureSink`].
//!
//! `HWND_MESSAGE` windows were ruled out by the Win32 documentation: message-
//! only windows do not receive broadcast messages, and `WM_CLIPBOARDUPDATE` is
//! exactly that.

use std::sync::mpsc::{self, SyncSender};
use std::sync::Arc;
use std::time::Duration;

use windows::core::PCWSTR;
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::DataExchange::{
    AddClipboardFormatListener, RemoveClipboardFormatListener,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, RegisterClassExW, SetWindowPos,
    TranslateMessage, GWLP_USERDATA, HWND_TOP, MSG, SWP_NOACTIVATE, SWP_NOZORDER, WINDOW_EX_STYLE,
    WINDOW_STYLE, WM_CLIPBOARDUPDATE, WM_DESTROY, WM_NCDESTROY, WNDCLASSEXW,
};

use crate::models::ItemKind;

use super::super::platform::source;
use super::formats::{self, Formats};
use super::hasher;

pub use super::event::{CaptureSink, ClipEvent};

/// Starts the listener thread and returns once the hidden window has been
/// registered with the shell.
pub fn start_listener(sink: Arc<dyn CaptureSink>) -> std::io::Result<()> {
    const EVENT_QUEUE_CAPACITY: usize = 32;
    let (event_tx, event_rx) = mpsc::sync_channel::<ClipEvent>(EVENT_QUEUE_CAPACITY);
    std::thread::Builder::new()
        .name("clipboard-persist".into())
        .spawn(move || {
            while let Ok(event) = event_rx.recv() {
                sink.handle(event);
            }
        })?;

    let (ready_tx, ready_rx) = mpsc::sync_channel::<std::result::Result<(), String>>(1);
    std::thread::Builder::new()
        .name("clipboard-listener".into())
        .spawn(move || {
            let failure_tx = ready_tx.clone();
            if let Err(err) = run(event_tx, ready_tx) {
                let _ = failure_tx.try_send(Err(err.to_string()));
                log::error!("clipboard listener terminated: {err}");
            }
        })?;

    match ready_rx.recv_timeout(Duration::from_secs(15)) {
        Ok(Ok(())) => Ok(()),
        Ok(Err(message)) => Err(std::io::Error::other(message)),
        Err(mpsc::RecvTimeoutError::Timeout) => Err(std::io::Error::new(
            std::io::ErrorKind::TimedOut,
            "clipboard listener did not become ready within 15 seconds",
        )),
        Err(mpsc::RecvTimeoutError::Disconnected) => Err(std::io::Error::other(
            "clipboard listener exited before reporting readiness",
        )),
    }
}

/// Result of `run` so we can log a specific failure rather than panic on the
/// listener thread (which would tear the process down silently).
type BoxResult<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

fn run(
    event_tx: SyncSender<ClipEvent>,
    ready_tx: SyncSender<std::result::Result<(), String>>,
) -> BoxResult<()> {
    unsafe {
        let class_name = to_wide("ClipdeckListener");
        let instance = GetModuleHandleW(None).map_err(|e| format!("GetModuleHandleW: {e}"))?;

        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(wndproc),
            hInstance: instance.into(),
            lpszClassName: PCWSTR(class_name.as_ptr()),
            ..Default::default()
        };
        let atom = RegisterClassExW(&wc);
        if atom == 0 {
            return Err("RegisterClassExW failed".into());
        }

        // WS_EX_TOOLWINDOW keeps the window off Alt-Tab; WS_EX_NOPARENTNOTIFY
        // prevents the broadcast from being forwarded as a parent notify.
        let ex = WINDOW_EX_STYLE(0x00000080 | 0x00000004);
        // WS_OVERLAPPED is the smallest valid top-level style; combined with a
        // 0×0 size and an off-screen position it is invisible but still a
        // top-level window — which is what `WM_CLIPBOARDUPDATE` requires.
        let style = WINDOW_STYLE(0x00000000);

        let title = to_wide("");
        let hwnd = CreateWindowExW(
            ex,
            PCWSTR(class_name.as_ptr()),
            PCWSTR(title.as_ptr()),
            style,
            -32000,
            -32000,
            0,
            0,
            None,
            None,
            Some(HINSTANCE(instance.0)),
            None,
        )
        .map_err(|e| format!("CreateWindowExW: {e}"))?;

        let _ = SetWindowPos(
            hwnd,
            Some(HWND_TOP),
            0,
            0,
            0,
            0,
            SWP_NOZORDER | SWP_NOACTIVATE,
        );

        AddClipboardFormatListener(hwnd).map_err(|e| format!("AddClipboardFormatListener: {e}"))?;

        // Stash the sink on the window so the wndproc can recover it via
        // GetWindowLongPtrW. We only have one listener, so a thread-local
        // would also work; using the HWND keeps the API symmetrical.
        set_user_data(hwnd, ListenerData { event_tx });
        ready_tx
            .send(Ok(()))
            .map_err(|_| "listener readiness receiver disconnected")?;

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        Ok(())
    }
}

#[derive(Clone)]
struct ListenerData {
    event_tx: SyncSender<ClipEvent>,
}

/// Window procedure. `WM_CLIPBOARDUPDATE` is the only custom message we
/// handle; everything else is forwarded to `DefWindowProcW`.
unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_CLIPBOARDUPDATE => {
            // Defensive: the user data is set immediately after CreateWindowExW
            // before the message loop starts, but a malformed subclass could
            // in principle clear it. Skip the event rather than panicking.
            if let Some(data) = get_user_data(hwnd) {
                let foreground = source::current_foreground();
                if let Some(event) = capture(foreground) {
                    if data.event_tx.try_send(event).is_err() {
                        log::warn!("clipboard persistence queue is full; capture was skipped");
                    }
                }
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            let _ = RemoveClipboardFormatListener(hwnd);
            windows::Win32::UI::WindowsAndMessaging::PostQuitMessage(0);
            LRESULT(0)
        }
        WM_NCDESTROY => {
            use windows::Win32::UI::WindowsAndMessaging::SetWindowLongPtrW;

            let raw = SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0) as *mut ListenerData;
            if !raw.is_null() {
                drop(Box::from_raw(raw));
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

/// Reads every clipboard format we care about and folds them into one event.
///
/// Returns `None` when the clipboard carries the sensitive-content flag or
/// when none of our formats produced any data, both of which mean the change
/// should be silently dropped.
fn capture(foreground_hint: isize) -> Option<ClipEvent> {
    let formats = Formats::register();
    let snapshot = formats::read_snapshot(&formats)?;
    let text = snapshot.text;
    let html = snapshot.html;
    let rtf = snapshot.rtf;
    let files = snapshot.files;
    let image = snapshot.image;

    // CF_HDROP is authoritative when present. Explorer and design tools may
    // also publish a decorative bitmap for a file selection; treating that as
    // the payload would lose the actual paths.
    let (kind, content, content_hash) = if !files.is_empty() {
        let display = files
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join("\n");
        let hash = hasher::hash_files(&files);
        (ItemKind::Files, display, hash)
    } else if let Some(bytes) = image.as_deref() {
        let hash = hasher::hash_image(bytes);
        (ItemKind::Image, String::new(), hash)
    } else {
        let text = text?;
        let hash = hasher::hash_text(&text);
        let kind = super::classify(&text);
        (kind, text, hash)
    };

    if content_hash.is_empty() {
        return None;
    }

    let preview = match kind {
        ItemKind::Image => {
            // The actual pixel dimensions are determined later, in the
            // preview writer, because the bytes have not been decoded yet.
            "Image".to_string()
        }
        ItemKind::Files => {
            let first = files
                .first()
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().to_string());
            let more = files.len().saturating_sub(1);
            match (first, more) {
                (Some(name), 0) => name,
                (Some(name), n) => format!("{name} + {n} more"),
                (None, _) => format!("{} files", files.len()),
            }
        }
        _ => content
            .lines()
            .find(|line| !line.trim().is_empty())
            .unwrap_or("")
            .chars()
            .take(140)
            .collect(),
    };

    let source = source::resolve(Some(foreground_hint));

    let file_strings = files
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    let size_bytes = content.len().min(i64::MAX as usize) as i64;

    Some(ClipEvent {
        kind,
        preview,
        content,
        html,
        rtf,
        image_bytes: (kind == ItemKind::Image).then_some(image).flatten(),
        files: file_strings,
        size_bytes,
        source,
        content_hash,
    })
}

// ---- window/user-data plumbing -------------------------------------------

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Stash per-window data on the GWLP_USERDATA slot.
fn set_user_data(hwnd: HWND, data: ListenerData) {
    use windows::Win32::UI::WindowsAndMessaging::SetWindowLongPtrW;
    let boxed = Box::new(data);
    let raw = Box::into_raw(boxed);
    unsafe {
        SetWindowLongPtrW(
            hwnd,
            windows::Win32::UI::WindowsAndMessaging::GWLP_USERDATA,
            raw as isize,
        );
    }
}

fn get_user_data(hwnd: HWND) -> Option<ListenerData> {
    unsafe {
        use windows::Win32::UI::WindowsAndMessaging::GetWindowLongPtrW;
        let raw = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut ListenerData;
        if raw.is_null() {
            None
        } else {
            Some((*raw).clone())
        }
    }
}
