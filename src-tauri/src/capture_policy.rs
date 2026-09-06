//! Portable capture-policy helpers kept free of Tauri and Win32 dependencies.

use std::collections::{BTreeSet, HashSet};
use std::path::{Path, PathBuf};

use crate::models::FileFilterMode;
use crate::models::{IgnoredApp, SourceApp};

pub fn normalize_extensions(values: &[String]) -> Vec<String> {
    values
        .iter()
        .filter_map(|value| normalize_extension(value))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

pub fn normalize_extension(value: &str) -> Option<String> {
    let value = value.trim().trim_start_matches('.').to_lowercase();
    if value.is_empty() || value.contains(['/', '\\']) || value.chars().any(char::is_whitespace) {
        None
    } else {
        Some(format!(".{value}"))
    }
}

pub fn filter_local_files(
    files: &[String],
    mode: FileFilterMode,
    configured: &[String],
) -> Vec<String> {
    filter_files_with(files, mode, configured, |path| path.is_dir())
}

fn filter_files_with(
    files: &[String],
    mode: FileFilterMode,
    configured: &[String],
    is_directory: impl Fn(&Path) -> bool,
) -> Vec<String> {
    if mode == FileFilterMode::All {
        return files.to_vec();
    }
    let extensions: HashSet<String> = configured
        .iter()
        .filter_map(|value| normalize_extension(value))
        .collect();
    files
        .iter()
        .filter(|value| {
            let path = PathBuf::from(value);
            if is_directory(&path) {
                return true;
            }
            let extension = path
                .extension()
                .map(|value| format!(".{}", value.to_string_lossy().to_lowercase()));
            let listed = extension.is_some_and(|value| extensions.contains(&value));
            match mode {
                FileFilterMode::All => true,
                FileFilterMode::Include => listed,
                FileFilterMode::Exclude => !listed,
            }
        })
        .cloned()
        .collect()
}

pub fn normalize_identity(value: &str) -> String {
    value
        .trim()
        .trim_matches('"')
        .replace('/', "\\")
        .trim_end_matches('\\')
        .to_lowercase()
}

pub fn normalize_ignored_apps(apps: &[IgnoredApp]) -> Vec<IgnoredApp> {
    let mut seen = BTreeSet::new();
    apps.iter()
        .filter_map(|app| {
            let mut app = app.clone();
            app.display_name = app.display_name.trim().to_string();
            app.executable_path = app.executable_path.trim().trim_matches('"').to_string();
            app.executable_name = app.executable_name.trim().to_string();
            let key = app
                .app_user_model_id
                .as_deref()
                .map(normalize_identity)
                .filter(|value| !value.is_empty())
                .or_else(|| {
                    let value = normalize_identity(&app.executable_path);
                    (!value.is_empty()).then_some(value)
                })
                .or_else(|| {
                    let value = normalize_identity(&app.executable_name);
                    (!value.is_empty()).then_some(value)
                })?;
            if app.id.trim().is_empty() {
                app.id = format!("app:{key}");
            }
            seen.insert(key).then_some(app)
        })
        .collect()
}

pub fn source_matches_ignored(source: &SourceApp, ignored: &IgnoredApp) -> bool {
    let source_path = normalize_identity(&source.exe_path);
    // Identities may have been imported from Windows on a Mac (or vice versa).
    let source_name = source_path.rsplit('\\').next().unwrap_or_default();
    let ignored_path = normalize_identity(&ignored.executable_path);
    let ignored_name = normalize_identity(&ignored.executable_name);
    let ignored_display = normalize_identity(&ignored.display_name);

    (!ignored_path.is_empty() && source_path == ignored_path)
        || (!ignored_name.is_empty() && source_name == ignored_name)
        || (ignored_path.is_empty()
            && ignored_name.is_empty()
            && !ignored_display.is_empty()
            && normalize_identity(&source.name) == ignored_display)
}

/// Pure parent-chain policy used by Windows source resolution and portable tests.
/// A WebView process maps only to the first non-WebView, non-broker ancestor.
pub fn webview_host_index(raw_executable: &str, ancestors: &[String]) -> Option<usize> {
    let raw_name = raw_executable
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(raw_executable);
    if !is_webview_name(raw_name) {
        return None;
    }
    ancestors.iter().position(|path| {
        let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
        !is_webview_name(name) && !is_broker_name(name)
    })
}

pub fn is_webview_name(name: &str) -> bool {
    name.eq_ignore_ascii_case("msedgewebview2.exe") || name.eq_ignore_ascii_case("webviewhost.exe")
}

fn is_broker_name(name: &str) -> bool {
    name.eq_ignore_ascii_case("applicationframehost.exe")
        || name.eq_ignore_ascii_case("runtimebroker.exe")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    #[test]
    fn extensions_are_trimmed_dotted_lowercase_deduplicated_and_validated() {
        assert_eq!(
            normalize_extensions(&strings(&[" TXT ", ".txt", "Pdf", "", "bad ext", "a/b"])),
            strings(&[".pdf", ".txt"])
        );
    }

    #[test]
    fn mixed_include_and_exclude_groups_keep_only_accepted_files() {
        let files = strings(&["one.TXT", "two.exe", "README"]);
        assert_eq!(
            filter_files_with(&files, FileFilterMode::Include, &strings(&["txt"]), |_| {
                false
            }),
            strings(&["one.TXT"])
        );
        assert_eq!(
            filter_files_with(&files, FileFilterMode::Exclude, &strings(&["exe"]), |_| {
                false
            }),
            strings(&["one.TXT", "README"])
        );
    }

    #[test]
    fn all_rejected_returns_an_empty_group() {
        let files = strings(&["one.exe", "two.CMD"]);
        assert!(filter_files_with(
            &files,
            FileFilterMode::Exclude,
            &strings(&["exe", "cmd"]),
            |_| false
        )
        .is_empty());
    }

    #[test]
    fn directories_survive_both_extension_policies() {
        let files = strings(&["folder.with.exe", "blocked.exe"]);
        let is_directory = |path: &Path| path.to_string_lossy().starts_with("folder");
        assert_eq!(
            filter_files_with(
                &files,
                FileFilterMode::Include,
                &strings(&["txt"]),
                is_directory
            ),
            strings(&["folder.with.exe"])
        );
        assert_eq!(
            filter_files_with(
                &files,
                FileFilterMode::Exclude,
                &strings(&["exe"]),
                is_directory
            ),
            strings(&["folder.with.exe"])
        );
    }

    fn ignored(path: &str, name: &str) -> IgnoredApp {
        IgnoredApp {
            id: String::new(),
            display_name: name.into(),
            executable_path: path.into(),
            executable_name: Path::new(path)
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            app_user_model_id: None,
            package_family_name: None,
            icon_path: None,
        }
    }

    #[test]
    fn ignored_app_matches_exact_path_and_filename_case_insensitively() {
        let source = SourceApp {
            name: "Clipdeck".into(),
            exe_path: r"C:\Program Files\Clipdeck\clipdeck.exe".into(),
            icon_path: None,
        };
        assert!(source_matches_ignored(
            &source,
            &ignored(r"c:/program files/clipdeck/CLIPDECK.EXE", "Clipdeck")
        ));
        assert!(source_matches_ignored(
            &source,
            &ignored("clipdeck.exe", "")
        ));
        assert!(!source_matches_ignored(
            &source,
            &ignored(r"C:\Apps\Other.exe", "Other")
        ));
    }

    #[test]
    fn webview_child_maps_to_its_specific_host_only() {
        let clipdeck_chain = strings(&[
            r"C:\WebView\msedgewebview2.exe",
            r"C:\Program Files\Clipdeck\clipdeck.exe",
        ]);
        assert_eq!(
            webview_host_index("msedgewebview2.exe", &clipdeck_chain),
            Some(1)
        );
        let other_chain = strings(&[r"C:\Apps\OtherWebViewApp.exe"]);
        assert_eq!(
            webview_host_index("msedgewebview2.exe", &other_chain),
            Some(0)
        );
        assert_ne!(
            normalize_identity(&clipdeck_chain[1]),
            normalize_identity(&other_chain[0])
        );
    }

    #[test]
    fn process_lookup_failure_has_no_invented_host() {
        assert_eq!(webview_host_index("msedgewebview2.exe", &[]), None);
        assert_eq!(webview_host_index("notepad.exe", &[]), None);
    }

    #[test]
    fn all_mode_preserves_order_and_every_path() {
        let files = strings(&["one.exe", "directory", "two.txt"]);
        assert_eq!(
            filter_files_with(&files, FileFilterMode::All, &[], |_| false),
            files
        );
    }
}
