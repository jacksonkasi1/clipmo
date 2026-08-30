//! Writes persisted Clipdeck entries back to the native Windows clipboard.
//!
//! Every payload builder is pure and unit-tested. The small Win32 boundary
//! owns one clipboard session and guarantees `CloseClipboard` through `Drop`.

use crate::error::{Error, Result};
use crate::models::{ClipItem, ItemKind, PasteFlavor, StoredFileStatus};

pub fn put_back_on_clipboard(
    item: &ClipItem,
    flavor: PasteFlavor,
    html: Option<&str>,
    rtf: Option<&str>,
) -> Result<()> {
    let _session = ClipboardSession::open()?;
    unsafe {
        windows::Win32::System::DataExchange::EmptyClipboard()
            .map_err(|error| Error::Clipboard(format!("EmptyClipboard: {error}")))?;
    }
    // Mark this exact clipboard transaction as internal before publishing any
    // user format. The listener checks the private format while reading the same
    // transaction, so no broad time window can suppress another application's copy.
    write_registered_bytes(super::formats::CLIPDECK_INTERNAL_WRITE, b"1")?;

    if flavor == PasteFlavor::PlainText {
        let plain = match item.kind {
            ItemKind::Image => item
                .image
                .as_ref()
                .map(|image| image.path.as_str())
                .unwrap_or_default()
                .to_string(),
            ItemKind::Files => copy_file_paths(item).join("\r\n"),
            _ => item.content.clone(),
        };
        return write_unicode_text(&plain);
    }

    match item.kind {
        ItemKind::Image => {
            let image = item
                .image
                .as_ref()
                .ok_or_else(|| Error::Other("image asset is missing".into()))?;
            write_image(&image.path)
        }
        ItemKind::Files => write_file_list(&copy_file_paths(item)),
        _ => {
            write_unicode_text(&item.content)?;
            if let Some(html) = html {
                write_html(html)?;
            }
            if let Some(rtf) = rtf {
                write_registered_text(super::formats::CF_RTF, rtf)?;
            }
            Ok(())
        }
    }
}

pub fn put_multiple_back_on_clipboard(items: &[ClipItem], flavor: PasteFlavor) -> Result<()> {
    if items.is_empty() {
        return Ok(());
    }
    if items.len() == 1 {
        return put_back_on_clipboard(&items[0], flavor, None, None);
    }

    let _session = ClipboardSession::open()?;
    unsafe {
        windows::Win32::System::DataExchange::EmptyClipboard()
            .map_err(|error| Error::Clipboard(format!("EmptyClipboard: {error}")))?;
    }
    write_registered_bytes(super::formats::CLIPDECK_INTERNAL_WRITE, b"1")?;

    let all_assets = items
        .iter()
        .all(|item| matches!(item.kind, ItemKind::Files | ItemKind::Image));
    if all_assets && flavor != PasteFlavor::PlainText {
        let all_paths = copy_asset_paths(items)?;
        if !all_paths.is_empty() {
            write_file_list(&all_paths)?;
            // Image selections are files when copied as a group. Do not also
            // advertise their storage paths as text to chat apps/editors.
            if items.iter().all(|item| item.kind == ItemKind::Files) {
                let _ = write_unicode_text(&all_paths.join("\r\n"));
            }
            return Ok(());
        }
    }

    let combined = build_combined_text_payload(items);
    write_unicode_text(&combined)
}

fn copy_asset_paths(items: &[ClipItem]) -> Result<Vec<String>> {
    let mut paths = Vec::new();
    for item in items {
        if item.kind == ItemKind::Image {
            let image = item
                .image
                .as_ref()
                .ok_or_else(|| Error::Other("image asset is missing".into()))?;
            paths.push(image.path.clone());
        } else {
            paths.extend(copy_file_paths(item));
        }
    }
    Ok(paths)
}

pub fn build_combined_text_payload(items: &[ClipItem]) -> String {
    let mut entries = Vec::new();
    for item in items {
        let text = match item.kind {
            ItemKind::Files => copy_file_paths(item).join("\r\n"),
            ItemKind::Image => item
                .image
                .as_ref()
                .map(|img| img.path.clone())
                .unwrap_or_else(|| item.content.clone()),
            _ => {
                if !item.content.is_empty() {
                    item.content.clone()
                } else {
                    item.preview.clone()
                }
            }
        };
        if !text.is_empty() {
            entries.push(text);
        }
    }
    entries.join("\r\n")
}

