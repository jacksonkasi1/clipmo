//! Reads the clipboard formats we care about from the Win32 clipboard.
//!
//! The Win32 clipboard is a synchronised shared resource. Each format read
//! follows the same recipe:
//!
//! 1. `OpenClipboard` (idempotent here because the listener is the only
//!    opener, but we still retry on `ERROR_ACCESS_DENIED`).
//! 2. `GetClipboardData(format)` returns an `HGLOBAL`. We lock it, copy the
//!    bytes out, and unlock before the next read.
//! 3. `CloseClipboard` exactly once.
//!
//! Lock contention with the source app is rare but real: the source app has
//! already finished writing by the time `WM_CLIPBOARDUPDATE` is dispatched, so
//! the retry is mostly belt-and-braces.

use std::path::PathBuf;
use std::sync::OnceLock;

use windows::Win32::Foundation::HGLOBAL;
use windows::Win32::System::DataExchange::{
    CloseClipboard, GetClipboardData, IsClipboardFormatAvailable, OpenClipboard,
    RegisterClipboardFormatW,
};
use windows::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};

use crate::platform::{from_wide, pcwstr, to_wide};

/// Standard clipboard format codes not exported by `windows` crate constants
/// that we still need.
pub const CF_UNICODETEXT: u32 = 13;
pub const CF_HDROP: u32 = 15;
pub const CF_DIB: u32 = 8;
pub const CF_DIBV5: u32 = 17;
pub const CF_HTML: &str = "HTML Format";
pub const CF_RTF: &str = "Rich Text Format";
pub const EXCLUDE_FROM_MONITOR: &str = "ExcludeClipboardContentFromMonitorProcessing";
pub const CAN_INCLUDE_IN_HISTORY: &str = "CanIncludeInClipboardHistory";
/// Private marker attached to writes restored by Clipdeck. Its presence suppresses
/// exactly that clipboard transaction instead of hiding unrelated copies in a timeout.
pub const CLIPDECK_INTERNAL_WRITE: &str = "app.clipdeck.desktop.InternalWrite.v1";

/// Pre-resolved handles to the registered clipboard formats so we only do the
/// `RegisterClipboardFormatW` lookup once per process.
pub struct Formats {
    pub html: u32,
    pub rtf: u32,
    pub exclude: u32,
    pub can_include: u32,
    pub internal_write: u32,
}

/// Consistent view of all supported formats captured during one clipboard
/// lock. Reading the formats together avoids mixed entries when another app
/// changes the clipboard between separate `OpenClipboard` calls.
pub struct ClipboardSnapshot {
    pub text: Option<String>,
    pub html: Option<String>,
    pub rtf: Option<String>,
    pub files: Vec<PathBuf>,
    pub image: Option<Vec<u8>>,
}

struct RawSnapshot {
    text: Option<String>,
    html: Option<String>,
    rtf: Option<String>,
    files: Vec<PathBuf>,
    image_dib: Option<Vec<u8>>,
}

impl Formats {
    pub fn register() -> Self {
        Self {
            html: register(CF_HTML),
            rtf: register(CF_RTF),
            exclude: register(EXCLUDE_FROM_MONITOR),
            can_include: register(CAN_INCLUDE_IN_HISTORY),
            internal_write: register(CLIPDECK_INTERNAL_WRITE),
        }
    }
}

fn register(name: &str) -> u32 {
    let wide = to_wide(name);
    unsafe { RegisterClipboardFormatW(pcwstr(&wide)) }
}

/// Indicates whether a clipboard change carries the "do not log this" flag
/// set by password managers and other sensitive apps.
pub fn is_sensitive(formats: &Formats) -> bool {
    with_clipboard(|| Some(is_sensitive_open(formats))).unwrap_or(false)
}

fn should_suppress_snapshot(sensitive: bool, internal_write: bool) -> bool {
    sensitive || internal_write
}

#[cfg(test)]
mod suppression_tests {
    use super::should_suppress_snapshot;

    #[test]
    fn self_write_marker_suppresses_only_marked_transactions() {
        assert!(should_suppress_snapshot(false, true));
        assert!(should_suppress_snapshot(true, false));
        assert!(!should_suppress_snapshot(false, false));
    }
}

