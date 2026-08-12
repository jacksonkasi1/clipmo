package app.clipdeck.desktop

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.Service
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.database.Cursor
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.net.Uri
import android.os.Build
import android.os.IBinder
import android.provider.OpenableColumns
import android.util.Log
import kotlinx.coroutines.*
import java.io.ByteArrayOutputStream
import java.io.File
import java.text.SimpleDateFormat
import java.security.MessageDigest
import java.util.*

class MonitorService : Service() {
	companion object {
		const val CHANNEL_ID = "clipmo_monitor_channel"
		const val NOTIF_ID = 1
		const val MAX_IMAGE_BYTES = 512 * 1024
		const val ACTION_CAPTURE_CURRENT = "app.clipdeck.desktop.CAPTURE_CURRENT"
	}

	private val binder = LocalBinder()
	private val serviceScope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
	private var clipboardManager: ClipboardManager? = null
	private var lastClipText: String? = null
	private var listenerRegistered = false

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

		Log.i("MonitorService", "started")
		return START_STICKY
	}

	override fun onBind(intent: Intent): IBinder = binder

	override fun onDestroy() {
		super.onDestroy()
		if (listenerRegistered) {
			clipboardManager?.removePrimaryClipChangedListener(clipListener)
			listenerRegistered = false
		}
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

	private suspend fun processClipChange() = withContext(Dispatchers.IO) {
		try {
			val clip = clipboardManager?.primaryClip ?: return@withContext
			if (clip.itemCount == 0) return@withContext
			val item = clip.getItemAt(0)
			val uri = item.uri
			if (uri != null && captureUri(uri)) return@withContext
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
					cv.put("id_hash", stableIdHash(text, now))
					cv.put("origin_device", preferences.getString("device_id", "local"))
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

	private fun captureUri(uri: Uri): Boolean {
		val preferences = getSharedPreferences("clipmo_sync", Context.MODE_PRIVATE)
		if (preferences.getString("clipboard_suppression_uri", null) == uri.toString()) {
			preferences.edit().remove("clipboard_suppression_uri").apply()
			return true
		}
		val mime = contentResolver.getType(uri).orEmpty()
		val isImage = mime.startsWith("image/")
		val limit = if (isImage) 420L * 1024 else 25L * 1024 * 1024
		val bytes = try {
			contentResolver.openInputStream(uri)?.use { readBounded(it, limit) }
		} catch (_: Exception) { null } ?: return false
		val now = System.currentTimeMillis()
		val idHash = stableIdHash(uri.toString(), now)
		val directory = File(filesDir, "local_assets/$idHash").apply { mkdirs() }
		val name = safeFileName(displayName(uri) ?: if (isImage) "clipboard-image.png" else "clipboard-file")
		val mainFile = File(directory, name)
		if (!writeAtomic(mainFile, bytes)) return false
		val assets = mutableListOf(mainFile.absolutePath)
		if (isImage) {
			val thumbnail = createThumbnail(bytes) ?: run { mainFile.delete(); return false }
			if (bytes.size + thumbnail.size > MAX_IMAGE_BYTES) {
				mainFile.delete()
				return false
			}
			val thumbFile = File(directory, "thumb.jpg")
			if (!writeAtomic(thumbFile, thumbnail)) return false
			assets += thumbFile.absolutePath
		}
		val values = android.content.ContentValues().apply {
			put("content", if (isImage) "Image" else name)
			put("type", if (isImage) "IMAGE" else "FILE")
			put("ts", now)
			put("source", "clipboard")
			put("id_hash", idHash)
			put("origin_device", preferences.getString("device_id", "local"))
			put("origin_wall_ms", now)
			put("size_bytes", bytes.size.toLong())
			put("asset_paths", assets.joinToString("\n"))
			put("sync_status", "local")
		}
		openOrCreateDatabase("clipdeck.db", Context.MODE_PRIVATE, null).use { db ->
			db.insertOrThrow("items", null, values)
		}
		lastClipText = uri.toString()
		return true
	}

	private fun readBounded(input: java.io.InputStream, limit: Long): ByteArray? {
		val output = ByteArrayOutputStream()
		val buffer = ByteArray(64 * 1024)
		var total = 0L
		while (true) {
			val count = input.read(buffer)
			if (count < 0) break
			total += count
			if (total > limit) return null
			output.write(buffer, 0, count)
		}
		return output.toByteArray()
	}

	private fun createThumbnail(bytes: ByteArray): ByteArray? {
		return try {
			val bitmap = BitmapFactory.decodeByteArray(bytes, 0, bytes.size) ?: return null
			val scale = minOf(1f, 160f / maxOf(bitmap.width, bitmap.height).coerceAtLeast(1))
			val thumb = Bitmap.createScaledBitmap(bitmap, (bitmap.width * scale).toInt().coerceAtLeast(1), (bitmap.height * scale).toInt().coerceAtLeast(1), true)
			ByteArrayOutputStream().use { output ->
				thumb.compress(Bitmap.CompressFormat.JPEG, 72, output)
				if (thumb !== bitmap) thumb.recycle()
				bitmap.recycle()
				output.toByteArray()
			}
		} catch (_: Exception) { null }
	}

	private fun displayName(uri: Uri): String? = contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use { cursor ->
		if (cursor.moveToFirst()) cursor.getString(0) else null
	}

	private fun safeFileName(value: String): String {
		val base = value.replace('\\', '/').substringAfterLast('/').take(180)
		return base.map { if (it.isLetterOrDigit() || it in ".-_ ") it else '_' }.joinToString("").trim('.', ' ').ifBlank { "clipboard-file" }
	}

	private fun writeAtomic(target: File, bytes: ByteArray): Boolean = try {
		val temporary = File(target.parentFile, "${target.name}.part")
		temporary.outputStream().use { it.write(bytes) }
		if (target.exists()) target.delete()
		temporary.renameTo(target)
	} catch (_: Exception) { false }

	private fun contentHash(content: String): String {
		val bytes = MessageDigest.getInstance("SHA-256").digest(content.toByteArray(Charsets.UTF_8))
		return bytes.take(16).joinToString("") { "%02x".format(it) }
	}

	private fun stableIdHash(content: String, timestamp: Long): String {
		val deviceId = getSharedPreferences("clipmo_sync", Context.MODE_PRIVATE)
			.getString("device_id", "local").orEmpty()
		val bytes = MessageDigest.getInstance("SHA-256")
			.digest("$deviceId|$timestamp|$content".toByteArray(Charsets.UTF_8))
		return bytes.take(20).joinToString("") { "%02x".format(it) }
	}

}
