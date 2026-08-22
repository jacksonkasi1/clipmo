# Clipmo Android clipboard capture model

Clipmo uses Android's public `ClipboardManager` listener. On Android 10 and newer, the operating system may deny clipboard reads when Clipmo is not the focused app and is not the active input method. A foreground-service notification keeps the process available, but it does not bypass that privacy restriction.

Supported behavior:

- Clipboard changes are captured while Clipmo is visible and Android grants clipboard access.
- New screenshots are captured automatically while monitoring is enabled and the "Screenshot capture" setting is on (the default): Clipmo observes `MediaStore.Images` and snapshots any image saved into a `Screenshots` folder (or named `Screenshot_*`) shortly after it is written. This needs the "Photos and videos" permission, requested when the feature is active. Screenshots taken while the setting is off are not backfilled later. Screenshots larger than the 512 KiB LAN sync budget stay on the phone.
- Clipmo appears in the system share sheet for text and images ("Save to Clipmo"). Shared content is stored directly in the history and never touches the system clipboard.
- Previously captured history remains available after the app or service restarts.
- LAN sync reconnects when the user opens Clipmo and the persisted sync toggle is enabled.
- Remote text is written to the Android clipboard when Android permits it; a one-shot hash prevents that write from being captured and sent back as a new clip.
- Image or file clipboard URIs are snapshotted immediately while the URI grant is valid, subject to the documented size and extension limits.

Not claimed:

- Unrestricted capture while another app is foregrounded.
- Clipboard access while the screen is locked.
- Automatic foreground-service startup from `BOOT_COMPLETED` on Android 15.
- Capture after process death until the user opens Clipmo again or Android otherwise allows the service to start.

The app never requests overlay or accessibility privileges to work around these platform rules.
