package app.clipdeck.desktop

import android.content.ContentValues
import android.content.Context
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.net.Uri
import android.provider.OpenableColumns
import java.io.ByteArrayOutputStream
import java.io.File
import java.security.MessageDigest

/**
 * Snapshots a content URI into Clipmo's private asset store and records it in
 * the clip database. Shared by the clipboard monitor, the screenshot observer,
 * and the system share sheet so every entry point stores media identically.
 *
 * Items larger than the LAN sync image budget are still stored locally; the
 * sync service simply leaves them unsynced, matching the desktop behavior.
 */
class ClipCapture(private val context: Context) {

	fun capture(
		uri: Uri,
		source: String,
		maxImageBytes: Long,
		dedupeKey: String? = null,
		mimeHint: String? = null,
	): Boolean {
		val mime = runCatching { context.contentResolver.getType(uri) }.getOrNull() ?: mimeHint
		val isImage = mime?.startsWith("image/") == true
		val limit = if (isImage) maxImageBytes else FILE_LIMIT_BYTES
		val bytes = try {
			context.contentResolver.openInputStream(uri)?.use { readBounded(it, limit) }
		} catch (_: Exception) { null } ?: return false
		val now = System.currentTimeMillis()
		val idHash = if (dedupeKey != null) {
			val stable = shortHash("${deviceId()}|$dedupeKey")
			if (alreadyCaptured(stable)) {
				touch(stable, now)
				return true
			}
			stable
		} else {
			shortHash("${deviceId()}|$now|$uri")
		}
		val directory = File(context.filesDir, "local_assets/$idHash").apply { mkdirs() }
		val name = safeFileName(displayName(uri) ?: if (isImage) "clipboard-image.png" else "clipboard-file")
		val mainFile = File(directory, name)
		if (!writeAtomic(mainFile, bytes)) return false
		val assets = mutableListOf(mainFile.absolutePath)
		if (isImage) {
			val thumbnail = createThumbnail(bytes) ?: run { mainFile.delete(); return false }
			val thumbFile = File(directory, "thumb.jpg")
			if (!writeAtomic(thumbFile, thumbnail)) {
				mainFile.delete()
				return false
			}
			assets += thumbFile.absolutePath
		}
		val values = ContentValues().apply {
			put("content", if (isImage) "Image" else name)
			put("type", if (isImage) "IMAGE" else "FILE")
			put("ts", now)
			put("source", source)
			put("id_hash", idHash)
			put("origin_device", deviceId())
			put("origin_wall_ms", now)
			put("size_bytes", bytes.size.toLong())
			put("asset_paths", assets.joinToString("\n"))
			put("sync_status", "local")
		}
		context.openOrCreateDatabase(DATABASE_NAME, Context.MODE_PRIVATE, null).use { db ->
			db.insertOrThrow("items", null, values)
		}
		return true
	}

	private fun deviceId(): String = context.getSharedPreferences(PREFERENCES_NAME, Context.MODE_PRIVATE)
		.getString(KEY_DEVICE_ID, "local").orEmpty()

	private fun alreadyCaptured(idHash: String): Boolean =
		context.openOrCreateDatabase(DATABASE_NAME, Context.MODE_PRIVATE, null).use { db ->
			db.rawQuery("SELECT id FROM items WHERE id_hash=? LIMIT 1", arrayOf(idHash)).use { it.moveToFirst() }
		}

	private fun touch(idHash: String, now: Long) {
		context.openOrCreateDatabase(DATABASE_NAME, Context.MODE_PRIVATE, null).use { db ->
			db.execSQL("UPDATE items SET ts=$now WHERE id_hash=?", arrayOf(idHash))
		}
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
			// Decode with subsampling so multi-megabyte screenshots do not
			// allocate a full-resolution bitmap just to build a 160px preview.
			val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
			BitmapFactory.decodeByteArray(bytes, 0, bytes.size, bounds)
			if (bounds.outWidth <= 0 || bounds.outHeight <= 0) return null
			val longest = maxOf(bounds.outWidth, bounds.outHeight)
			var sample = 1
			while (longest / (sample * 2) >= THUMBNAIL_SIZE) sample *= 2
			val options = BitmapFactory.Options().apply { inSampleSize = sample }
			val bitmap = BitmapFactory.decodeByteArray(bytes, 0, bytes.size, options) ?: return null
			val scale = minOf(1f, THUMBNAIL_SIZE.toFloat() / maxOf(bitmap.width, bitmap.height).coerceAtLeast(1))
			val thumb = Bitmap.createScaledBitmap(
				bitmap,
				(bitmap.width * scale).toInt().coerceAtLeast(1),
				(bitmap.height * scale).toInt().coerceAtLeast(1),
				true,
			)
			ByteArrayOutputStream().use { output ->
				thumb.compress(Bitmap.CompressFormat.JPEG, 72, output)
				if (thumb !== bitmap) thumb.recycle()
				bitmap.recycle()
				output.toByteArray()
			}
		} catch (_: Exception) { null }
	}

	private fun displayName(uri: Uri): String? = context.contentResolver
		.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)
		?.use { cursor -> if (cursor.moveToFirst()) cursor.getString(0) else null }

	private fun safeFileName(value: String): String {
		val base = value.replace('\\', '/').substringAfterLast('/').take(180)
		return base.map { if (it.isLetterOrDigit() || it in ".-_ ") it else '_' }
			.joinToString("").trim('.', ' ').ifBlank { "clipboard-file" }
	}

	private fun writeAtomic(target: File, bytes: ByteArray): Boolean = try {
		val temporary = File(target.parentFile, "${target.name}.part")
		temporary.outputStream().use { it.write(bytes) }
		if (target.exists()) target.delete()
		temporary.renameTo(target)
	} catch (_: Exception) { false }

	companion object {
		const val PREFERENCES_NAME = "clipmo_sync"
		const val KEY_DEVICE_ID = "device_id"
		const val DATABASE_NAME = "clipdeck.db"
		const val FILE_LIMIT_BYTES = 25L * 1024 * 1024
		// Local capture budget for screenshots and shared images. Anything the
		// LAN sync image budget cannot carry simply stays on this device.
		const val LOCAL_IMAGE_LIMIT_BYTES = 8L * 1024 * 1024
		private const val THUMBNAIL_SIZE = 160

		fun shortHash(input: String, bytes: Int = 20): String =
			MessageDigest.getInstance("SHA-256").digest(input.toByteArray(Charsets.UTF_8))
				.take(bytes).joinToString("") { "%02x".format(it) }
	}
}
