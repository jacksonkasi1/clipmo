//! Managed content storage for image and file snapshots.
//!
//! Clipdeck only writes beneath a marked storage root. Clipboard source paths
//! are read-only inputs: they are never moved, renamed, or deleted.

use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::Component;
#[cfg(windows)]
use std::path::Prefix;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::models::{StoredFile, StoredFileStatus};

pub const MARKER_FILE: &str = ".clipdeck-storage";

pub fn paths_overlap(left: &Path, right: &Path) -> io::Result<bool> {
    let left = canonical_or_normalized(left)?;
    let right = canonical_or_normalized(right)?;
    Ok(path_starts_with_case_insensitive(&left, &right)
        || path_starts_with_case_insensitive(&right, &left))
}

fn canonical_or_normalized(path: &Path) -> io::Result<PathBuf> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    let mut normalized = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "path escapes its filesystem root",
                    ));
                }
            }
            Component::Prefix(_) | Component::RootDir | Component::Normal(_) => {
                normalized.push(component.as_os_str());
            }
        }
    }

    // A destination may not exist yet. Canonicalize its nearest existing
    // ancestor so short names, junctions, and verbatim prefixes are resolved
    // consistently on both sides of an overlap comparison.
    let mut candidate = normalized.as_path();
    let mut suffix: Vec<OsString> = Vec::new();
    loop {
        if let Ok(mut resolved) = fs::canonicalize(candidate) {
            for component in suffix.iter().rev() {
                resolved.push(component);
            }
            return Ok(strip_windows_verbatim_prefix(&resolved));
        }
        let Some(name) = candidate.file_name() else {
            break;
        };
        suffix.push(name.to_os_string());
        let Some(parent) = candidate.parent() else {
            break;
        };
        candidate = parent;
    }

    Ok(strip_windows_verbatim_prefix(&normalized))
}

#[cfg(windows)]
fn strip_windows_verbatim_prefix(path: &Path) -> PathBuf {
    let mut components = path.components();
    let Some(Component::Prefix(prefix)) = components.next() else {
        return path.to_path_buf();
    };
    let mut normalized = match prefix.kind() {
        Prefix::VerbatimDisk(drive) => PathBuf::from(format!("{}:", char::from(drive))),
        Prefix::VerbatimUNC(server, share) => PathBuf::from(r"\\").join(server).join(share),
        _ => return path.to_path_buf(),
    };
    normalized.extend(components);
    normalized
}

#[cfg(not(windows))]
fn strip_windows_verbatim_prefix(path: &Path) -> PathBuf {
    path.to_path_buf()
}

fn path_starts_with_case_insensitive(path: &Path, base: &Path) -> bool {
    let path: Vec<String> = path
        .components()
        .map(|part| part.as_os_str().to_string_lossy().to_lowercase())
        .collect();
    let base: Vec<String> = base
        .components()
        .map(|part| part.as_os_str().to_string_lossy().to_lowercase())
        .collect();
    path.len() >= base.len() && path.iter().zip(base.iter()).all(|(a, b)| a == b)
}

pub fn prepare_root(root: &Path) -> Result<()> {
    fs::create_dir_all(root)?;
    let marker = root.join(MARKER_FILE);
    if !marker.is_file() {
        let has_legacy_database = root.join("clipdeck.db").is_file();
        let collides_with_existing_folder = ["images", "thumbs", "files"]
            .iter()
            .any(|folder| root.join(folder).exists());
        if collides_with_existing_folder && !has_legacy_database {
            return Err(Error::Other(
                "storage folder already contains reserved Clipmo directories".into(),
            ));
        }
        fs::write(&marker, b"Clipdeck managed storage\n")?;
    }
    for folder in ["images", "thumbs", "files"] {
        fs::create_dir_all(root.join(folder))?;
    }
    Ok(())
}

pub fn image_root(root: &Path) -> PathBuf {
    root.join("images")
}

pub fn thumb_root(root: &Path) -> PathBuf {
    root.join("thumbs")
}

pub fn file_root(root: &Path) -> PathBuf {
    root.join("files")
}

/// Directories exposed through Tauri's asset protocol. The storage marker and
/// any neighboring application data remain outside the webview scope.
pub fn managed_asset_roots(root: &Path) -> [PathBuf; 3] {
    [image_root(root), thumb_root(root), file_root(root)]
}