struct ClipboardSession;

impl ClipboardSession {
    fn open() -> Result<Self> {
        use windows::Win32::System::DataExchange::OpenClipboard;

        const RETRY_DELAYS_MS: [u64; 7] = [0, 2, 4, 8, 16, 32, 50];
        for delay in RETRY_DELAYS_MS {
            if unsafe { OpenClipboard(None) }.is_ok() {
                return Ok(Self);
            }
            if delay == 0 {
                std::thread::yield_now();
            } else {
                std::thread::sleep(std::time::Duration::from_millis(delay));
            }
        }
        Err(Error::Clipboard("OpenClipboard failed".into()))
    }
}

impl Drop for ClipboardSession {
    fn drop(&mut self) {
        unsafe {
            let _ = windows::Win32::System::DataExchange::CloseClipboard();
        }
    }
}

fn copy_file_paths(item: &ClipItem) -> Vec<String> {
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

fn write_unicode_text(text: &str) -> Result<()> {
    let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let bytes = unsafe {
        std::slice::from_raw_parts(
            wide.as_ptr().cast::<u8>(),
            wide.len() * std::mem::size_of::<u16>(),
        )
    };
    write_clipboard_bytes(super::formats::CF_UNICODETEXT, bytes)
}

fn write_file_list(paths: &[String]) -> Result<()> {
    let bytes = build_file_drop_bytes(paths)?;
    write_clipboard_bytes(super::formats::CF_HDROP, &bytes)
}

fn build_file_drop_bytes(paths: &[String]) -> Result<Vec<u8>> {
    const DROPFILES_HEADER_BYTES: usize = 20;
    if paths.is_empty() {
        return Err(Error::Other("file clipboard entry has no paths".into()));
    }

    let mut wide_paths: Vec<u16> = Vec::new();
    for path in paths {
        wide_paths.extend(path.encode_utf16());
        wide_paths.push(0);
    }
    wide_paths.push(0);

    let mut bytes = vec![0_u8; DROPFILES_HEADER_BYTES];
    bytes[0..4].copy_from_slice(&(DROPFILES_HEADER_BYTES as u32).to_le_bytes());
    bytes[16..20].copy_from_slice(&1_u32.to_le_bytes());
    bytes.extend(unsafe {
        std::slice::from_raw_parts(
            wide_paths.as_ptr().cast::<u8>(),
            wide_paths.len() * std::mem::size_of::<u16>(),
        )
    });
    Ok(bytes)
}

fn write_image(path: &str) -> Result<()> {
    let image = image::open(path)?.to_rgba8();
    let dib = image_to_dib(&image)?;
    write_clipboard_bytes(super::formats::CF_DIB, &dib)
}

fn image_to_dib(image: &image::RgbaImage) -> Result<Vec<u8>> {
    let width = image.width();
    let height = image.height();
    if width == 0 || height == 0 || width > i32::MAX as u32 || height > i32::MAX as u32 {
        return Err(Error::Other("image dimensions are invalid".into()));
    }

    const BITMAP_INFO_HEADER_BYTES: usize = 40;
    let pixel_bytes = width
        .checked_mul(height)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| Error::Other("image is too large for the clipboard".into()))?;
    let mut dib = vec![0_u8; BITMAP_INFO_HEADER_BYTES];
    dib[0..4].copy_from_slice(&(BITMAP_INFO_HEADER_BYTES as u32).to_le_bytes());
    dib[4..8].copy_from_slice(&(width as i32).to_le_bytes());
    dib[8..12].copy_from_slice(&(height as i32).to_le_bytes());
    dib[12..14].copy_from_slice(&1_u16.to_le_bytes());
    dib[14..16].copy_from_slice(&32_u16.to_le_bytes());
    dib[20..24].copy_from_slice(&pixel_bytes.to_le_bytes());
    dib.reserve(pixel_bytes as usize);
    let raw = image.as_raw();
    let row_bytes = width as usize * 4;
    for y in (0..height).rev() {
        let row_start = y as usize * row_bytes;
        for pixel in raw[row_start..row_start + row_bytes].chunks_exact(4) {
            dib.extend_from_slice(&[pixel[2], pixel[1], pixel[0], pixel[3]]);
        }
    }
    Ok(dib)
}

