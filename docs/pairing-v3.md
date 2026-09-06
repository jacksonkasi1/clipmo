# Pairing and saved connections

The invitation code is no longer the credential for an entire sync group. Each successful pairing creates a separate, persistent device credential on both devices.

## Using it

1. Update Clipmo on Windows, Mac, and Android. This changes the LAN protocol from v2 to v3; existing v2 connections need to be paired once after updating. Local clipboard history is retained.
2. On one device, open **Add / Pair device** (Android: **Devices**) and choose **Start pairing** or **New code**.
3. On the other device, enter that six-digit code and choose **Connect**. Android can instead use **Scan QR code** on the desktop invitation.
4. Wait for a confirmed connection. Entering a code does not replace the joining device's own invitation.
5. Pair each pair of devices that should sync directly. Connections do not depend on a group code or a device acting as a relay.

Version 0.2.13 displays the app version and pairing protocol in Settings. Published 0.2.11 uses v2 and is incompatible with v3; the earlier locally modified 0.2.11 test apps used v3, so that version label alone was misleading. Install a matching 0.2.13 build on all devices.

If code entry cannot discover the other device, enter the **LAN address** displayed on that device alongside its fresh six-digit code. Desktop QR invitations include this address, allowing Android to connect directly without waiting for UDP discovery. Old QR invitations without an address still use discovery. Direct entry accepts private/link-local IPv4 endpoints on Clipmo's sync ports. Discovery now also sends to interface-specific broadcast addresses, and re-pairing a saved device can reuse its last address. Pairing controls allow three seconds to connect and five seconds to receive a response.

Invitations expire after two minutes. **New code** immediately generates a different random code and opens a fresh invitation. **Close pairing** closes invitations while saved devices continue syncing. **Remove** revokes only the selected connection, closes the local invitation to prevent stale re-pairing, and retains local history. Removal works even if the other device is offline; its old credential is rejected on reconnect.

Saved devices survive app restarts. A device becomes connected only after an authenticated TCP exchange; a UDP announcement or an open TCP port alone is insufficient. Devices not heard from for 30 seconds appear offline.

On macOS, allow Clipmo's Local Network permission when prompted (or in System Settings → Privacy & Security → Local Network). The app now supplies the usage description required for local-network discovery; see [Apple's local network privacy guidance](https://developer.apple.com/documentation/technotes/tn3179-understanding-local-network-privacy). Also allow incoming Clipmo connections through the desktop firewall.

## Protocol

- Protocol identifier: `clipmo-lan-v3`; discovery UDP 47633; data/control TCP 47634–47644.
- Discovery announces identity, TCP port, and `pairingAvailable`. It does not broadcast invitation codes or saved credentials.
- Control frames use a four-byte, big-endian JSON length followed by JSON. `kind: pair` exchanges the invitation and a random per-connection token; `kind: ping` authenticates a saved connection. Responses confirm the device identity and token.
- Clipboard frames retain their existing shape; their legacy `pairingCode` field now carries the per-connection token, never the displayed six-digit invitation. Both platforms send and require framed success/failure acknowledgements.
- Desktop credentials are persisted in SQLite's `sync_preferences` table under `peer:<device-id>`. Android credentials use app-private preferences and the existing trusted-device records.
- Pair attempts are limited to 20 per minute per receiving service. Desktop joins and Android pending joins time out after 20 seconds. Invalid/expired codes cannot create trust.
- This remains a local-network, plaintext TCP protocol; it does not provide TLS or protection against an active network attacker. Do not describe it as end-to-end encrypted.

The Android scanner follows the [Google Code Scanner API](https://developers.google.com/ml-kit/vision/barcode-scanning/code-scanner). If its Google Play services module is unavailable, six-digit entry remains available.

## Verification

- 175 frontend tests passed, including pending/failure UI, no local-code replacement, regeneration, QR visibility, expiry, and individual removal.
- 91 Rust tests passed, including real framed TCP pairing between three service instances, credential persistence/reload, code rotation, invalid/closed/expired invitations, individual revocation, and required delivery acknowledgements.
- 9 Android JVM tests passed, including cross-platform control JSON, QR validation, code generation, and existing sync payload contracts.
- Frontend production build and Android debug APK build passed.
- Optimized Mac 0.2.11 app built and installed locally with a verified ad-hoc signature; the previous app was backed up. The installed process starts, listens on TCP 47634, rejects an unpaired v3 ping, and retains all eight pre-install clips.
- Desktop dialog rendered without browser errors or horizontal overflow at 640px; reviewed with connected and offline devices.
- Physical Mac/Android verification on September 6, 2026: installed Android 0.2.11 on CPH2781 through ADB after backing up and restoring the previous app's private data; all 1,537 existing clips survived installation. A fresh Mac invitation entered in Android completed pairing and saved credentials on both devices. Database checks confirmed six Mac-origin hashes on Android and two Android-origin items on Mac. Generating another Mac invitation retained the connection; after force-stopping and reopening Android, authenticated contact resumed without entering a code.
- The earlier reported connection timeout was not reproduced with the fresh invitation. Its original cause remains unconfirmed. Physical Windows delivery and the Android camera scanner still require hardware verification.

Follow-up: code-only joining from Mac to Android reproduced a discovery timeout while Android's invitation was still open and direct TCP reachability worked. Android also logged a transient socket timeout. The direct-address code-entry flow subsequently completed on the physical phone against the Mac. New tests cover direct QR address validation, direct-address UI submission, incompatible discovery diagnostics, and re-pairing saved devices without any UDP announcements. This does not establish Windows hardware connectivity; that requires installing the matching Windows build.

The reverse direct-address test also completed through the installed Mac pairing form using Android's fresh invitation. Saved connections survived subsequent app replacements and Android restarts, with authenticated contact observed again. Connection help is collapsed by default on desktop and Android; the optional address input appears only when expanded. A physical Android screen inspection confirmed one visible code input in the normal flow and a connected Mac. Version labels in Settings are compact. Android release lint passes after raising the scanner's transitive Fragment dependency to 1.8.5.

Debug Android builds use the standard Android debug signing configuration; release signing settings are unchanged.
