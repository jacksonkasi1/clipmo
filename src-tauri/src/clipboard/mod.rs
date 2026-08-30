//! Clipboard monitoring pipeline.
//!
//! The flow:
//!
//! 1. A dedicated OS thread is started at app launch. It owns a hidden
//!    top-level window with `WS_EX_TOOLWINDOW` so it never appears in Alt-Tab
//!    or on the taskbar. `AddClipboardFormatListener` makes the shell broadcast
//!    `WM_CLIPBOARDUPDATE` to us on every clipboard change.
//! 2. On each notification we read every clipboard format we care about and
//!    hand the result to the [`CaptureSink`], which then writes to the DB and
//!    forwards the change to the frontend.
//!
//! `HWND_MESSAGE` was deliberately avoided: message-only windows do not
//! receive `WM_CLIPBOARDUPDATE`, which the shell broadcasts to top-level
//! windows only.

mod classifier;
pub mod formats;
mod hasher;
pub mod listener;
pub mod paste_batch;
pub mod writer;

pub use classifier::classify;
pub use hasher::{hash_files, hash_image, hash_text};
pub use listener::{start_listener, CaptureSink, ClipEvent};

use crate::models::{ImageMeta, ItemKind, SourceApp};

/// The shape the sink receives for every clipboard change.
pub struct CapturedPayload {
    pub kind: ItemKind,
    pub preview: String,
    pub content: String,
    pub html: Option<String>,
    pub rtf: Option<String>,
    pub image: Option<ImageMeta>,
    pub files: Vec<String>,
    pub size_bytes: u64,
    pub source: Option<SourceApp>,
    pub content_hash: String,
}
