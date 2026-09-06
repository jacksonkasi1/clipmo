# Clipmo

<p align="left">
  <img src="assets/logo-256.png" alt="Clipmo logo" width="200" height="200" />
</p>

**Fast, private clipboard history for Windows and macOS.** Clipmo uses a Fluent surface
that feels at home on Windows 11, with a compact quick window and a full history
window for previewing and editing clipboard items.

> Press **Ctrl + Shift + V** anywhere to summon the quick window, type to filter,
> use the arrow keys to navigate, and press **Enter** to paste back to the app you
> were using.

## Download

The Windows installer and macOS Intel/Apple Silicon builds are published on the
[Releases](https://github.com/jacksonkasi1/clipmo/releases/latest) page.

[![Latest release](https://img.shields.io/github/v/release/jacksonkasi1/clipmo?label=Clipmo&sort=semver)](https://github.com/jacksonkasi1/clipmo/releases/latest)
[![Platform](https://img.shields.io/badge/platform-Windows%2011-0078d4)](https://github.com/jacksonkasi1/clipmo/releases/latest)
[![License](https://img.shields.io/github/license/jacksonkasi1/clipmo)](LICENSE)

**Direct download — [Clipmo 0.2.4 x64-setup.exe](https://github.com/jacksonkasi1/clipmo/releases/download/v0.2.4/Clipmo_0.2.4_x64-setup.exe)**
(SHA-256 `320803D2EED3B129CE4806F3338F7B8E9E6E0B2C8DE94E5FE1AFD203BD752D12` — signed NSIS installer, ~2.26 MB)

The installer bootstraps WebView2 on machines that do not already have it, then
creates a Start Menu shortcut, a desktop tray entry, and the `Ctrl + Shift + V`
quick-window global hotkey. Existing Clipdeck installations are upgraded in
place; the legacy `app.clipdeck.desktop-*` identifier and storage path are
preserved so your history and settings carry over.

For older builds and the full changelog, see
[all releases](https://github.com/jacksonkasi1/clipmo/releases).

## Features

- **Compact by default** — the quick history opens first; an optional preview
  pane expands beside it for editing, rich previews, and metadata.
- **Captures text, links, emails, colors, images, files, and folders.** Sensitive
  entries flagged by password managers are skipped automatically.
- **Hash-based deduplication** — copying the same content again updates its
  recency and count instead of creating duplicates.
- **Full-text search** — SQLite FTS5 search across visible content, tags,
  application names, and executable paths.
- **Complete history controls** — edit supported values, pin favorites, delete
  individual entries, clear a type, or clear every non-favorite entry.
- **Managed local storage** — durable image and file snapshots with configurable
  location, retention, size limits, and verified migration.
- **Capture controls** — ignore selected applications, configure file-extension
  filters, cap snapshot size, and choose image format/compression.
- **Windows 11 visuals** — Acrylic for the quick flyout, Mica for full windows,
  native accent/theme integration, small rounded quick-window corners, and no
  visible DWM border around the flyout.
- **Fast startup** — pre-created warm windows, a dedicated Win32 clipboard
  listener, virtualized history rendering, and file-backed image thumbnails.
- **No telemetry.** Clipboard history stays on the device unless the user enables
  trusted local-network synchronization.

## Requirements

| Tool | Version |
| --- | --- |
| OS | Windows 11 recommended |
| Rust | 1.85 or newer |
| Node.js | 22.12 or newer |
| Visual Studio | Desktop development with C++ + Windows 11 SDK |
| WebView2 | Included with Windows 11; otherwise the installer bootstraps it |

macOS builds require macOS 12 or newer, on either Intel or Apple Silicon.
Development requires the Xcode Command Line Tools (`xcode-select --install`),
Rust, and Node.js 22.12+. WebKit is included with macOS.

## macOS installation and usage

Choose `Clipmo_<version>_x86_64.dmg` for Intel or
`Clipmo_<version>_arm64.dmg` for Apple Silicon from the same release page as
the Windows installer. Open the disk image and drag Clipmo into Applications.
An `.app.zip` and SHA-256 checksums are also provided for each architecture.

Current Mac builds are **ad-hoc signed, not Apple notarized**. Gatekeeper may
block the first launch; after attempting to open Clipmo, use **System Settings →
Privacy & Security → Open Anyway** if you trust this download. Ad-hoc signing
does not establish a verified developer identity.

Press **Command+Shift+V** for quick history, or **Command+Option+Shift+V** for
the full window. The menu-bar icon also opens either window and Settings.
Closing a window keeps capture running; choose Quit Clipmo from the menu to exit.
Dock activation reopens the full window. Launch at login uses a LaunchAgent.

Automatic paste requires **System Settings → Privacy & Security → Accessibility →
Clipmo**. Clipmo requests this on the first paste attempt and displays instructions
if access is denied. Copying history items and manually pressing Command+V works
without Accessibility permission. Move the app to Applications before granting access.

The native pasteboard captures text, HTML/RTF, PNG/TIFF images, and Finder file
URLs every 200 ms, skipping password-manager concealed/transient markers and its
own writes. Very short-lived clipboard changes between polls can be missed.
Source attribution uses the foreground application at capture time. Mac windows
use native AppKit vibrancy (Sidebar for history/settings, Popover for quick paste),
with a Solid option in Appearance settings. Light/dark themes follow the native
window appearance; macOS Reduce Transparency is respected by AppKit. The initial
opaque Mac build migrates to vibrancy once. Windows Acrylic/Mica remain unchanged.
App icons use
generic glyphs. Data lives in `~/Library/Application Support/app.clipdeck.desktop/`.

## macOS builds and release verification

```bash
npm ci
rustup target add x86_64-apple-darwin aarch64-apple-darwin
npm run tauri build -- --target x86_64-apple-darwin
npm run tauri build -- --target aarch64-apple-darwin
# Optional single app containing both architectures:
npm run tauri build -- --target universal-apple-darwin
bash scripts/test-macos-native.sh
```

Tauri automatically merges `src-tauri/tauri.macos.conf.json` on macOS.
The macOS workflow runs on native Intel and Apple Silicon runners, checks frontend
tests/build, Rust formatting/Clippy/tests, native clipboard round trips on an
isolated pasteboard, app signatures and CPU architecture, DMG integrity, and the
packaged main/quick windows' WebKit readiness handshakes.
Paste into a different application and the Accessibility grant still require a
manual desktop check; CI does not grant privacy permissions.

For new `v*` tags, Mac assets are appended after the Windows workflow creates the
release. To add Mac downloads to an existing release, run **macOS build and release**
from the branch containing this support and set `release_tag` to `v0.2.10` (or the
matching source version). A blank input only builds downloadable CI artifacts.
Both architectures must pass before publishing; existing Windows/Android assets
are preserved. A backfill uses the selected branch's source, not the old tag's
Windows-only source; the workflow run records that commit.

For trusted distribution, supply a Developer ID signing identity and notarization
credentials through Tauri's documented signing setup before replacing the ad-hoc
configuration: [macOS code signing](https://v2.tauri.app/distribute/sign/macos/).

## Development

```bash
npm install
npm run tauri dev
```

The first Rust build compiles the native dependency graph; later builds are
incremental.

## Production build

```bash
npm run tauri build
```

The Windows build produces:

```text
src-tauri/target/release/clipmo.exe
src-tauri/target/release/bundle/nsis/Clipmo_<version>_x64-setup.exe
```

For a reproducible x64 build with a custom Rust target directory:

```powershell
.\scripts\build-win64.ps1
```

The script defaults to `D:\Program\rust-target\clipmo` and supports both GNU
and MSVC toolchains.

## Project layout

```text
clipmo/
├── src/                      React frontend
│   ├── App.tsx               Quick/full application shell
│   ├── Settings.tsx          Settings window
│   ├── components/           Search, history, preview, commands, footer
│   ├── lib/                  Store, platform helpers, typed Tauri wrappers
│   └── styles/               Tokens, component styles, window polish
└── src-tauri/                Rust/Tauri core
    ├── src/
    │   ├── main.rs           Windows-subsystem entry
    │   ├── lib.rs            Tauri builder and bootstrap
    │   ├── commands.rs       Native command handlers
    │   ├── tray.rs           Clipmo tray menu
    │   ├── window.rs         Quick/full window lifecycle
    │   ├── db.rs             SQLite + FTS5 + retention
    │   ├── clipboard/        Listener, readers, writer, classifier
    │   └── win/              DWM, source detection, paste, appearance
    ├── icons/                Window and tray icons
    ├── tauri.conf.json       Product, binary, bundle, and window configuration
    └── Cargo.toml
```

## Default hotkey

**Ctrl + Shift + V.** Win+V is reserved by Windows, so Clipmo uses the common
third-party clipboard-manager shortcut.

## Architecture notes

### Clipboard listener

The listener runs on a dedicated thread with a hidden top-level window and uses
`AddClipboardFormatListener`. Each update checks sensitive-data opt-out formats,
reads supported clipboard formats, classifies the content, computes a stable
hash, and hands the result to the persistence layer.

### Paste back to the previous app

Clipmo captures the previously focused HWND before opening, restores that window
on paste, and sends a native Ctrl+V input sequence after releasing held modifier
keys.

### Window materials

The native layer uses `window-vibrancy` for Acrylic/Mica and DWM attributes for
dark mode, corner clipping, shadows, and border policy. Unsupported attributes
fail gracefully on older Windows builds.

## Upgrade compatibility

The Windows application identifier and legacy managed-storage markers are kept
stable so existing Clipdeck installations retain their database, settings, and
history when upgrading to Clipmo. New user-facing windows, shortcuts, executable
names, installer assets, tray text, and release titles use Clipmo.

## Privacy

Clipmo reads local Windows appearance settings and clipboard data. It does not
send telemetry. Entries marked as excluded from clipboard history by password
managers or Windows are discarded before database persistence.

## License

MIT