/// Recognised raster image extensions. Anything else is treated as a generic
/// file and never receives a generated thumbnail.
const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "gif", "bmp", "webp"];

/// True when the path's extension is a supported raster image format.
pub fn is_image_path(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| {
            IMAGE_EXTENSIONS
                .iter()
                .any(|known| known.eq_ignore_ascii_case(ext))
        })
        .unwrap_or(false)
}

/// Generates a downscaled PNG preview for an image file. Returns `None` for
/// non-image files, unreadable sources, or decoders that fail. The thumbnail
/// is written under `thumb_root` keyed by the source's content hash so the
/// file can be deduplicated across captures.
pub fn generate_image_thumbnail(
    storage_root: &Path,
    source: &Path,
    hash: &str,
) -> Result<Option<PathBuf>> {
    if !is_image_path(source) {
        return Ok(None);
    }
    let Ok(bytes) = fs::read(source) else {
        return Ok(None);
    };
    let extension = source
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
        .unwrap_or_else(|| "png".to_string());
    let Some(format) = image::ImageFormat::from_extension(&extension) else {
        return Ok(None);
    };
    let Ok(loaded) = image::load_from_memory_with_format(&bytes, format) else {
        return Ok(None);
    };
    let thumbnail = loaded.thumbnail(256, 256);
    let thumb_dir = thumb_root(storage_root);
    prepare_root(storage_root)?;
    fs::create_dir_all(&thumb_dir)?;
    let thumb_path = thumb_dir.join(format!("{hash}.png"));
    thumbnail.save(&thumb_path)?;
    Ok(Some(thumb_path))
}

/// Copies clipboard file/folder inputs into one hash-addressed snapshot group.
/// Each item reports its own result so one inaccessible file does not discard
/// the rest of the clipboard entry.
pub fn snapshot_files(
    storage_root: &Path,
    hash: &str,
    originals: &[String],
    max_bytes: u64,
) -> Result<Vec<StoredFile>> {
    if hash.len() < 16 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(Error::Other("invalid snapshot content hash".into()));
    }
    prepare_root(storage_root)?;
    let group = file_root(storage_root).join(hash);
    fs::create_dir_all(&group)?;

    let mut used_bytes = 0_u64;
    let mut assets = Vec::with_capacity(originals.len());
    for (index, original) in originals.iter().enumerate() {
        let source = PathBuf::from(original);
        let metadata = match fs::symlink_metadata(&source) {
            Ok(metadata) => metadata,
            Err(error) => {
                assets.push(failed_asset(original, false, error.to_string()));
                continue;
            }
        };
        if metadata.file_type().is_symlink() {
            assets.push(skipped_asset(
                original,
                metadata.is_dir(),
                "Symbolic links are not copied",
            ));
            continue;
        }

        let is_directory = metadata.is_dir();
        let item_size = match measured_size(&source, 0) {
            Ok(size) => size,
            Err(error) => {
                assets.push(failed_asset(original, is_directory, error.to_string()));
                continue;
            }
        };
        if used_bytes.saturating_add(item_size) > max_bytes {
            assets.push(skipped_asset(
                original,
                is_directory,
                "Snapshot size limit exceeded",
            ));
            continue;
        }

        let name = source
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| "clipboard-item".to_string());
        let destination = group.join(format!("{index:03}-{name}"));
        remove_existing_destination(&destination)?;
        let copy_result = if is_directory {
            copy_directory(&source, &destination, 0)
        } else {
            copy_file_verified(&source, &destination)
        };
        match copy_result {
            Ok(()) => {
                used_bytes = used_bytes.saturating_add(item_size);
                // Image files get a managed thumbnail so the Quick View row and
                // the details preview can show a preview without re-decoding the
                // original every render. Generation failures are non-fatal:
                // the asset is still Ready, the thumbnail simply stays `None`.
                let thumb_path = if !is_directory {
                    match generate_image_thumbnail(storage_root, &source, hash) {
                        Ok(Some(path)) => Some(path.to_string_lossy().into_owned()),
                        _ => None,
                    }
                } else {
                    None
                };
                assets.push(StoredFile {
                    original_path: original.clone(),
                    stored_path: Some(destination.to_string_lossy().into_owned()),
                    size_bytes: item_size,
                    is_directory,
                    status: StoredFileStatus::Ready,
                    message: None,
                    thumb_path,
                });
            }
            Err(error) => {
                let _ = remove_existing_destination(&destination);
                assets.push(failed_asset(original, is_directory, error.to_string()));
            }
        }
    }
    Ok(assets)
}

