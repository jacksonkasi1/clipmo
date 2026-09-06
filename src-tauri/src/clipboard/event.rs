use crate::models::{ItemKind, SourceApp};

/// The single user-supplied callback that receives each parsed clipboard
/// change. Implemented as a trait object so this module does not need to
/// know about Tauri, the DB, or the frontend.
pub trait CaptureSink: Send + Sync + 'static {
    fn handle(&self, event: ClipEvent);
}

/// A new entry the UI should add to its history.
#[derive(Debug)]
pub struct ClipEvent {
    pub kind: ItemKind,
    pub preview: String,
    pub content: String,
    pub html: Option<String>,
    pub rtf: Option<String>,
    /// PNG bytes captured during the clipboard notification. Keeping these on
    /// the event avoids reopening the clipboard after another app has changed it.
    pub image_bytes: Option<Vec<u8>>,
    pub files: Vec<String>,
    pub size_bytes: i64,
    pub source: Option<SourceApp>,
    pub content_hash: String,
}