/// Reads every supported clipboard flavor under one short-lived lock.
pub fn read_snapshot(formats: &Formats) -> Option<ClipboardSnapshot> {
    let raw = with_clipboard(|| {
        let internal_write = formats.internal_write != 0
            && unsafe { IsClipboardFormatAvailable(formats.internal_write) }.is_ok();
        if should_suppress_snapshot(is_sensitive_open(formats), internal_write) {
            return None;
        }

        Some(RawSnapshot {
            text: read_text_open(),
            html: read_html_open(formats.html),
            rtf: read_rtf_open(formats.rtf),
            files: read_files_open(),
            image_dib: read_image_bytes_open(),
        })
    })?;

    // DIB conversion is intentionally outside the clipboard lock. PNG
    // encoding can take milliseconds for screenshots and must never prevent
    // another application from copying.
    let image = raw.image_dib.as_deref().and_then(dib_to_png);

    Some(ClipboardSnapshot {
        text: raw.text,
        html: raw.html,
        rtf: raw.rtf,
        files: raw.files,
        image,
    })
}

/// Returns the visible Unicode text on the clipboard, or an empty string if
/// there is none.
pub fn read_text() -> Option<String> {
    with_clipboard(read_text_open)
}

/// Reads the CF_HTML format and returns just the inner fragment, stripping the
/// pre/post HTML envelope. Many apps (including Chromium-based ones) leave a
/// full document in CF_HTML and only the fragment is interesting.
pub fn read_html() -> Option<String> {
    let fmt = get_html_format()?;
    with_clipboard(|| read_html_open(fmt))
}

fn get_html_format() -> Option<u32> {
    static FORMATS: OnceLock<Formats> = OnceLock::new();
    Some(FORMATS.get_or_init(Formats::register).html)
}

fn get_rtf_format() -> Option<u32> {
    static FORMATS: OnceLock<Formats> = OnceLock::new();
    Some(FORMATS.get_or_init(Formats::register).rtf)
}

/// Returns the raw RTF bytes as a UTF-8 string with non-ASCII replaced.
///
/// RTF is a 7-bit format; everything beyond that is control data, so the
/// lossy conversion is safe.
pub fn read_rtf() -> Option<String> {
    let fmt = get_rtf_format()?;
    with_clipboard(|| read_rtf_open(fmt))
}

/// Reads CF_HDROP into a vector of absolute file paths.
pub fn read_files() -> Vec<PathBuf> {
    with_clipboard(|| Some(read_files_open())).unwrap_or_default()
}

/// Reads CF_DIB or CF_DIBV5 and returns the bitmap as a PNG byte buffer.
///
/// We prefer the simpler CF_DIB form when present; CF_DIBV5 carries a few
/// extra metadata fields that the PNG encoder does not need.
pub fn read_image() -> Option<Vec<u8>> {
    let bytes = with_clipboard(read_image_bytes_open)?;
    dib_to_png(&bytes)
}

/// Wraps `OpenClipboard` with a small retry loop because a pasting app can
/// briefly hold the clipboard exclusively.
fn with_clipboard<R>(f: impl FnOnce() -> Option<R>) -> Option<R> {
    const RETRY_DELAYS_MS: [u64; 7] = [0, 2, 4, 8, 16, 32, 50];
    for delay in RETRY_DELAYS_MS {
        if unsafe { OpenClipboard(None) }.is_ok() {
            let result = f();
            unsafe {
                let _ = CloseClipboard();
            }
            return result;
        }
        if delay == 0 {
            std::thread::yield_now();
        } else {
            std::thread::sleep(std::time::Duration::from_millis(delay));
        }
    }
    None
}

fn is_sensitive_open(formats: &Formats) -> bool {
    unsafe {
        if IsClipboardFormatAvailable(formats.exclude).is_ok() {
            return true;
        }

        if IsClipboardFormatAvailable(formats.can_include).is_ok() {
            let value = GetClipboardData(formats.can_include)
                .ok()
                .and_then(lock_handle)
                .and_then(|bytes| bytes.first().copied());
            return matches!(value, Some(0));
        }
    }
    false
}

fn read_text_open() -> Option<String> {
    if unsafe { IsClipboardFormatAvailable(CF_UNICODETEXT) }.is_err() {
        return None;
    }
    let handle = unsafe { GetClipboardData(CF_UNICODETEXT) }.ok()?;
    let bytes = lock_handle(handle)?;
    let end = bytes
        .chunks_exact(2)
        .position(|chunk| chunk == [0, 0])
        .map(|index| index * 2)
        .unwrap_or(bytes.len());
    let wide: Vec<u16> = bytes[..end]
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .collect();
    Some(String::from_utf16_lossy(&wide))
}

