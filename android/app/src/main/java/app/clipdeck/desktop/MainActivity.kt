package app.clipdeck.desktop

import android.Manifest
import android.app.NotificationChannel
import android.app.NotificationManager
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.widget.Toast
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.core.content.ContextCompat
import androidx.core.content.FileProvider
import androidx.core.content.IntentCompat
import androidx.core.view.WindowCompat
import androidx.lifecycle.lifecycleScope
import app.clipdeck.desktop.data.ClipRecord
import app.clipdeck.desktop.data.ClipboardStore
import app.clipdeck.desktop.data.TrustedDeviceRecord
import app.clipdeck.desktop.ui.ClipmoApp
import app.clipdeck.desktop.ui.ClipmoUiState
import app.clipdeck.desktop.ui.theme.ClipmoThemeMode
import java.util.UUID
import java.security.MessageDigest
import java.io.File
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

class MainActivity : ComponentActivity() {
    private lateinit var store: ClipboardStore
    private val clipCapture by lazy { ClipCapture(applicationContext) }
    private val preferences by lazy { getSharedPreferences(PREFERENCES_NAME, Context.MODE_PRIVATE) }

    private var clips by mutableStateOf<List<ClipRecord>>(emptyList())
    private var monitorEnabled by mutableStateOf(false)
    private var screenshotCaptureEnabled by mutableStateOf(true)
    private var syncEnabled by mutableStateOf(false)
    private var copyLiveSyncToClipboard by mutableStateOf(false)
    private var pairingCode by mutableStateOf("")
    private var pairingModeActive by mutableStateOf(false)
    private var themeMode by mutableStateOf(ClipmoThemeMode.SYSTEM)
    private var trustedDevices by mutableStateOf<List<TrustedDeviceRecord>>(emptyList())
    private var collections by mutableStateOf<List<String>>(emptyList())
    private val refreshHandler = Handler(Looper.getMainLooper())
    private val refreshTask = object : Runnable {
        override fun run() {
            if (::store.isInitialized) refreshAsync()
            refreshHandler.postDelayed(this, DEVICE_REFRESH_MS)
        }
    }

    private val notificationPermission = registerForActivityResult(
        ActivityResultContracts.RequestPermission(),
    ) { requestMediaPermissionIfNeeded() }