fn write_html(fragment: &str) -> Result<()> {
    let payload = build_html_payload(fragment);
    write_registered_bytes(super::formats::CF_HTML, payload.as_bytes())
}

fn build_html_payload(fragment: &str) -> String {
    const PREFIX: &str = "<!DOCTYPE html><html><body><!--StartFragment-->";
    const SUFFIX: &str = "<!--EndFragment--></body></html>";
    const HEADER_TEMPLATE: &str =
        "Version:0.9\r\nStartHTML:0000000000\r\nEndHTML:0000000000\r\nStartFragment:0000000000\r\nEndFragment:0000000000\r\n";

    let start_html = HEADER_TEMPLATE.len();
    let start_fragment = start_html + PREFIX.len();
    let end_fragment = start_fragment + fragment.len();
    let end_html = end_fragment + SUFFIX.len();
    let header = format!(
        "Version:0.9\r\nStartHTML:{start_html:010}\r\nEndHTML:{end_html:010}\r\nStartFragment:{start_fragment:010}\r\nEndFragment:{end_fragment:010}\r\n"
    );
    debug_assert_eq!(header.len(), HEADER_TEMPLATE.len());
    format!("{header}{PREFIX}{fragment}{SUFFIX}\0")
}

fn write_registered_text(format_name: &str, value: &str) -> Result<()> {
    let mut bytes = value.trim_end_matches('\0').as_bytes().to_vec();
    bytes.push(0);
    write_registered_bytes(format_name, &bytes)
}

fn write_registered_bytes(format_name: &str, bytes: &[u8]) -> Result<()> {
    use windows::Win32::System::DataExchange::RegisterClipboardFormatW;

    let wide: Vec<u16> = format_name
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let format = unsafe { RegisterClipboardFormatW(windows::core::PCWSTR(wide.as_ptr())) };
    if format == 0 {
        return Err(Error::Clipboard(format!(
            "RegisterClipboardFormatW failed for {format_name}"
        )));
    }
    write_clipboard_bytes(format, bytes)
}

