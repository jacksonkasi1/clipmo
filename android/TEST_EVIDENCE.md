# Android sync test evidence

Date: 2026-08-12 (Asia/Calcutta)

## Build

- JDK: Temurin 17.
- `gradlew.bat clean testDebugUnitTest assembleDebug lintDebug`: passed. The subsequent `testDebugUnitTest assembleDebug lintDebug` run also produced a fresh APK, lint report, and test report after the clipboard-loop fix.
- Protocol unit tests: 4 passed, covering Rust-compatible field names, tagged bodies, binary manifests, edits, favorites, tombstones, and the fresh-live-only system-clipboard policy.
- APK installed with `adb install -r` and launched as `app.clipdeck.desktop.debug`.
- Device: CPH2781, Android API 36.
- Logcat: `MainActivity`, `MonitorService`, and `ClipSyncService` showed no application crash. Both foreground services were active with their declared types.
- Rust: `cargo +stable-x86_64-pc-windows-gnu check --lib` and a full debug executable build passed with the portable LLVM/MinGW toolchain. The mutation regression test compiles; its GNU test binary cannot execute on this host (`STATUS_ENTRYPOINT_NOT_FOUND`).

## Real LAN transfer

Windows peer: `JACKSONKASI WIN`, device id `clipdeck-19fbe300d9a-2a8c`, address `192.168.1.2`.

Android peer: `Android CPH2781`, device id `b6f9d5c2-b262-421c-b79e-70c970490574`, address `192.168.1.4`.

- Windows -> Android text: `clipmo-win-to-android-20260812-0103` appeared in Android history under `JACKSONKASI WIN`.
- Windows -> Android clipboard: pasting into Chrome produced the same exact text.
- Android -> Windows text: `clipmo-android-to-win-20260812-0102` appeared in Windows SQLite as kind `text`, Android device id, and status `synced`.
- Restart/reconnect: the Android APK/service was restarted; the persisted trusted endpoint was loaded and the Android -> Windows transfer passed without another manual discovery packet.
- Image: the desktop Clipmo logo copied on Windows appeared as an Android image history item under `JACKSONKASI WIN`, including a rendered thumbnail.
- Loop check: each unique transfer appeared once in its destination history; the remote Android clipboard write did not create a second local-origin row.
- Clipboard-loop hardening: inbound sync is now application-history-only by
  default. Reconnect history is explicitly marked `live=false`; Android will
  never auto-copy it. A default-off setting can copy only a fresh, monotonic,
  explicitly live text/link/email/color delivery. Android sets the persisted
  suppression hash before writing the system clipboard, while the Windows
  receiver imports Android records into its database/UI without writing the
  Windows system clipboard.
- Current desktop database contained 792 clips: 477 text, 179 links, 123 images, 9 emails, 4 files, and 5 favorites.
- Reconnect backfill copied a bounded 493 Windows-origin records to Android (500 total visible records with 7 local records) and preserved all 5 favorites. Android logged no `SQLITE_BUSY` after inbound frames were serialized.
- Live Windows -> Android text passed without refresh using `clipmo-live-win-to-android-20260812-0525`.
- Android -> Windows favorite passed after fixing Rust mutation-field casing: Windows changed item 2330 to `favorite=1` and stamped Android device/version `1786494035131`.
- The live LAN was on a Windows Public network with TCP client isolation/firewall blocking between `192.168.1.2` and `192.168.1.4`. Reconnect/backfill and the final mutation verification therefore used ADB TCP forwarding over the same Clipmo v2 frames; no database row was injected.
- Forget migration passed: the revoked `JACKSONKASI WIN · 3` mirror group was removed on reopen before re-pairing.
- Final screenshots are in `android/artifacts`: History, tag Collections, Devices/Add control, and Settings.
- Final History device counts are mutually exclusive and exact: 500 total = 7 phone + 493 `JACKSONKASI WIN`; both dark and light icon/theme renders were captured.

## Open / failed evidence

- The clipboard-loop build was installed in place on the CPH2781 with
  `adb install -r`, preserving its existing data and device identity. Clipmo
  launched successfully; `MonitorService` and `ClipSyncService` started, the
  TCP listener opened, and startup Logcat contained no fatal application
  exception.
- The subsequent gesture/collection build also passed unit tests, APK assembly,
  and lint, then installed in place with `adb install -r`. Long-press on a real
  History card exposed `1 selected` plus Add to collection, Star, Delete, and
  Exit controls. The Collections tab exposed `+ New`, and tapping it opened the
  collection-name modal. UI hierarchy and screenshots are stored under
  `android/artifacts/clipmo-multi-select.png`, `clipmo-select.xml`, and
  `clipmo-create-collection-ui.xml`.
- Android clipboard capture diagnosis: API 36 Logcat explicitly reported
  `Denying clipboard access to app.clipdeck.desktop.debug, application is not
  in focus`. Capture was moved from the early `onResume` race to delayed window
  focus, with pull-to-refresh as a recovery path. A real Chrome copy of
  `clipmo-mobile-capture-20260812-1104` appeared in mobile History and Windows
  SQLite item 2340 as `synced` from `Android CPH2781`.
- The installed Windows receiver was stale and silently rejected the new Android
  frame. It was replaced with the verified workspace executable plus its
  `WebView2Loader.dll`; the previous executable remains recoverable as
  `C:/Users/jacks/AppData/Local/Clipmo/clipmo.exe.previous`.
- Vertical History performance after moving periodic database queries off the
  main thread, avoiding unchanged state replacement, and decoding thumbnails:
  195 frames, 10 modern janky frames (5.13%), 16 ms median and 28 ms p90 across
  twelve automated up/down swipes on the CPH2781.
- Edit, tombstone, tag mutation, and file payloads were not proven end-to-end in this final run.
- Authenticated encryption/per-device cryptographic trust is not implemented; current pairing-code LAN frames remain plaintext.
- API 29, API 34, API 35, two simultaneous Windows peers, reboot, Wi-Fi/IP change, and revoked-device reconnection were not available/proven in this run.
- Desktop source and executable build successfully with the workspace-local GNU toolchain; the machine still has no Microsoft MSVC linker/Build Tools for the default Rust target.