fn measured_size(path: &Path, depth: usize) -> io::Result<u64> {
    if depth > 64 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Folder nesting is too deep",
        ));
    }
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() {
        return Ok(0);
    }
    if metadata.is_file() {
        return Ok(metadata.len());
    }
    let mut total = 0_u64;
    for entry in fs::read_dir(path)? {
        total = total.saturating_add(measured_size(&entry?.path(), depth + 1)?);
    }
    Ok(total)
}

fn copy_directory(source: &Path, destination: &Path, depth: usize) -> io::Result<()> {
    if depth > 64 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Folder nesting is too deep",
        ));
    }
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let source_child = entry.path();
        let metadata = fs::symlink_metadata(&source_child)?;
        if metadata.file_type().is_symlink() {
            continue;
        }
        let destination_child = destination.join(entry.file_name());
        if metadata.is_dir() {
            copy_directory(&source_child, &destination_child, depth + 1)?;
        } else {
            copy_file_verified(&source_child, &destination_child)?;
        }
    }
    Ok(())
}

fn copy_file_verified(source: &Path, destination: &Path) -> io::Result<()> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }
    let copied = fs::copy(source, destination)?;
    let expected = fs::metadata(source)?.len();
    let actual = fs::metadata(destination)?.len();
    if copied != expected || actual != expected {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Snapshot verification failed",
        ));
    }
    Ok(())
}

fn failed_asset(original: &str, is_directory: bool, message: String) -> StoredFile {
    StoredFile {
        original_path: original.to_string(),
        stored_path: None,
        size_bytes: 0,
        is_directory,
        status: StoredFileStatus::Failed,
        message: Some(message),
        thumb_path: None,
    }
}

fn skipped_asset(original: &str, is_directory: bool, message: &str) -> StoredFile {
    StoredFile {
        original_path: original.to_string(),
        stored_path: None,
        size_bytes: 0,
        is_directory,
        status: StoredFileStatus::Skipped,
        message: Some(message.to_string()),
        thumb_path: None,
    }
}

pub fn validate_managed_asset(storage_root: &Path, asset: &Path) -> bool {
    let Ok(asset) = fs::canonicalize(asset) else {
        return false;
    };
    managed_asset_roots(storage_root)
        .into_iter()
        .filter_map(|root| fs::canonicalize(root).ok())
        .any(|root| asset.starts_with(&root) && asset != root)
}

