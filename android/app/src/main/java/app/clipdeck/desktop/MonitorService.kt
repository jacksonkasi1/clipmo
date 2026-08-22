package app.clipdeck.desktop

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.Service
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.database.ContentObserver
import android.net.Uri
import android.os.Build
import android.os.Handler
import android.os.IBinder
import android.os.Looper
import android.provider.MediaStore
import android.util.Log
import androidx.core.content.ContextCompat
import kotlinx.coroutines.*

class MonitorService : Service() {
	companion object {
		const val CHANNEL_ID = "clipmo_monitor_channel"
		const val NOTIF_ID = 1
		const val ACTION_CAPTURE_CURRENT = "app.clipdeck.desktop.CAPTURE_CURRENT"
		// Clipboard-captured images keep the LAN sync budget so they remain
		// shareable to the desktop; larger screenshots stay local instead.
		private const val MAX_SYNC_IMAGE_BYTES = 512L * 1024
		private const val SCREENSHOT_SCAN_DELAY_MS = 1_500L
	}

	private val binder = LocalBinder()
	private val serviceScope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
	private val screenshotHandler = Handler(Looper.getMainLooper())
	private var clipboardManager: ClipboardManager? = null
	private var lastClipText: String? = null
	private var listenerRegistered = false
	private var screenshotObserverRegistered = false
	private var screenshotsBaselineSeconds = 0L
	private val clipCapture = ClipCapture(this)

	inner class LocalBinder : android.os.Binder() {
		fun getService() = this@MonitorService
	}

	override fun onCreate() {
		super.onCreate()
		val nm = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
		if (nm.getNotificationChannel(CHANNEL_ID) == null) {
			val ch = NotificationChannel(
				CHANNEL_ID,
				"Clipmo Clipboard",
				NotificationManager.IMPORTANCE_LOW
			)
			ch.description = "Shows that clipboard monitoring is active"
			ch.setShowBadge(false)
			nm.createNotificationChannel(ch)
		}
		clipboardManager = getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
	}

	override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
		val notif = Notification.Builder(this, CHANNEL_ID)
			.setContentTitle("Clipmo")
			.setContentText("Clipboard monitoring active")
			.setSmallIcon(R.mipmap.ic_launcher)
			.setOngoing(true)
			.build()
		startForeground(NOTIF_ID, notif)

		if (intent?.action == ACTION_CAPTURE_CURRENT) {
			serviceScope.launch { processClipChange() }
		} else {
			// Read current clipboard content to avoid re-inserting it on service startup.
			readCurrentClipboard()
		}

		// Start clipboard listener
		if (!listenerRegistered) {
			clipboardManager?.addPrimaryClipChangedListener(clipListener)
			listenerRegistered = true
		}
		registerScreenshotObserver()