    private val mediaPermission = registerForActivityResult(
        ActivityResultContracts.RequestPermission(),
    ) { granted ->
        if (!granted) {
            Toast.makeText(this, "Without photo access Clipmo cannot auto-capture screenshots", Toast.LENGTH_LONG).show()
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        store = ClipboardStore(this)
        loadPreferences()
        createNotificationChannel()
        requestRuntimePermissionsIfNeeded()
        restartEnabledServices()
        handleSharedClip(intent)
        setContent {
            val systemDark = isSystemInDarkTheme()
            val isDark = when (themeMode) {
                ClipmoThemeMode.SYSTEM -> systemDark
                ClipmoThemeMode.LIGHT -> false
                ClipmoThemeMode.DARK -> true
            }
            SideEffect {
                WindowCompat.getInsetsController(window, window.decorView).apply {
                    isAppearanceLightStatusBars = !isDark
                    isAppearanceLightNavigationBars = !isDark
                }
            }
            ClipmoApp(
                state = ClipmoUiState(
                    clips = clips,
                    monitorEnabled = monitorEnabled,
                    screenshotCaptureEnabled = screenshotCaptureEnabled,
                    syncEnabled = syncEnabled,
                    copyLiveSyncToClipboard = copyLiveSyncToClipboard,
                    pairingCode = pairingCode,
                    pairingModeActive = pairingModeActive,
                    localDeviceName = getOrCreateDeviceName(),
                    localDeviceId = getOrCreateDeviceId(),
                    themeMode = themeMode,
                    trustedDevices = trustedDevices,
                    collections = collections,
                ),
                onCopy = ::copyToClipboard,
                onFavorite = { store.toggleFavorite(it.id); refresh() },
                onDelete = { store.delete(it.id); refresh() },
                onFavoriteMany = { store.favorite(it); refresh() },
                onDeleteMany = { store.delete(it); refresh() },
                onCreateCollection = { store.createCollection(it); refresh() },
                onAddToCollection = { ids, collection -> store.addToCollection(ids, collection); refresh() },
                onEditClip = { id, content ->
                    if (store.editContent(id, content)) {
                        Toast.makeText(this, "Saved", Toast.LENGTH_SHORT).show()
                    }
                    refresh()
                },
                onClear = { store.clear(); refresh() },
                onMonitorChanged = ::handleMonitorChanged,
                onScreenshotCaptureChanged = ::handleScreenshotCaptureChanged,
                onSyncChanged = ::handleSyncChanged,
                onCopyLiveSyncChanged = ::handleCopyLiveSyncChanged,
                onPairingCodeChanged = ::handlePairingCodeChanged,
                onThemeChanged = ::handleThemeChanged,
                onForgetDevice = ::forgetDevice,
                onStartPairing = ::startPairing,
                onRefresh = ::refreshSync,
            )
        }
    }

    override fun onResume() {
        super.onResume()
        if (::store.isInitialized) refreshAsync()
        refreshHandler.removeCallbacks(refreshTask)
        refreshHandler.postDelayed(refreshTask, DEVICE_REFRESH_MS)
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        setIntent(intent)
        handleSharedClip(intent)
    }

    override fun onWindowFocusChanged(hasFocus: Boolean) {
        super.onWindowFocusChanged(hasFocus)
        if (hasFocus) {
            // Android 10+ permits ordinary apps to read the clipboard only while
            // focused. Waiting for window focus avoids the onResume race where
            // the system still considers the previous application foreground.
            refreshHandler.postDelayed({
                captureClipboardWhileForeground()
                refreshAsync()
            }, CLIPBOARD_FOCUS_DELAY_MS)
        }
    }

    override fun onPause() {
        refreshHandler.removeCallbacks(refreshTask)
        super.onPause()
    }

    override fun onDestroy() {
        if (::store.isInitialized) store.close()
        super.onDestroy()
    }

    private fun refresh() = refreshAsync()

    private fun refreshAsync() {
        // Capture the current lists on the main thread, then diff on IO:
        // comparing ~2k records every refresh pulse on the main thread was
        // causing jank spikes during fast scrolling.
        val currentClips = clips
        val currentCollections = collections
        val currentDevices = trustedDevices
        lifecycleScope.launch {
            val snapshot = withContext(Dispatchers.IO) {
                val updatedClips = store.all()
                val updatedCollections = store.collections()
                val updatedDevices = store.trustedDevices()
                RefreshSnapshot(
                    clips = updatedClips,
                    collections = updatedCollections,
                    devices = updatedDevices,
                    clipsChanged = updatedClips != currentClips,
                    collectionsChanged = updatedCollections != currentCollections,
                    devicesChanged = updatedDevices != currentDevices,
                )
            }
            if (snapshot.clipsChanged) clips = snapshot.clips
            if (snapshot.collectionsChanged) collections = snapshot.collections
            if (snapshot.devicesChanged) trustedDevices = snapshot.devices
            pairingModeActive = preferences.getLong(KEY_PAIRING_UNTIL, 0L) > System.currentTimeMillis()
        }
    }

    private class RefreshSnapshot(
        val clips: List<ClipRecord>,
        val collections: List<String>,
        val devices: List<TrustedDeviceRecord>,
        val clipsChanged: Boolean,
        val collectionsChanged: Boolean,
        val devicesChanged: Boolean,
    )

    private fun forgetDevice(device: TrustedDeviceRecord) {
        store.forgetDevice(device.id)
        if (syncEnabled) {
            stopService(Intent(this, ClipSyncService::class.java))
            startSyncService()
        }
        refresh()
    }

    private fun refreshSync() {
        captureClipboardWhileForeground()
        refresh()
        if (syncEnabled) {
            // The service continuously broadcasts and listens. Re-delivering its
            // start intent recovers a stopped service without tearing down a live
            // TCP listener (which previously caused the port to hop on refresh).
            startSyncService()
        }
    }

    private fun startPairing() {
        if (pairingCode.isBlank()) {
            Toast.makeText(this, "Enter a pairing code first", Toast.LENGTH_SHORT).show()
            return
        }
        preferences.edit().putLong(KEY_PAIRING_UNTIL, System.currentTimeMillis() + PAIRING_WINDOW_MS).apply()
        pairingModeActive = true
        if (!syncEnabled) handleSyncChanged(true) else refreshSync()
        Toast.makeText(this, "Pairing open for 2 minutes", Toast.LENGTH_SHORT).show()
    }

    private fun copyToClipboard(clip: ClipRecord) {
        val clipboard = getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
        val asset = clip.assetPaths?.lineSequence()?.firstOrNull()?.let(::File)?.takeIf(File::isFile)
        if (asset != null && clip.kind in setOf(app.clipdeck.desktop.data.ClipKind.IMAGE, app.clipdeck.desktop.data.ClipKind.FILE)) {
            val uri = FileProvider.getUriForFile(this, "$packageName.files", asset)
            preferences.edit().putString(KEY_CLIPBOARD_SUPPRESSION_URI, uri.toString()).apply()
            clipboard.setPrimaryClip(ClipData.newUri(contentResolver, "Clipmo", uri))
        } else {
            clipboard.setPrimaryClip(ClipData.newPlainText("Clipmo", clip.content))
        }
        Toast.makeText(this, "Copied", Toast.LENGTH_SHORT).show()
    }

    private fun captureClipboardWhileForeground(): Boolean {
        if (!::store.isInitialized || !monitorEnabled || !hasWindowFocus()) return false
        val clipboard = getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
        val item = runCatching {
            clipboard.primaryClip?.takeIf { it.itemCount > 0 }?.getItemAt(0)
        }.getOrNull() ?: return false
        if (item.uri != null) {
            startForegroundService(
                Intent(this, MonitorService::class.java).setAction(MonitorService.ACTION_CAPTURE_CURRENT),
            )
            return false
        }
        val content = item.coerceToText(this)?.toString()?.takeIf(String::isNotBlank) ?: return false
        val hash = MessageDigest.getInstance("SHA-256")
            .digest(content.toByteArray(Charsets.UTF_8))
            .take(16).joinToString("") { "%02x".format(it) }
        val suppression = preferences.getString(KEY_CLIPBOARD_SUPPRESSION_HASH, null)
        if (suppression == hash) {
            preferences.edit().remove(KEY_CLIPBOARD_SUPPRESSION_HASH).apply()
            return false
        }
        return store.captureText(content, getOrCreateDeviceId())
    }

    private fun handleMonitorChanged(enabled: Boolean) {
        monitorEnabled = enabled
        preferences.edit().putBoolean(KEY_MONITOR_ENABLED, enabled).apply()
        val intent = Intent(this, MonitorService::class.java)
        if (enabled) {
            startForegroundService(intent)
            requestMediaPermissionIfNeeded()
        } else {
            stopService(intent)
        }
    }

    private fun handleScreenshotCaptureChanged(enabled: Boolean) {
        screenshotCaptureEnabled = enabled
        preferences.edit().putBoolean(KEY_SCREENSHOT_CAPTURE_ENABLED, enabled).apply()
        // The service reads the preference on every scan, so no restart is
        // needed; only the photo-access prompt depends on the toggle.
        if (enabled && monitorEnabled) requestMediaPermissionIfNeeded()
    }

    private fun handleSyncChanged(enabled: Boolean) {
        if (enabled && pairingCode.isBlank()) {
            Toast.makeText(this, "Enter a pairing code first", Toast.LENGTH_SHORT).show()
            syncEnabled = false
            return
        }
        syncEnabled = enabled
        preferences.edit().putBoolean(KEY_SYNC_ENABLED, enabled).apply()
        if (enabled) startSyncService() else stopService(Intent(this, ClipSyncService::class.java))
    }

    private fun handleCopyLiveSyncChanged(enabled: Boolean) {
        copyLiveSyncToClipboard = enabled
        preferences.edit().putBoolean(KEY_COPY_LIVE_SYNC_TO_CLIPBOARD, enabled).apply()
    }

    private fun handlePairingCodeChanged(code: String) {
        pairingCode = code.filter { it.isLetterOrDigit() }.take(MAX_PAIRING_CODE_LENGTH)
        preferences.edit().putString(KEY_PAIRING_CODE, pairingCode).apply()
        if (syncEnabled) {
            stopService(Intent(this, ClipSyncService::class.java))
            startSyncService()
        }
    }

    private fun handleThemeChanged(mode: ClipmoThemeMode) {
        themeMode = mode
        preferences.edit().putString(KEY_THEME_MODE, mode.name).apply()
    }

    private fun startSyncService() {
        startForegroundService(
            Intent(this, ClipSyncService::class.java).apply {
                putExtra("device_id", getOrCreateDeviceId())
                putExtra("device_name", getOrCreateDeviceName())
                putExtra("pairing_code", pairingCode)
            },
        )
    }

    private fun restartEnabledServices() {
        if (monitorEnabled) startForegroundService(Intent(this, MonitorService::class.java))
        if (syncEnabled && pairingCode.isNotBlank()) startSyncService()
    }

    private fun loadPreferences() {
        monitorEnabled = preferences.getBoolean(KEY_MONITOR_ENABLED, false)
        screenshotCaptureEnabled = preferences.getBoolean(KEY_SCREENSHOT_CAPTURE_ENABLED, true)
        syncEnabled = preferences.getBoolean(KEY_SYNC_ENABLED, false)
        copyLiveSyncToClipboard = preferences.getBoolean(KEY_COPY_LIVE_SYNC_TO_CLIPBOARD, false)
        pairingCode = preferences.getString(KEY_PAIRING_CODE, "").orEmpty()
        themeMode = runCatching {
            ClipmoThemeMode.valueOf(preferences.getString(KEY_THEME_MODE, ClipmoThemeMode.SYSTEM.name).orEmpty())
        }.getOrDefault(ClipmoThemeMode.SYSTEM)
    }

    private fun getOrCreateDeviceId(): String =
        preferences.getString(KEY_DEVICE_ID, null) ?: UUID.randomUUID().toString().also {
            preferences.edit().putString(KEY_DEVICE_ID, it).apply()
        }

    private fun getOrCreateDeviceName(): String =
        preferences.getString(KEY_DEVICE_NAME, null) ?: "${Build.MANUFACTURER} ${Build.MODEL}".trim().also {
            preferences.edit().putString(KEY_DEVICE_NAME, it).apply()
        }

    private fun createNotificationChannel() {
        val manager = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
        manager.createNotificationChannel(
            NotificationChannel(
                MonitorService.CHANNEL_ID,
                "Clipmo clipboard",
                NotificationManager.IMPORTANCE_LOW,
            ).apply {
                description = "Clipboard monitoring status"
                setShowBadge(false)
            },
        )
    }

    private fun requestRuntimePermissionsIfNeeded() {
        if (
            Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU &&
            ContextCompat.checkSelfPermission(this, Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED
        ) {
            // The completion callback chains the media permission request;
            // launching both at once would drop one of the dialogs.
            notificationPermission.launch(Manifest.permission.POST_NOTIFICATIONS)
        } else {
            requestMediaPermissionIfNeeded()
        }
    }

    private fun requiredImagePermission(): String =
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            Manifest.permission.READ_MEDIA_IMAGES
        } else {
            @Suppress("DEPRECATION")
            Manifest.permission.READ_EXTERNAL_STORAGE
        }

    private fun hasImagePermission(): Boolean =
        ContextCompat.checkSelfPermission(this, requiredImagePermission()) == PackageManager.PERMISSION_GRANTED

    private fun requestMediaPermissionIfNeeded() {
        if (!monitorEnabled || !screenshotCaptureEnabled || hasImagePermission()) return
        mediaPermission.launch(requiredImagePermission())
    }

    private fun handleSharedClip(intent: Intent?) {
        if (intent?.action != Intent.ACTION_SEND) return
        val mime = intent.type?.substringBefore(';').orEmpty()
        when {
            mime.startsWith("image/") -> handleSharedImage(intent, mime)
            mime.startsWith("text/") -> handleSharedText(intent)
            else -> Toast.makeText(this, "Clipmo can save shared text and images", Toast.LENGTH_SHORT).show()
        }
    }

    private fun handleSharedText(intent: Intent) {
        val text = (
            intent.getCharSequenceExtra(Intent.EXTRA_TEXT) ?: intent.getCharSequenceExtra(Intent.EXTRA_SUBJECT)
            )?.toString()?.trim().orEmpty()
        if (text.isEmpty()) {
            Toast.makeText(this, "Nothing to save", Toast.LENGTH_SHORT).show()
            return
        }
        lifecycleScope.launch {
            val saved = withContext(Dispatchers.IO) {
                store.captureText(text, getOrCreateDeviceId(), source = "share")
            }
            Toast.makeText(this@MainActivity, if (saved) "Saved to Clipmo" else "Already in Clipmo", Toast.LENGTH_SHORT).show()
            refreshAsync()
        }
    }

    private fun handleSharedImage(intent: Intent, mime: String) {
        // Grab the URI synchronously: share grants are only valid while the
        // receiving task is alive, while the byte copy runs on IO.
        val uri = IntentCompat.getParcelableExtra(intent, Intent.EXTRA_STREAM, Uri::class.java)
            ?: intent.clipData?.takeIf { it.itemCount > 0 }?.getItemAt(0)?.uri
        if (uri == null) {
            Toast.makeText(this, "Could not read the shared image", Toast.LENGTH_SHORT).show()
            return
        }
        lifecycleScope.launch {
            val saved = withContext(Dispatchers.IO) {
                clipCapture.capture(
                    uri,
                    source = "share",
                    maxImageBytes = ClipCapture.LOCAL_IMAGE_LIMIT_BYTES,
                    dedupeKey = "share|$uri",
                    mimeHint = mime,
                )
            }
            Toast.makeText(this@MainActivity, if (saved) "Saved to Clipmo" else "Could not save the shared image", Toast.LENGTH_SHORT).show()
            refreshAsync()
        }
    }

    private companion object {
        const val PREFERENCES_NAME = "clipmo_sync"
        const val KEY_DEVICE_ID = "device_id"
        const val KEY_DEVICE_NAME = "device_name"
        const val KEY_PAIRING_CODE = "pairing_code"
        const val KEY_SYNC_ENABLED = "sync_enabled"
        const val KEY_MONITOR_ENABLED = "monitor_enabled"
        const val KEY_SCREENSHOT_CAPTURE_ENABLED = "screenshot_capture_enabled"
        const val KEY_COPY_LIVE_SYNC_TO_CLIPBOARD = "copy_live_sync_to_clipboard"
        const val KEY_THEME_MODE = "theme_mode"
        const val KEY_PAIRING_UNTIL = "pairing_until"
        const val KEY_CLIPBOARD_SUPPRESSION_HASH = "clipboard_suppression_hash"
        const val KEY_CLIPBOARD_SUPPRESSION_URI = "clipboard_suppression_uri"
        const val MAX_PAIRING_CODE_LENGTH = 12
        const val DEVICE_REFRESH_MS = 3_000L
        const val PAIRING_WINDOW_MS = 120_000L
        const val CLIPBOARD_FOCUS_DELAY_MS = 250L
    }
}