pub fn remove_managed_asset(storage_root: &Path, asset: &Path) -> Result<()> {
    if !validate_managed_asset(storage_root, asset) {
        return Err(Error::Other(format!(
            "refusing to remove unmanaged path: {}",
            asset.display()
        )));
    }
    match fs::symlink_metadata(asset) {
        Ok(metadata) if metadata.is_dir() => fs::remove_dir_all(asset)?,
        Ok(_) => fs::remove_file(asset)?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    Ok(())
}

/// Copies every managed asset to a new root and verifies aggregate byte counts.
/// The old root is deliberately left untouched until the caller has committed
/// updated database paths and switched the live root.
pub fn copy_managed_storage(old_root: &Path, new_root: &Path) -> Result<()> {
    if old_root == new_root {
        return Ok(());
    }
    validate_empty_migration_target(new_root)?;
    let copy_result = (|| {
        prepare_root(new_root)?;
        // Validate every destination before copying any source folder. This
        // keeps rollback safe for an existing, empty marked location.
        for folder in ["images", "thumbs", "files"] {
            let destination = new_root.join(folder);
            if fs::read_dir(&destination)?.next().transpose()?.is_some() {
                return Err(Error::Other(format!(
                    "new storage folder already contains Clipmo {folder} data"
                )));
            }
        }
        for folder in ["images", "thumbs", "files"] {
            let source = old_root.join(folder);
            if !source.exists() {
                continue;
            }
            let destination = new_root.join(folder);
            copy_directory(&source, &destination, 0)?;
            if measured_size(&source, 0)? != measured_size(&destination, 0)? {
                return Err(Error::Other(format!(
                    "storage verification failed for {folder}"
                )));
            }
        }
        Ok(())
    })();
    if copy_result.is_err() && new_root.join(MARKER_FILE).is_file() {
        let _ = remove_managed_directories(new_root);
    }
    copy_result
}

/// Ensures a migration destination contains no pre-existing managed assets.
/// Once this succeeds, a failed copy can safely remove the three managed
/// subdirectories because they were empty (or absent) before migration began.
pub fn validate_empty_migration_target(root: &Path) -> Result<()> {
    if root.exists() && !root.is_dir() {
        return Err(Error::Other("storage location must be a directory".into()));
    }

    let marker = root.join(MARKER_FILE);
    if root.exists() && !marker.is_file() {
        let reserved = ["images", "thumbs", "files"]
            .iter()
            .any(|folder| root.join(folder).exists());
        if reserved {
            return Err(Error::Other(
                "storage folder contains reserved Clipmo directories".into(),
            ));
        }
        return Ok(());
    }

    for folder in ["images", "thumbs", "files"] {
        let path = root.join(folder);
        if !path.exists() {
            continue;
        }
        if !path.is_dir() || fs::read_dir(&path)?.next().transpose()?.is_some() {
            return Err(Error::Other(format!(
                "new storage folder already contains Clipmo {folder} data"
            )));
        }
    }
    Ok(())
}

fn remove_existing_destination(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || metadata.is_file() => {
            fs::remove_file(path)
        }
        Ok(_) => fs::remove_dir_all(path),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

/// Removes only the three known managed subdirectories from a marked root.
/// The root itself is retained because the default root also contains SQLite.
pub fn remove_managed_directories(root: &Path) -> Result<()> {
    if !root.join(MARKER_FILE).is_file() {
        return Err(Error::Other(
            "refusing to clean an unmarked storage root".into(),
        ));
    }
    for folder in ["images", "thumbs", "files"] {
        let path = root.join(folder);
        if path.is_dir() {
            fs::remove_dir_all(path)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_root(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("clipdeck-{label}-{}-{unique}", std::process::id()))
    }

    #[test]
    fn managed_asset_validation_rejects_sources_and_roots() {
        let root = test_root("validation");
        prepare_root(&root).unwrap();
        let asset = root.join("files").join("hash").join("note.txt");
        fs::create_dir_all(asset.parent().unwrap()).unwrap();
        fs::write(&asset, b"managed").unwrap();
        let source = root.with_extension("source.txt");
        fs::write(&source, b"source").unwrap();
        assert!(validate_managed_asset(&root, &asset));
        assert!(!validate_managed_asset(&root, &root.join("files")));
        assert!(!validate_managed_asset(&root, &source));
        fs::remove_file(source).unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn snapshot_respects_limit_and_never_moves_the_source() {
        let root = test_root("snapshot-root");
        let source_root = test_root("snapshot-source");
        fs::create_dir_all(&source_root).unwrap();
        let small = source_root.join("small.txt");
        let large = source_root.join("large.txt");
        fs::write(&small, b"small").unwrap();
        fs::write(&large, vec![7_u8; 32]).unwrap();

        let assets = snapshot_files(
            &root,
            "00112233445566778899aabbccddeeff",
            &[
                small.to_string_lossy().into_owned(),
                large.to_string_lossy().into_owned(),
            ],
            8,
        )
        .unwrap();
        assert_eq!(assets[0].status, StoredFileStatus::Ready);
        assert_eq!(assets[1].status, StoredFileStatus::Skipped);
        assert!(small.is_file());
        assert!(large.is_file());
        assert_eq!(
            fs::read(assets[0].stored_path.as_ref().unwrap()).unwrap(),
            b"small"
        );

        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(source_root).unwrap();
    }

    #[test]
    fn unmarked_reserved_folders_are_rejected() {
        let root = test_root("reserved");
        fs::create_dir_all(root.join("files")).unwrap();
        assert!(prepare_root(&root).is_err());
        assert!(!root.join(MARKER_FILE).exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn overlap_check_normalizes_parent_components_and_case() {
        let base = PathBuf::from(r"C:\Users\Person\Clipdeck");
        let nested = PathBuf::from(r"c:\users\person\Clipdeck\files\..\images");
        assert!(paths_overlap(&base, &nested).unwrap());
    }

    #[test]
    fn overlap_check_normalizes_native_parent_components() {
        let base = test_root("native-parent-overlap");
        let nested = base.join("files").join("..").join("images");
        assert!(paths_overlap(&base, &nested).unwrap());
        assert!(!paths_overlap(&base, &base.with_file_name("unrelated-root")).unwrap());
    }

    #[test]
    fn overlap_check_normalizes_canonical_and_fallback_paths_consistently() {
        let root = test_root("canonical-overlap");
        fs::create_dir_all(&root).unwrap();

        assert!(paths_overlap(&root, &root.join("not-created")).unwrap());

        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn overlap_check_strips_windows_verbatim_prefixes() {
        let normal = PathBuf::from(r"C:\Users\Person\Clipdeck");
        let verbatim = PathBuf::from(r"\\?\C:\Users\Person\Clipdeck\images");

        assert!(paths_overlap(&normal, &verbatim).unwrap());
    }

    #[test]
    fn migration_target_validation_rejects_existing_managed_data() {
        let root = test_root("migration-target");
        prepare_root(&root).unwrap();
        fs::write(root.join("images").join("existing.png"), b"existing").unwrap();

        assert!(validate_empty_migration_target(&root).is_err());
        assert!(root.join("images").join("existing.png").is_file());

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn failed_storage_copy_cleans_the_new_managed_directories() {
        let old_root = test_root("migration-broken-source");
        let new_root = test_root("migration-rollback-target");
        fs::create_dir_all(&old_root).unwrap();
        fs::write(old_root.join("images"), b"not a directory").unwrap();

        assert!(copy_managed_storage(&old_root, &new_root).is_err());
        assert!(new_root.join(MARKER_FILE).is_file());
        for folder in ["images", "thumbs", "files"] {
            assert!(!new_root.join(folder).exists());
        }

        fs::remove_dir_all(old_root).unwrap();
        fs::remove_dir_all(new_root).unwrap();
    }

    #[test]
    fn is_image_path_recognises_supported_extensions_case_insensitively() {
        assert!(is_image_path(Path::new("C:/photos/og_images.png")));
        assert!(is_image_path(Path::new("C:/photos/screenshot.JPG")));
        assert!(is_image_path(Path::new("image.WebP")));
        assert!(!is_image_path(Path::new("diagram.svg")));
        assert!(!is_image_path(Path::new("notes.txt")));
        assert!(!is_image_path(Path::new("no-extension")));
    }

    #[test]
    fn image_thumbnail_is_generated_for_image_files_only() {
        let root = test_root("thumbnail-root");
        let source_root = test_root("thumbnail-source");
        fs::create_dir_all(&source_root).unwrap();

        // Create a tiny 2x2 PNG so the decoder has something real to work with.
        let image = image::RgbaImage::from_fn(2, 2, |_x, _y| image::Rgba([255, 0, 0, 255]));
        let image_path = source_root.join("og_images.png");
        image.save(&image_path).unwrap();
        let text_path = source_root.join("notes.txt");
        fs::write(&text_path, b"plain text").unwrap();

        let hash = "00112233445566778899aabbccddeeff";
        let assets = snapshot_files(
            &root,
            hash,
            &[
                image_path.to_string_lossy().into_owned(),
                text_path.to_string_lossy().into_owned(),
            ],
            1024,
        )
        .unwrap();

        let image_asset = assets
            .iter()
            .find(|a| a.original_path.contains(".png"))
            .unwrap();
        assert_eq!(image_asset.status, StoredFileStatus::Ready);
        assert!(
            image_asset.thumb_path.is_some(),
            "image files should produce a managed thumbnail"
        );
        let thumb_path = image_asset.thumb_path.as_ref().unwrap();
        assert!(thumb_path.starts_with(root.to_string_lossy().as_ref()));
        assert!(
            Path::new(thumb_path).is_file(),
            "thumbnail must exist on disk"
        );

        let text_asset = assets
            .iter()
            .find(|a| a.original_path.contains(".txt"))
            .unwrap();
        assert!(text_asset.thumb_path.is_none());

        // Invalid source must not blow up snapshot_files; the asset is still
        // reported as Ready and only the thumbnail is missing.
        let missing_path = source_root.join("missing.png");
        let assets_missing = snapshot_files(
            &root,
            "ffeeddccbbaa99887766554433221100",
            &[missing_path.to_string_lossy().into_owned()],
            64,
        )
        .unwrap();
        let missing = &assets_missing[0];
        assert_eq!(missing.status, StoredFileStatus::Failed);
        assert!(missing.thumb_path.is_none());

        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(source_root).unwrap();
    }
}