		Log.i("MonitorService", "started")
		return START_STICKY
	}

	override fun onBind(intent: Intent): IBinder = binder

	override fun onDestroy() {
		if (listenerRegistered) {
			clipboardManager?.removePrimaryClipChangedListener(clipListener)
			listenerRegistered = false
		}
		if (screenshotObserverRegistered) {
			contentResolver.unregisterContentObserver(screenshotObserver)
			screenshotObserverRegistered = false
		}
		screenshotHandler.removeCallbacks(pendingScreenshotScan)
		serviceScope.cancel()
		Log.i("MonitorService", "stopped")
	}

	private fun readCurrentClipboard() {
		try {
			val clip = clipboardManager?.primaryClip
			if (clip != null && clip.itemCount > 0) {
				val text = clip.getItemAt(0).coerceToText(this)?.toString()
				if (!text.isNullOrEmpty()) {
					lastClipText = text
				}
			}
		} catch (_: Exception) {}
	}

	private val clipListener = ClipboardManager.OnPrimaryClipChangedListener {
		serviceScope.launch {
			processClipChange()
		}
	}

	private val screenshotObserver = object : ContentObserver(screenshotHandler) {
		override fun onChange(selfChange: Boolean, uri: Uri?) {
			// MediaStore notifies per-row while a screenshot is being written;
			// debouncing collapses the burst into a single scan.
			screenshotHandler.removeCallbacks(pendingScreenshotScan)
			screenshotHandler.postDelayed(pendingScreenshotScan, SCREENSHOT_SCAN_DELAY_MS)
		}
	}

	private val pendingScreenshotScan = Runnable {
		serviceScope.launch { scanForNewScreenshots() }
	}

	private fun registerScreenshotObserver() {
		if (screenshotObserverRegistered) return
		// Baseline is "now" so screenshots taken before monitoring started are
		// not backfilled into the history.
		screenshotsBaselineSeconds = System.currentTimeMillis() / 1000
		contentResolver.registerContentObserver(
			MediaStore.Images.Media.getContentUri(MediaStore.VOLUME_EXTERNAL),
			true,
			screenshotObserver,
		)
		screenshotObserverRegistered = true
	}

	private fun imageReadPermission(): String =
		if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU)
			android.Manifest.permission.READ_MEDIA_IMAGES
		else
			@Suppress("DEPRECATION")
			android.Manifest.permission.READ_EXTERNAL_STORAGE

	private fun hasImageReadPermission(): Boolean =
		ContextCompat.checkSelfPermission(this, imageReadPermission()) == PackageManager.PERMISSION_GRANTED

	private suspend fun scanForNewScreenshots() = withContext(Dispatchers.IO) {
		if (!getSharedPreferences("clipmo_sync", Context.MODE_PRIVATE)
				.getBoolean("screenshot_capture_enabled", true)
		) {
			// Feature is off: keep the window moving so screenshots taken in
			// the meantime are not backfilled when it is turned on again.
			screenshotsBaselineSeconds = System.currentTimeMillis() / 1000
			return@withContext
		}
		if (!hasImageReadPermission()) return@withContext
		try {
			val collection = MediaStore.Images.Media.getContentUri(MediaStore.VOLUME_EXTERNAL)
			val projection = arrayOf(
				MediaStore.Images.Media._ID,
				MediaStore.Images.Media.DATE_ADDED,
			)
			// Screenshots land in Pictures/Screenshots on stock Android and
			// DCIM/Screenshots on several OEMs; the display-name clause covers
			// ROMs that use a different folder for "Screenshot_*" files.
			val selection = "${MediaStore.Images.Media.DATE_ADDED} >= ?" +
				" AND ${MediaStore.Images.Media.IS_PENDING} = 0" +
				" AND (${MediaStore.Images.Media.RELATIVE_PATH} LIKE ? OR ${MediaStore.Images.Media.DISPLAY_NAME} LIKE ?)"
			val args = arrayOf(screenshotsBaselineSeconds.toString(), "%Screenshot%", "Screenshot%")
			var newest = screenshotsBaselineSeconds
			contentResolver.query(collection, projection, selection, args, "${MediaStore.Images.Media.DATE_ADDED} ASC")?.use { cursor ->
				while (cursor.moveToNext()) {
					val id = cursor.getLong(0)
					val added = cursor.getLong(1)
					if (added > newest) newest = added
					val uri = android.content.ContentUris.withAppendedId(collection, id)
					clipCapture.capture(
						uri,
						source = "screenshot",
						maxImageBytes = ClipCapture.LOCAL_IMAGE_LIMIT_BYTES,
						dedupeKey = "screenshot|$uri",
					)
				}
			}
			screenshotsBaselineSeconds = newest
		} catch (e: Exception) {
			Log.w("MonitorService", "screenshot scan failed", e)
		}
	}

	private suspend fun processClipChange() = withContext(Dispatchers.IO) {
		try {
			val clip = clipboardManager?.primaryClip ?: return@withContext
			if (clip.itemCount == 0) return@withContext
			val item = clip.getItemAt(0)
			val uri = item.uri
			if (uri != null) {
				val preferences = getSharedPreferences("clipmo_sync", Context.MODE_PRIVATE)
				if (preferences.getString("clipboard_suppression_uri", null) == uri.toString()) {
					preferences.edit().remove("clipboard_suppression_uri").apply()
				} else {
					clipCapture.capture(uri, source = "clipboard", maxImageBytes = MAX_SYNC_IMAGE_BYTES)
				}
				return@withContext
			}
			val text = item.coerceToText(this@MonitorService)?.toString() ?: return@withContext
			if (text.isEmpty()) return@withContext
			if (text == lastClipText) return@withContext
			lastClipText = text
			val preferences = getSharedPreferences("clipmo_sync", Context.MODE_PRIVATE)
			val suppressionHash = preferences.getString("clipboard_suppression_hash", null)
			if (suppressionHash == contentHash(text)) {
				preferences.edit().remove("clipboard_suppression_hash").apply()
				return@withContext
			}

			val db = openOrCreateDatabase("clipdeck.db", Context.MODE_PRIVATE, null)
			val now = System.currentTimeMillis()
			val cursor = db.query(
				"items",
				arrayOf("id", "content"),
				"content = ?",
				arrayOf(text),
				null, null, null, "1"
			)
			cursor.use { c ->
				if (c.moveToFirst()) {
					// Already exists - update timestamp
					val id = c.getLong(c.getColumnIndexOrThrow("id"))
					db.execSQL("UPDATE items SET ts=$now WHERE id=$id")
				} else {
					val type = when {
						text.startsWith("http://") || text.startsWith("https://") -> "URL"
						text.startsWith("content://") -> "IMAGE"
						else -> "TEXT"
					}
					val cv = android.content.ContentValues()
					cv.put("content", text)
					cv.put("type", type)
					cv.put("ts", now)
					cv.put("source", "clipboard")
					cv.put("id_hash", ClipCapture.shortHash("${deviceId()}|$now|$text"))
					cv.put("origin_device", deviceId())
					cv.put("origin_wall_ms", now)
					cv.put("sync_status", "local")
					db.insert("items", null, cv)
				}
			}
			db.close()
		} catch (e: Exception) {
			Log.w("MonitorService", "processClipChange error", e)
		}
	}

	private fun deviceId(): String = getSharedPreferences("clipmo_sync", Context.MODE_PRIVATE)
		.getString("device_id", "local").orEmpty()

	private fun contentHash(content: String): String = ClipCapture.shortHash(content, 16)

}
