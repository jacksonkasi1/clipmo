use crate::error::{Error, Result};
use crate::models::{ClipItem, ItemKind, PasteFlavor, StoredFileStatus};
use base64::Engine;

fn file_paths(item: &ClipItem) -> Vec<String> {
    item.files
        .iter()
        .map(|original| {
            item.file_assets
                .iter()
                .find(|asset| {
                    asset.original_path == *original && asset.status == StoredFileStatus::Ready
                })
                .and_then(|asset| asset.stored_path.clone())
                .unwrap_or_else(|| original.clone())
        })
        .collect()
}

fn plain(item: &ClipItem) -> String {
    match item.kind {
        ItemKind::Files => file_paths(item).join("\n"),
        ItemKind::Image => item
            .image
            .as_ref()
            .map(|image| image.path.clone())
            .unwrap_or_else(|| item.content.clone()),
        _ => item.content.clone(),
    }
}

pub fn put_back_on_clipboard(
    item: &ClipItem,
    flavor: PasteFlavor,
    html: Option<&str>,
    rtf: Option<&str>,
) -> Result<()> {
    let payload = if flavor == PasteFlavor::PlainText {
        serde_json::json!({ "text": plain(item) })
    } else {
        match item.kind {
            ItemKind::Files => serde_json::json!({ "files": file_paths(item) }),
            ItemKind::Image => {
                let image = item
                    .image
                    .as_ref()
                    .ok_or_else(|| Error::Clipboard("image asset is missing".into()))?;
                let decoded =
                    image::open(&image.path).map_err(|e| Error::Clipboard(e.to_string()))?;
                let mut png = std::io::Cursor::new(Vec::new());
                decoded
                    .write_to(&mut png, image::ImageFormat::Png)
                    .map_err(|e| Error::Clipboard(e.to_string()))?;
                serde_json::json!({ "image": base64::engine::general_purpose::STANDARD.encode(png.into_inner()) })
            }
            _ => {
                let mut payload = serde_json::json!({ "text": item.content });
                if let Some(html) = html {
                    payload["html"] = html.into();
                }
                if let Some(rtf) = rtf {
                    payload["rtf"] = rtf.into();
                }
                payload
            }
        }
    };
    crate::platform::write(&payload)
}

pub fn put_multiple_back_on_clipboard(items: &[ClipItem], flavor: PasteFlavor) -> Result<()> {
    if items.is_empty() {
        return Ok(());
    }
    if items.len() == 1 {
        return put_back_on_clipboard(&items[0], flavor, None, None);
    }
    if flavor != PasteFlavor::PlainText
        && items
            .iter()
            .all(|item| matches!(item.kind, ItemKind::Files | ItemKind::Image))
    {
        let mut paths = Vec::new();
        for item in items {
            if item.kind == ItemKind::Image {
                paths.push(
                    item.image
                        .as_ref()
                        .ok_or_else(|| Error::Clipboard("image asset is missing".into()))?
                        .path
                        .clone(),
                );
            } else {
                paths.extend(file_paths(item));
            }
        }
        return crate::platform::write(&serde_json::json!({ "files": paths }));
    }
    crate::platform::write(
        &serde_json::json!({ "text": items.iter().map(plain).collect::<Vec<_>>().join("\n") }),
    )
}
