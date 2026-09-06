//! Poll the native pasteboard generation, persisting on a separate bounded queue.
pub use super::event::{CaptureSink, ClipEvent};
use crate::{
    models::{ItemKind, SourceApp},
    platform,
};
use base64::Engine;
use std::{path::PathBuf, sync::Arc, time::Duration};

#[derive(Default, serde::Deserialize)]
pub(crate) struct Snapshot {
    pub text: Option<String>,
    pub html: Option<String>,
    pub rtf: Option<String>,
    pub image: Option<String>,
    #[serde(default)]
    pub files: Vec<PathBuf>,
    pub source: Option<SourceApp>,
}

pub fn start_listener(sink: Arc<dyn CaptureSink>) -> std::io::Result<()> {
    let (tx, rx) = std::sync::mpsc::sync_channel(32);
    std::thread::Builder::new()
        .name("clipboard-persist".into())
        .spawn(move || {
            while let Ok(event) = rx.recv() {
                sink.handle(event);
            }
        })?;
    std::thread::Builder::new()
        .name("clipboard-listener".into())
        .spawn(move || {
            // Do not import stale clipboard contents on startup.
            let mut last = unsafe { platform::clipmo_change_count(std::ptr::null()) };
            loop {
                std::thread::sleep(Duration::from_millis(200));
                let revision = unsafe { platform::clipmo_change_count(std::ptr::null()) };
                if revision == last {
                    continue;
                }
                last = revision;
                let snapshot: Option<Snapshot> =
                    unsafe { platform::take_json(platform::clipmo_read(std::ptr::null())) };
                if let Some(event) = snapshot.and_then(capture) {
                    if tx.try_send(event).is_err() {
                        log::warn!("clipboard persistence queue is full; capture was skipped");
                    }
                }
            }
        })?;
    Ok(())
}

pub(crate) fn capture(snapshot: Snapshot) -> Option<ClipEvent> {
    let image = snapshot
        .image
        .as_deref()
        .and_then(|value| base64::engine::general_purpose::STANDARD.decode(value).ok());
    let (kind, content, content_hash) = if !snapshot.files.is_empty() {
        (
            ItemKind::Files,
            snapshot
                .files
                .iter()
                .map(|p| p.to_string_lossy())
                .collect::<Vec<_>>()
                .join("\n"),
            super::hash_files(&snapshot.files),
        )
    } else if let Some(bytes) = image.as_deref() {
        (ItemKind::Image, String::new(), super::hash_image(bytes))
    } else {
        let text = snapshot.text.filter(|s| !s.is_empty())?;
        (
            super::classify(&text),
            text.clone(),
            super::hash_text(&text),
        )
    };
    let preview = if kind == ItemKind::Image {
        "Image".into()
    } else {
        content.chars().take(140).collect()
    };
    Some(ClipEvent {
        kind,
        preview,
        size_bytes: image.as_ref().map_or(content.len(), Vec::len) as i64,
        content,
        content_hash,
        html: snapshot.html,
        rtf: snapshot.rtf,
        image_bytes: (kind == ItemKind::Image).then_some(image).flatten(),
        files: snapshot
            .files
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect(),
        source: snapshot.source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn finder_files_take_precedence_over_decorative_images() {
        let event = capture(Snapshot {
            files: vec![PathBuf::from("/tmp/example.txt")],
            image: Some("aWNvbg==".into()),
            text: Some("example.txt".into()),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(event.kind, ItemKind::Files);
        assert!(event.image_bytes.is_none());
    }
    #[test]
    fn captures_unicode_rich_text_and_ignores_empty_payloads() {
        let event = capture(Snapshot {
            text: Some("Hello 🌍".into()),
            html: Some("<b>Hello 🌍</b>".into()),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(event.content_hash, super::super::hash_text("Hello 🌍"));
        assert!(event.html.is_some());
        assert!(capture(Snapshot::default()).is_none());
    }
}