fn write_clipboard_bytes(format: u32, bytes: &[u8]) -> Result<()> {
    use windows::Win32::Foundation::{GlobalFree, HANDLE};
    use windows::Win32::System::DataExchange::SetClipboardData;
    use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};

    if bytes.is_empty() {
        return Err(Error::Clipboard("clipboard payload was empty".into()));
    }
    let allocation = unsafe { GlobalAlloc(GMEM_MOVEABLE, bytes.len()) }
        .map_err(|error| Error::Clipboard(format!("GlobalAlloc: {error}")))?;
    let pointer = unsafe { GlobalLock(allocation) };
    if pointer.is_null() {
        unsafe {
            let _ = GlobalFree(Some(allocation));
        }
        return Err(Error::Clipboard("GlobalLock failed".into()));
    }
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), pointer.cast::<u8>(), bytes.len());
        let _ = GlobalUnlock(allocation);
    }

    if unsafe { SetClipboardData(format, Some(HANDLE(allocation.0))) }.is_err() {
        unsafe {
            let _ = GlobalFree(Some(allocation));
        }
        return Err(Error::Clipboard(format!(
            "SetClipboardData failed for format {format}"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_drop_payload_has_dropfiles_header_and_utf16_paths() {
        let paths = vec![r"C:\Temp\one.txt".to_string(), r"D:\two.png".to_string()];
        let payload = build_file_drop_bytes(&paths).unwrap();
        assert_eq!(u32::from_le_bytes(payload[0..4].try_into().unwrap()), 20);
        assert_eq!(u32::from_le_bytes(payload[16..20].try_into().unwrap()), 1);
        let wide: Vec<u16> = payload[20..]
            .chunks_exact(2)
            .map(|bytes| u16::from_le_bytes([bytes[0], bytes[1]]))
            .collect();
        let decoded: Vec<String> = wide
            .split(|value| *value == 0)
            .filter(|part| !part.is_empty())
            .map(String::from_utf16_lossy)
            .collect();
        assert_eq!(decoded, paths);
        assert!(wide.ends_with(&[0, 0]));
    }

    #[test]
    fn image_dib_is_bottom_up_bgra() {
        let image = image::RgbaImage::from_fn(1, 2, |_x, y| {
            if y == 0 {
                image::Rgba([255, 0, 0, 255])
            } else {
                image::Rgba([0, 0, 255, 128])
            }
        });
        let dib = image_to_dib(&image).unwrap();
        assert_eq!(u32::from_le_bytes(dib[0..4].try_into().unwrap()), 40);
        assert_eq!(i32::from_le_bytes(dib[4..8].try_into().unwrap()), 1);
        assert_eq!(i32::from_le_bytes(dib[8..12].try_into().unwrap()), 2);
        assert_eq!(&dib[40..44], &[255, 0, 0, 128]);
        assert_eq!(&dib[44..48], &[0, 0, 255, 255]);
    }

    #[test]
    fn html_clipboard_offsets_select_the_exact_utf8_fragment() {
        let fragment = "<strong>naïve</strong>";
        let payload = build_html_payload(fragment);
        let read_offset = |label: &str| -> usize {
            let start = payload.find(label).unwrap() + label.len();
            payload[start..start + 10].parse().unwrap()
        };
        let start = read_offset("StartFragment:");
        let end = read_offset("EndFragment:");
        assert_eq!(&payload.as_bytes()[start..end], fragment.as_bytes());
    }

    fn text_item() -> ClipItem {
        ClipItem {
            id: 1,
            kind: ItemKind::Text,
            preview: "First".into(),
            content: "First content".into(),
            has_html: false,
            has_rtf: false,
            image: None,
            files: vec![],
            file_assets: vec![],
            size_bytes: 13,
            tags: vec![],
            source: None,
            favorite: false,
            copy_count: 1,
            device: crate::models::DeviceIdentity {
                id: "local".into(),
                name: "This device".into(),
                platform: crate::models::PlatformKind::Windows,
                color: "#000".into(),
            },
            sync_status: crate::models::SyncStatus::Local,
            first_copied_at: 1,
            last_copied_at: 1,
        }
    }

    #[test]
    fn combined_text_payload_joins_with_newlines() {
        let item1 = text_item();
        let item2 = ClipItem {
            id: 2,
            kind: ItemKind::Link,
            preview: "https://example.com".into(),
            content: "https://example.com".into(),
            ..item1.clone()
        };
        let combined = build_combined_text_payload(&[item1, item2]);
        assert_eq!(combined, "First content\r\nhttps://example.com");
    }

    #[test]
    fn copied_images_use_file_drop_paths_in_selection_order() {
        let items: Vec<_> = ["third.webp", "first.webp", "second.webp"]
            .iter()
            .map(|name| ClipItem {
                kind: ItemKind::Image,
                image: Some(crate::models::ImageMeta {
                    path: format!(r"C:\images\{name}"),
                    thumb_path: "unused-thumbnail.webp".into(),
                    width: 2,
                    height: 2,
                }),
                ..text_item()
            })
            .collect();
        let paths = copy_asset_paths(&items).unwrap();
        assert_eq!(
            paths,
            [
                r"C:\images\third.webp",
                r"C:\images\first.webp",
                r"C:\images\second.webp"
            ]
        );
        let payload = build_file_drop_bytes(&paths).unwrap();
        let wide: Vec<_> = payload[20..]
            .chunks_exact(2)
            .map(|bytes| u16::from_le_bytes([bytes[0], bytes[1]]))
            .collect();
        assert_eq!(
            String::from_utf16_lossy(&wide),
            format!("{}\0\0", paths.join("\0"))
        );
        // Explicit plain-text paste still offers paths, as before.
        assert_eq!(build_combined_text_payload(&items), paths.join("\r\n"));
    }

    #[test]
    fn missing_image_metadata_does_not_silently_omit_an_image() {
        let item = ClipItem {
            kind: ItemKind::Image,
            ..text_item()
        };
        assert!(copy_asset_paths(&[item]).is_err());
    }
}