fn read_html_open(format: u32) -> Option<String> {
    if unsafe { IsClipboardFormatAvailable(format) }.is_err() {
        return None;
    }
    let handle = unsafe { GetClipboardData(format) }.ok()?;
    let bytes = lock_handle(handle)?;
    let bytes = bytes.split(|byte| *byte == 0).next().unwrap_or_default();
    let raw = String::from_utf8_lossy(bytes).into_owned();
    extract_fragment(&raw).or(Some(raw))
}

fn read_rtf_open(format: u32) -> Option<String> {
    if unsafe { IsClipboardFormatAvailable(format) }.is_err() {
        return None;
    }
    let handle = unsafe { GetClipboardData(format) }.ok()?;
    let bytes = lock_handle(handle)?;
    let bytes = bytes.split(|byte| *byte == 0).next().unwrap_or_default();
    Some(String::from_utf8_lossy(bytes).into_owned())
}

fn read_files_open() -> Vec<PathBuf> {
    use windows::Win32::UI::Shell::{DragQueryFileW, HDROP};

    if unsafe { IsClipboardFormatAvailable(CF_HDROP) }.is_err() {
        return Vec::new();
    }
    let Some(handle) = (unsafe { GetClipboardData(CF_HDROP) }).ok() else {
        return Vec::new();
    };
    let hdrop = HDROP(handle.0);
    unsafe {
        let count = DragQueryFileW(hdrop, 0xFFFF_FFFF, None);
        let mut paths = Vec::with_capacity(count as usize);
        for index in 0..count {
            let required = DragQueryFileW(hdrop, index, None);
            if required == 0 {
                continue;
            }
            let mut buffer = vec![0u16; required as usize + 1];
            let length = DragQueryFileW(hdrop, index, Some(&mut buffer));
            if length > 0 {
                paths.push(PathBuf::from(from_wide(&buffer[..length as usize])));
            }
        }
        paths
    }
}

fn read_image_bytes_open() -> Option<Vec<u8>> {
    let format = if unsafe { IsClipboardFormatAvailable(CF_DIBV5) }.is_ok() {
        CF_DIBV5
    } else if unsafe { IsClipboardFormatAvailable(CF_DIB) }.is_ok() {
        CF_DIB
    } else {
        return None;
    };
    let handle = unsafe { GetClipboardData(format) }.ok()?;
    lock_handle(handle)
}

/// Locks an `HANDLE` returned by `GetClipboardData` and returns its bytes as a `Vec<u8>`.
fn lock_handle(handle: windows::Win32::Foundation::HANDLE) -> Option<Vec<u8>> {
    // Cast `HANDLE` to `HGLOBAL` since the clipboard always returns GMEM-movable
    // memory blocks for the formats we care about.
    let hglobal = HGLOBAL(handle.0);
    lock_hglobal(hglobal)
}

/// Locks an `HGLOBAL` and returns its bytes as a `Vec<u8>`.
fn lock_hglobal(handle: HGLOBAL) -> Option<Vec<u8>> {
    unsafe {
        let ptr = GlobalLock(handle);
        if ptr.is_null() {
            return None;
        }
        let size = GlobalSize(handle);
        let bytes = std::slice::from_raw_parts(ptr as *const u8, size).to_vec();
        let _ = GlobalUnlock(handle);
        Some(bytes)
    }
}

/// Extracts the `<!--StartFragment-->...<!--EndFragment-->` body from a
/// CF_HTML byte stream. Falls back to the full body when the markers are
/// absent, which is the case for older producers.
fn extract_fragment(raw: &str) -> Option<String> {
    let start_marker = "<!--StartFragment-->";
    let end_marker = "<!--EndFragment-->";
    let start = raw.find(start_marker)? + start_marker.len();
    let end = raw.find(end_marker)?;
    if end < start {
        return None;
    }
    Some(raw[start..end].to_string())
}

