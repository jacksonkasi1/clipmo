# Clipmo

<p align="left">
  <img src="assets/logo-256.png" alt="Clipmo logo" width="200" height="200" />
</p>

**Your clipboard, remembered — on Windows, Mac and Android.**

## What is Clipmo?

Normally your computer remembers only the *last* thing you copied. **Clipmo remembers everything you copy**
(text, links, images, files) so you can search it, paste it again, star the important ones and organize them —
and it keeps your **PC, Mac and phone in sync**, so something you copy on one device is waiting on the others.

- **Private by default.** Your clips stay on your devices. No account, no telemetry, nothing is uploaded to the
  developer. Syncing happens directly between your own devices on your Wi-Fi.
- **Works when Wi-Fi sharing is blocked.** An optional encrypted "relay" you host yourself (free on Cloudflare) syncs
  devices that can't see each other.
- **Works with AI assistants.** An optional built-in MCP server lets Claude, Codex, OpenCode and others search and
  organize your clipboard history for you.

> **Quick start:** press **Ctrl + Shift + V** (Windows) or **Command + Shift + V** (Mac), type to filter, use the
> arrow keys and press **Enter** to paste back into the app you were using.

### Get going in 3 steps

1. **Install** Clipmo on your computer and phone — see [Download](#download).
2. **Pair your devices** (optional): on the computer open **Add / Pair device**, then enter the six-digit code on the
   phone (**Devices**) or scan the QR code — see [Sync across devices](#sync-across-devices).
3. **Use it:** copy as usual. Open Clipmo to search, star ⭐, tag, or send a clip to another device.

### Features at a glance

| | What you get |
| --- | --- |
| **History** | Everything you copy — text, links, emails, colors, images, files — searchable, deduplicated, with favorites. |
| **Quick paste** | A tiny popup (`Ctrl/Cmd + Shift + V`) to find and paste anything you copied before. |
| **Organize** | Star favorites, add tags, group clips into collections, clear history whenever you like. |
| **Sync** | Windows, Mac and Android stay in sync over your local network; reconnects automatically. |
| **Send to a device** | Push a clip to one device or all of them with a click. |
| **Clear safely** | Clear on **this device only** or on **every device**. |
| **Relay (optional)** | End-to-end encrypted sync through a server you own, for networks that block direct sync. |
| **AI control (optional)** | A local MCP server so assistants can search, tag, star and clean up your clips. |
| **Private** | No telemetry, no developer-run servers, password-manager entries are skipped. |

## Download

Windows, macOS Intel/Apple Silicon, and Android builds are published on the
[Releases](https://github.com/jacksonkasi1/clipmo/releases/latest) page.

[![Latest release](https://img.shields.io/github/v/release/jacksonkasi1/clipmo?label=Clipmo&sort=semver)](https://github.com/jacksonkasi1/clipmo/releases/latest)
[![Platform](https://img.shields.io/badge/platform-Windows%2011-0078d4)](https://github.com/jacksonkasi1/clipmo/releases/latest)
[![macOS](https://img.shields.io/badge/macOS-Intel%20%7C%20Apple%20Silicon-333333)](https://github.com/jacksonkasi1/clipmo/releases/latest)
[![License](https://img.shields.io/github/license/jacksonkasi1/clipmo)](LICENSE)

**Clipmo 0.3.0 downloads**

| Platform | Download | Checksums |
| --- | --- | --- |
| macOS — Apple Silicon (M-series) | [ARM64 DMG](https://github.com/jacksonkasi1/clipmo/releases/download/v0.3.0/Clipmo_0.3.0_arm64.dmg) | [SHA-256](https://github.com/jacksonkasi1/clipmo/releases/download/v0.3.0/Clipmo_0.3.0_arm64.sha256) |
| macOS — Intel | [x86_64 DMG](https://github.com/jacksonkasi1/clipmo/releases/download/v0.3.0/Clipmo_0.3.0_x86_64.dmg) | [SHA-256](https://github.com/jacksonkasi1/clipmo/releases/download/v0.3.0/Clipmo_0.3.0_x86_64.sha256) |
| Windows — x64 | [Windows installer](https://github.com/jacksonkasi1/clipmo/releases/download/v0.3.0/Clipmo_0.3.0_x64-setup.exe) | [SHA-256](https://github.com/jacksonkasi1/clipmo/releases/download/v0.3.0/SHA256SUMS.txt) |
| Android 10+ | [APK](https://github.com/jacksonkasi1/clipmo/releases/tag/v0.3.0) (see the release assets) | [SHA-256](https://github.com/jacksonkasi1/clipmo/releases/download/v0.3.0/SHA256SUMS.txt) |

The Mac downloads include native vibrancy and are also available as app ZIPs.
See the [release notes](https://github.com/jacksonkasi1/clipmo/releases/tag/v0.3.0)
for what changed, build provenance, verification, and platform signing details.

**Install Android:** download the APK to the phone (or `adb install -r Clipmo_0.3.0_android*.apk`),
allow "Install unknown apps" for your browser/files app when asked, open Clipmo and allow notifications
(they keep sync running). Then pair it with a desktop: on the desktop choose **Add / Pair device**, on the phone
open **Devices** and enter the six-digit code or scan the QR code.

## What's new in 0.3.0

- **Sync that reconnects by itself.** Saved devices are found again even when your network blocks UDP
  broadcasts or the IP address changed, with a **Reconnect** button on desktop and Android.
- **Deleted clips stay deleted.** Fixes cleared clips reappearing; deletes also reach devices that were offline.
- **Faster, ordered sync.** A fresh copy is never stuck behind a bulk delete; each device has its own queue.
- **Clear on this device only, or on every device**, from desktop and Android.
- **Send to device.** Push selected clips (or the latest one from the tray menu) to one device or all of them.
- **Optional encrypted relay** for networks that block direct sync (Cloudflare Worker, your own server, or Firebase) —
  you own the mailbox; see [Sync across devices](#sync-across-devices).
- **Local MCP server.** Let Claude Code, Codex, OpenCode, Cursor or Claude Desktop search, star, tag, organize and
  delete clips — see [Control Clipmo from AI assistants](#control-clipmo-from-ai-assistants-mcp).
- **Android polish.** Clips grouped by day with a Select mode (and select a whole day), confirmation before deleting,
  and the back button navigates instead of closing the app.

The Windows installer bootstraps WebView2 on machines that do not already have it, then
creates a Start Menu shortcut, a desktop tray entry, and the `Ctrl + Shift + V`
quick-window global hotkey. Existing Clipdeck installations are upgraded in
place; the legacy `app.clipdeck.desktop-*` identifier and storage path are
preserved so your history and settings carry over.

For older builds and the full changelog, see
[all releases](https://github.com/jacksonkasi1/clipmo/releases).

## Features in detail

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
- **macOS glass** — native vibrancy on Intel and Apple Silicon, light/dark themes,
  and a saved Vibrancy/Solid choice in Settings → Appearance → Window material.
- **Fast startup** — pre-created warm windows, a dedicated Win32 clipboard
  listener, virtualized history rendering, and file-backed image thumbnails.
- **Sync across your devices** — pair Windows, Mac and Android over the local network,
  optionally through your own encrypted relay. Details below.
- **AI-ready** — an optional local MCP server lets assistants organize your clips.
- **No telemetry.** Clipboard history stays on the device unless you enable
  synchronization. Clipmo's developer operates no servers and never receives your data.

## Requirements

| Tool | Version |
| --- | --- |
| OS | Windows 11 recommended, or macOS 12+ on Intel/Apple Silicon |
| Rust | 1.85 or newer |
| Node.js | 22.12 or newer |
| Visual Studio (Windows development) | Desktop development with C++ + Windows 11 SDK |
| WebView2 (Windows) | Included with Windows 11; otherwise the installer bootstraps it |

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
App icons use generic glyphs. Data lives in
`~/Library/Application Support/app.clipdeck.desktop/`.

To update an existing Mac installation, quit Clipmo and replace the app in
Applications with the new copy. History and preferences are kept separately and
survive the update. Vibrancy turns on automatically when upgrading from the
initial opaque Mac build. If glass is disabled, check **Settings → Appearance →
Window material** and the macOS **Accessibility → Display → Reduce transparency**
preference; Clipmo respects that system preference.

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
packaged main/quick windows' WebKit readiness handshakes. It also verifies an
active AppKit effect behind transparent WebKit surfaces and captures screenshots
over a colored background. Only hosted CI runners have Reduce Transparency
disabled for this visual test; local user accessibility preferences are untouched.
Paste into a different application and the Accessibility grant still require a
manual desktop check; CI does not grant privacy permissions.

For new `v*` tags, Mac assets are appended after the Windows workflow creates the
release. To add Mac downloads to an existing release, run **macOS build and release**
from the branch containing this support and set `release_tag` to `v0.2.11` (or the
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
    │   ├── win/              DWM, source detection, paste, appearance
    │   └── macos/            AppKit pasteboard, paste, appearance, vibrancy
    ├── icons/                Window and tray icons
    ├── tauri.conf.json       Product, binary, bundle, and window configuration
    └── Cargo.toml
```

## Sync across devices

Pair devices once, then clips, stars, tags and deletes follow you.

1. **Pair (local network).** On one device open **Add / Pair device** (Android: **Devices**), then enter the six-digit
   code on the other device or scan the QR code. Both devices need LAN sync switched on and to be on the same Wi-Fi.
   Allow Clipmo through the desktop firewall and, on macOS, the Local Network prompt.
2. **If a device does not show as connected,** press **Reconnect saved devices** (desktop) or **Reconnect** (Android).
   Clipmo checks the saved address and scans your local network for the device if its IP changed.
3. **Clear and send.** *Clear history* asks whether to apply to **this device only** or **all devices**. Select clips and
   choose **Send to device**, or use the tray item **Send latest clip to all devices**.

### When your network blocks direct sync (optional relay)

Wi-Fi "client isolation", strict firewalls or different networks can prevent devices from reaching each other. The
**relay** is a mailbox that **you** own; Clipmo's developer never sees your clips. Every device encrypts clips
(AES-256-GCM) with a **sync passphrase** you choose, so the mailbox only stores scrambled text. Only text clips, edits,
favorites and deletes use the relay; images and files stay on the direct path.

Turn it on in **Settings → Cross-device sync → Relay (optional)** on **every** device, with the **same passphrase**.

| Backend | What you enter | Setup |
| --- | --- | --- |
| **Cloudflare Worker** (or your own HTTP server) | Server address + access password | Deploy `relay/worker.js` (steps below) |
| **Firebase Realtime Database** | Database URL + database secret | Create a Firebase project and enable Realtime Database |
| Turso / Postgres / other SQL | Server address + access password | Put a small server in front of the database that speaks the relay API (see [`relay/README.md`](relay/README.md)); Clipmo never connects to a database directly |

**Cloudflare Worker in four commands**

```bash
cd relay
npx wrangler d1 create clipmo-relay                 # paste the id into wrangler.toml
npx wrangler d1 execute clipmo-relay --remote --file schema.sql
npx wrangler secret put RELAY_TOKEN                 # any long random string = your "access password"
npx wrangler deploy                                 # prints the https://….workers.dev address
```

Enter the printed address and your `RELAY_TOKEN` in Clipmo, plus your passphrase. Full details for Firebase and SQL
backends are in [`relay/README.md`](relay/README.md). Google Drive is not supported yet.

## Control Clipmo from AI assistants (MCP)

**What is this?** MCP ("Model Context Protocol") is the standard way for AI assistants to use other apps. When you turn
it on, assistants such as **Claude, Codex or OpenCode can work with your clipboard history** — you just ask in plain words:

- "Find the link I copied yesterday about invoices."
- "Star my last three clips and tag them *project-x*."
- "Delete all clips that contain the word *password*." *(destructive tools ask for confirmation)*
- "Put today's clips in a collection called *Standup notes*."
- "Send my latest clip to my phone."

**Set it up in three steps**

1. In Clipmo open **Settings → Cross-device sync → AI assistants (MCP)** and tick **Enable local MCP server**.
2. In the same panel, expand your assistant (Claude Code, Codex, OpenCode, Cursor, Claude Desktop) and press **Copy** —
   the snippet already contains your address and access token. Paste it into your assistant (or its config file).
3. Ask your assistant something about your clipboard.

The reference below shows the same snippets with a `YOUR_TOKEN` placeholder, for people setting it up by hand.


Clipmo includes an optional local [MCP](https://modelcontextprotocol.io) server, so Claude Code, Codex, OpenCode, Cursor and other MCP clients can search, star, tag, organize, add and delete clips for you.

- **Off by default.** Turn it on in **Settings → Cross-device sync → AI assistants (MCP)**. That page shows your URL and access token inside ready-to-copy snippets, so you can skip the placeholders below.
- **Local only.** It listens on `127.0.0.1` (never your network), requires a bearer token, and rejects browser requests.
- Default address: `http://127.0.0.1:47650/mcp` (ports 47650–47659 are tried). Replace `YOUR_TOKEN` with the token shown in Settings.

### Claude Code

```bash
claude mcp add --transport http clipmo http://127.0.0.1:47650/mcp --header "Authorization: Bearer YOUR_TOKEN"
```

### Codex

Add to `~/.codex/config.toml` and export the token in your shell profile:

```toml
[mcp_servers.clipmo]
url = "http://127.0.0.1:47650/mcp"
bearer_token_env_var = "CLIPMO_MCP_TOKEN"
```

```bash
export CLIPMO_MCP_TOKEN=YOUR_TOKEN
```

### OpenCode

Add to `opencode.json` (project) or `~/.config/opencode/opencode.json`:

```json
{
  "$schema": "https://opencode.ai/config.json",
  "mcp": {
    "clipmo": {
      "type": "remote",
      "url": "http://127.0.0.1:47650/mcp",
      "headers": { "Authorization": "Bearer YOUR_TOKEN" },
      "enabled": true
    }
  }
}
```

### Cursor / Windsurf (`mcp.json`)

```json
{
  "mcpServers": {
    "clipmo": {
      "url": "http://127.0.0.1:47650/mcp",
      "headers": { "Authorization": "Bearer YOUR_TOKEN" }
    }
  }
}
```

### Claude Desktop (via `mcp-remote`)

```json
{
  "mcpServers": {
    "clipmo": {
      "command": "npx",
      "args": ["-y", "mcp-remote", "http://127.0.0.1:47650/mcp", "--header", "Authorization:Bearer YOUR_TOKEN"]
    }
  }
}
```

### Tools

| Tool | What it does |
| --- | --- |
| `list_clips` | Search/list history (filters: text, kind, favorites, tag, limit) |
| `get_clip` | Full text of one clip |
| `add_clip` | Add a text clip (optional star and tags) |
| `set_favorite` | Star / unstar |
| `delete_clip` | Delete one clip (also on synced devices) |
| `set_tags`, `add_tags`, `remove_tags`, `list_tags` | Manage tags |
| `list_collections`, `create_collection`, `delete_collection` | Manage collections |
| `add_to_collection`, `remove_from_collection` | Organize clips |
| `list_devices` | Paired devices and whether each is connected |
| `send_to_device` | Force-send clips to one device or all devices |
| `clear_history` | Destructive; needs `confirm=true`. Keeps favorites unless `include_favorites`; `scope` is `all` or `device` |

Quick check without any client:

```bash
curl -s http://127.0.0.1:47650/mcp -H "Authorization: Bearer YOUR_TOKEN" -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/list"}'
```

Anything holding the token can read and change your clipboard history, so treat it like a password. **New access token** in Settings revokes the old one.

## Default hotkey

**Ctrl + Shift + V.** Win+V is reserved by Windows, so Clipmo uses the common
third-party clipboard-manager shortcut.

On macOS, use **Command + Shift + V** for quick history and
**Command + Option + Shift + V** for the full window.

## Architecture notes

### Clipboard listener

On Windows, the listener runs on a dedicated thread with a hidden top-level window and uses
`AddClipboardFormatListener`. Each update checks sensitive-data opt-out formats,
reads supported clipboard formats, classifies the content, computes a stable
hash, and hands the result to the persistence layer.

### Paste back to the previous app

On Windows, Clipmo captures the previously focused HWND before opening, restores that window
on paste, and sends a native Ctrl+V input sequence after releasing held modifier
keys.

On macOS, Clipmo remembers the previous application's PID, activates it, and
sends Command+V after checking Accessibility permission.

### Window materials

The native layer uses `window-vibrancy` for Acrylic/Mica and DWM attributes for
dark mode, corner clipping, shadows, and border policy. Unsupported attributes
fail gracefully on older Windows builds.

On macOS, AppKit supplies Sidebar/Popover vibrancy behind transparent WebKit
windows. Effect updates run on the main thread and replace the previous effect
view, so repeated theme changes do not stack blur layers. Solid disables the effect.

## Upgrade compatibility

The Windows application identifier and legacy managed-storage markers are kept
stable so existing Clipdeck installations retain their database, settings, and
history when upgrading to Clipmo. New user-facing windows, shortcuts, executable
names, installer assets, tray text, and release titles use Clipmo.

0.3.0 keeps the local-network protocol (`clipmo-lan-v3`) unchanged, so 0.2.12+ devices still pair and sync with it; update every
device to get the reconnect and delete fixes. Your history and saved connections carry over.

## Privacy

Clipmo reads local operating-system appearance settings and clipboard data. It does not
send telemetry. Entries marked as excluded from clipboard history by password
managers or Windows are discarded before database persistence.

Synchronization is off until you enable it. Direct sync stays on your local network. The optional relay and the optional
MCP server are off by default: the relay only stores text encrypted with a passphrase that never leaves your devices, on a
mailbox you own, and the MCP server listens on `127.0.0.1` only behind an access token. Clipmo's developer operates no servers.

## License

MIT