/// Decodes a DIB into a PNG byte stream.
fn dib_to_png(bytes: &[u8]) -> Option<Vec<u8>> {
    use image::{ImageBuffer, RgbaImage};

    if bytes.len() < std::mem::size_of::<windows::Win32::Graphics::Gdi::BITMAPINFOHEADER>() {
        return None;
    }

    let header_size = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    if header_size < 40 || header_size as usize > bytes.len() {
        return None;
    }
    let width = i32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
    let height = i32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
    let planes = u16::from_le_bytes([bytes[12], bytes[13]]);
    let bpp = u16::from_le_bytes([bytes[14], bytes[15]]);
    let compression = u32::from_le_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);

    if planes != 1
        || width <= 0
        || height == 0
        || !matches!(bpp, 24 | 32)
        || !matches!(compression, 0 | 3)
    {
        return None;
    }

    let mask_bytes = if header_size == 40 && compression == 3 {
        12_usize
    } else {
        0
    };
    let pixel_offset = (header_size as usize).checked_add(mask_bytes)?;
    let src_w = width as u32;
    // Positive height = bottom-up rows, the common case for screenshots.
    let (top_down, src_h) = if height < 0 {
        (true, height.unsigned_abs())
    } else {
        (false, height as u32)
    };

    let bytes_per_pixel = u32::from(bpp) / 8;
    let row_bytes = bytes_per_pixel.checked_mul(src_w)?;
    let stride = row_bytes.checked_add(3)? & !3; // each row is 4-byte aligned
    let pixels_size = usize::try_from(stride.checked_mul(src_h)?).ok()?;
    let pixel_end = pixel_offset.checked_add(pixels_size)?;
    if bytes.len() < pixel_end {
        return None;
    }
    let pixels = &bytes[pixel_offset..pixel_end];

    let mut img: RgbaImage = ImageBuffer::new(src_w, src_h);
    let mut has_nonzero_alpha = bpp == 24;
    for y in 0..src_h {
        let row = &pixels[(y * stride) as usize..];
        let target_y = if top_down { y } else { src_h - 1 - y };
        for x in 0..src_w {
            let px = (x * (bpp as u32 / 8)) as usize;
            let r = row[px + 2];
            let g = row[px + 1];
            let b = row[px];
            let a = if bpp == 32 { row[px + 3] } else { 255 };
            has_nonzero_alpha |= a != 0;
            img.put_pixel(x, target_y, image::Rgba([r, g, b, a]));
        }
    }
    // Many CF_DIB producers leave the unused BI_RGB alpha byte at zero. Such
    // images are opaque, not transparent.
    if bpp == 32 && compression == 0 && !has_nonzero_alpha {
        for pixel in img.pixels_mut() {
            pixel[3] = 255;
        }
    }

    let mut out = Vec::new();
    let mut cursor = std::io::Cursor::new(&mut out);
    img.write_to(&mut cursor, image::ImageFormat::Png).ok()?;
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dib(width: i32, height: i32, bpp: u16, pixels: &[u8]) -> Vec<u8> {
        let mut bytes = vec![0_u8; 40];
        bytes[0..4].copy_from_slice(&40_u32.to_le_bytes());
        bytes[4..8].copy_from_slice(&width.to_le_bytes());
        bytes[8..12].copy_from_slice(&height.to_le_bytes());
        bytes[12..14].copy_from_slice(&1_u16.to_le_bytes());
        bytes[14..16].copy_from_slice(&bpp.to_le_bytes());
        bytes.extend_from_slice(pixels);
        bytes
    }

    #[test]
    fn dib_conversion_flips_bottom_up_rows() {
        // Bottom row blue, top row red; each 24-bit row is padded to 4 bytes.
        let bytes = dib(1, 2, 24, &[255, 0, 0, 0, 0, 0, 255, 0]);
        let png = dib_to_png(&bytes).unwrap();
        let image = image::load_from_memory(&png).unwrap().to_rgba8();
        assert_eq!(image.get_pixel(0, 0).0, [255, 0, 0, 255]);
        assert_eq!(image.get_pixel(0, 1).0, [0, 0, 255, 255]);
    }

    #[test]
    fn zero_alpha_bi_rgb_is_treated_as_opaque() {
        let bytes = dib(1, 1, 32, &[30, 20, 10, 0]);
        let png = dib_to_png(&bytes).unwrap();
        let image = image::load_from_memory(&png).unwrap().to_rgba8();
        assert_eq!(image.get_pixel(0, 0).0, [10, 20, 30, 255]);
    }

    #[test]
    fn malformed_or_unsupported_dibs_are_rejected() {
        assert!(dib_to_png(&[0; 8]).is_none());
        assert!(dib_to_png(&dib(0, 1, 32, &[0; 4])).is_none());
        assert!(dib_to_png(&dib(1, 1, 16, &[0; 4])).is_none());
    }

    #[test]
    fn html_fragment_extraction_is_exact() {
        let raw = "header<!--StartFragment--><b>copy</b><!--EndFragment-->tail";
        assert_eq!(extract_fragment(raw).as_deref(), Some("<b>copy</b>"));
    }
}
