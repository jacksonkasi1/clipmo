package app.clipdeck.desktop.data

import android.content.ContentValues
import android.content.Context
import android.database.Cursor
import android.database.sqlite.SQLiteDatabase
import android.database.sqlite.SQLiteOpenHelper
import java.io.File
import java.security.MessageDigest

enum class ClipKind { TEXT, URL, IMAGE, FILE }

data class ClipRecord(
    val id: Long,
    val content: String,
    val kind: ClipKind,
    val timestamp: Long,
    val source: String?,
    val originDevice: String?,
    val favorite: Boolean,
    val tags: List<String>,
    val sizeBytes: Long,
    val assetPaths: String?,
)

data class TrustedDeviceRecord(
    val id: String,
    val name: String,
    val platform: String,
    val color: String,
    val lastSeenMs: Long,
    val online: Boolean,
)

class ClipboardStore(context: Context) :
    SQLiteOpenHelper(context, DATABASE_NAME, null, DATABASE_VERSION) {
    private val managedFilesRoot = context.filesDir.canonicalFile

    override fun onOpen(db: SQLiteDatabase) {
        super.onOpen(db)
        db.rawQuery("SELECT id, content, ts FROM items WHERE id_hash IS NULL OR id_hash=''", null).use { cursor ->
            while (cursor.moveToNext()) {
                val id = cursor.getLong(0)
                val content = cursor.getString(1)
                val timestamp = cursor.getLong(2)
                val digest = MessageDigest.getInstance("SHA-256")
                    .digest("legacy|$id|$timestamp|$content".toByteArray(Charsets.UTF_8))
                    .take(20).joinToString("") { "%02x".format(it) }
                db.execSQL("UPDATE items SET id_hash=? WHERE id=?", arrayOf(digest, id))
            }
        }
        val revokedAssets = mutableListOf<String>()
        db.rawQuery(
            """
            SELECT asset_paths FROM items
            WHERE origin_device IN (SELECT device_id FROM trusted_devices WHERE revoked=1)
              AND asset_paths IS NOT NULL
            """.trimIndent(),
            null,
        ).use { cursor ->
            while (cursor.moveToNext()) {
                cursor.getString(0)?.lineSequence()?.filter(String::isNotBlank)?.toList()?.let(revokedAssets::addAll)
            }
        }
        db.execSQL(
            "DELETE FROM items WHERE origin_device IN (SELECT device_id FROM trusted_devices WHERE revoked=1)",
        )
        revokedAssets.forEach(::deleteManagedAsset)
    }

    override fun onCreate(db: SQLiteDatabase) {
        db.execSQL(
            """
            CREATE TABLE items(
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                content TEXT NOT NULL,
                type TEXT NOT NULL,
                ts INTEGER NOT NULL,
                source TEXT,
                fav INTEGER DEFAULT 0,
                id_hash TEXT,
                origin_device TEXT,
                origin_lamport INTEGER DEFAULT 0,
                origin_wall_ms INTEGER DEFAULT 0,
                size_bytes INTEGER DEFAULT 0,
                asset_paths TEXT,
                tags TEXT DEFAULT '',
                sync_status TEXT DEFAULT 'local'
            )
            """.trimIndent(),
        )
        db.execSQL("CREATE INDEX idx_ts ON items(ts DESC)")
        db.execSQL("CREATE INDEX idx_hash ON items(id_hash)")
        createSyncTables(db)
        createCollectionsTable(db)
    }

    override fun onUpgrade(db: SQLiteDatabase, oldVersion: Int, newVersion: Int) {
        if (oldVersion < 2) {
            db.execSQL("ALTER TABLE items ADD COLUMN id_hash TEXT")
            db.execSQL("ALTER TABLE items ADD COLUMN origin_device TEXT")
            db.execSQL("ALTER TABLE items ADD COLUMN origin_lamport INTEGER DEFAULT 0")
            db.execSQL("ALTER TABLE items ADD COLUMN origin_wall_ms INTEGER DEFAULT 0")
            db.execSQL("ALTER TABLE items ADD COLUMN sync_status TEXT DEFAULT 'local'")
            db.execSQL("CREATE INDEX idx_hash ON items(id_hash)")
        }
        if (oldVersion < 3) {
            db.execSQL("ALTER TABLE items ADD COLUMN size_bytes INTEGER DEFAULT 0")
        }
        if (oldVersion < 5) {
            db.execSQL("ALTER TABLE items ADD COLUMN asset_paths TEXT")
        }
        if (oldVersion < 6) {
            db.execSQL("ALTER TABLE items ADD COLUMN tags TEXT DEFAULT ''")
        }
        createSyncTables(db)
        createCollectionsTable(db)
    }

    fun all(): List<ClipRecord> = query(null, null)

    fun collections(): List<String> {
        val names = sortedSetOf(String.CASE_INSENSITIVE_ORDER)
        readableDatabase.query("collections", arrayOf("name"), null, null, null, null, "name COLLATE NOCASE").use { cursor ->
            while (cursor.moveToNext()) names += cursor.getString(0)
        }
        readableDatabase.query("items", arrayOf("tags"), "tags IS NOT NULL AND tags != ''", null, null, null, null).use { cursor ->
            while (cursor.moveToNext()) {
                cursor.getString(0).split(',').map(String::trim).filter(String::isNotEmpty).forEach(names::add)
            }
        }
        return names.toList()
    }

    fun createCollection(name: String): Boolean {
        val normalized = normalizeCollectionName(name) ?: return false
        return writableDatabase.insertWithOnConflict(
            "collections",
            null,
            ContentValues().apply { put("name", normalized) },
            SQLiteDatabase.CONFLICT_IGNORE,
        ) != -1L
    }

    fun addToCollection(ids: Set<Long>, name: String) {
        val normalized = normalizeCollectionName(name) ?: return
        if (ids.isEmpty()) return
        writableDatabase.beginTransaction()
        try {
            writableDatabase.insertWithOnConflict(
                "collections",
                null,
                ContentValues().apply { put("name", normalized) },
                SQLiteDatabase.CONFLICT_IGNORE,
            )
            ids.forEach { id ->
                writableDatabase.rawQuery(
                    "SELECT tags, id_hash FROM items WHERE id=?",
                    arrayOf(id.toString()),
                ).use { cursor ->
                    if (!cursor.moveToFirst()) return@use
                    val tags = cursor.getString(0).orEmpty().split(',')
                        .map(String::trim).filter(String::isNotEmpty).toMutableList()
                    if (tags.none { it.equals(normalized, ignoreCase = true) }) tags += normalized
                    val idHash = cursor.getString(1) ?: return@use
                    writableDatabase.update(
                        "items",
                        ContentValues().apply { put("tags", tags.distinct().joinToString(",")) },
                        "id=?",
                        arrayOf(id.toString()),
                    )
                    enqueueMutation(writableDatabase, "upsert", idHash, null)
                }
            }
            writableDatabase.setTransactionSuccessful()
        } finally {
            writableDatabase.endTransaction()
        }
    }

    fun editContent(id: Long, content: String): Boolean {
        val trimmed = content.trim()
        if (trimmed.isEmpty()) return false
        var idHash: String? = null
        var changed = false
        writableDatabase.beginTransaction()
        try {
            writableDatabase.rawQuery("SELECT content, id_hash FROM items WHERE id=?", arrayOf(id.toString())).use { cursor ->
                if (cursor.moveToFirst()) {
                    changed = trimmed != cursor.getString(0)
                    idHash = cursor.getString(1)
                }
            }
            val hash = idHash
            if (hash != null && changed) {
                writableDatabase.update(
                    "items",
                    ContentValues().apply {
                        put("content", trimmed)
                        put("ts", System.currentTimeMillis())
                    },
                    "id=?",
                    arrayOf(id.toString()),
                )
                enqueueMutation(writableDatabase, "edit", hash, null)
            }
            writableDatabase.setTransactionSuccessful()
        } finally {
            writableDatabase.endTransaction()
        }
        return changed
    }

    fun captureText(content: String, deviceId: String, source: String = "clipboard"): Boolean {
        if (content.isBlank()) return false
        val now = System.currentTimeMillis()
        readableDatabase.rawQuery("SELECT id FROM items WHERE content=? LIMIT 1", arrayOf(content)).use { cursor ->
            if (cursor.moveToFirst()) {
                writableDatabase.execSQL("UPDATE items SET ts=? WHERE id=?", arrayOf(now, cursor.getLong(0)))
                return false
            }
        }
        val digest = MessageDigest.getInstance("SHA-256")
            .digest("$deviceId|$now|$content".toByteArray(Charsets.UTF_8))
            .take(20).joinToString("") { "%02x".format(it) }
        val values = ContentValues().apply {
            put("content", content)
            put("type", if (content.startsWith("http://") || content.startsWith("https://")) "URL" else "TEXT")
            put("ts", now)
            put("source", source)
            put("id_hash", digest)
            put("origin_device", deviceId)
            put("origin_wall_ms", now)
            put("sync_status", "local")
        }
        return writableDatabase.insertOrThrow("items", null, values) > 0
    }

    fun trustedDevices(): List<TrustedDeviceRecord> {
        val now = System.currentTimeMillis()
        val devices = mutableListOf<TrustedDeviceRecord>()
        readableDatabase.query("trusted_devices", null, "revoked=0", null, null, null, "name COLLATE NOCASE").use { cursor ->
            while (cursor.moveToNext()) {
                val seen = cursor.getLong(cursor.getColumnIndexOrThrow("last_seen_ms"))
                devices += TrustedDeviceRecord(
                    id = cursor.getString(cursor.getColumnIndexOrThrow("device_id")),
                    name = cursor.getString(cursor.getColumnIndexOrThrow("name")),
                    platform = cursor.getString(cursor.getColumnIndexOrThrow("platform")),
                    color = cursor.getString(cursor.getColumnIndexOrThrow("color")),
                    lastSeenMs = seen,
                    online = now - seen <= 30_000,
                )
            }
        }
        return devices
    }

    fun forgetDevice(deviceId: String) {
        val values = ContentValues().apply {
            put("revoked", 1)
            putNull("public_key")
            putNull("shared_secret")
            putNull("last_host")
            putNull("last_port")
        }
        val assets = mutableListOf<String>()
        writableDatabase.beginTransaction()
        try {
            readableDatabase.rawQuery(
                "SELECT asset_paths FROM items WHERE origin_device=? AND asset_paths IS NOT NULL",
                arrayOf(deviceId),
            ).use { cursor ->
                while (cursor.moveToNext()) {
                    cursor.getString(0)?.lineSequence()?.filter(String::isNotBlank)?.toList()?.let(assets::addAll)
                }
            }
            writableDatabase.delete("items", "origin_device=?", arrayOf(deviceId))
            writableDatabase.update("trusted_devices", values, "device_id=?", arrayOf(deviceId))
            writableDatabase.setTransactionSuccessful()
        } finally {
            writableDatabase.endTransaction()
        }
        assets.forEach(::deleteManagedAsset)
    }

    fun toggleFavorite(id: Long) {
        writableDatabase.rawQuery("SELECT fav, id_hash FROM items WHERE id=?", arrayOf(id.toString())).use {
            if (!it.moveToFirst()) return
            val favorite = it.getInt(0) == 0
            val idHash = it.getString(1) ?: return
            writableDatabase.beginTransaction()
            try {
                val values = ContentValues().apply { put("fav", if (favorite) 1 else 0) }
                writableDatabase.update("items", values, "id=?", arrayOf(id.toString()))
                enqueueMutation(writableDatabase, "favorite", idHash, favorite)
                writableDatabase.setTransactionSuccessful()
            } finally {
                writableDatabase.endTransaction()
            }
        }
    }

    fun favorite(ids: Set<Long>) {
        if (ids.isEmpty()) return
        writableDatabase.beginTransaction()
        try {
            ids.forEach { id ->
                writableDatabase.rawQuery(
                    "SELECT fav, id_hash FROM items WHERE id=?",
                    arrayOf(id.toString()),
                ).use { cursor ->
                    if (!cursor.moveToFirst() || cursor.getInt(0) != 0) return@use
                    val idHash = cursor.getString(1) ?: return@use
                    writableDatabase.update(
                        "items",
                        ContentValues().apply { put("fav", 1) },
                        "id=?",
                        arrayOf(id.toString()),
                    )
                    enqueueMutation(writableDatabase, "favorite", idHash, true)
                }
            }
            writableDatabase.setTransactionSuccessful()
        } finally {
            writableDatabase.endTransaction()
        }
    }

    fun delete(id: Long) {
        writableDatabase.rawQuery("SELECT id_hash FROM items WHERE id=?", arrayOf(id.toString())).use {
            if (!it.moveToFirst()) return
            val idHash = it.getString(0) ?: return
            writableDatabase.beginTransaction()
            try {
                enqueueMutation(writableDatabase, "tombstone", idHash, null)
                writableDatabase.delete("items", "id=?", arrayOf(id.toString()))
                writableDatabase.setTransactionSuccessful()
            } finally {
                writableDatabase.endTransaction()
            }
        }
    }

    fun delete(ids: Set<Long>) {
        if (ids.isEmpty()) return
        writableDatabase.beginTransaction()
        try {
            ids.forEach { id ->
                writableDatabase.rawQuery(
                    "SELECT id_hash FROM items WHERE id=?",
                    arrayOf(id.toString()),
                ).use { cursor ->
                    if (!cursor.moveToFirst()) return@use
                    val idHash = cursor.getString(0) ?: return@use
                    enqueueMutation(writableDatabase, "tombstone", idHash, null)
                    writableDatabase.delete("items", "id=?", arrayOf(id.toString()))
                }
            }
            writableDatabase.setTransactionSuccessful()
        } finally {
            writableDatabase.endTransaction()
        }
    }

    fun clear() {
        writableDatabase.beginTransaction()
        try {
            writableDatabase.rawQuery("SELECT id_hash FROM items WHERE id_hash IS NOT NULL", null).use { cursor ->
                while (cursor.moveToNext()) enqueueMutation(writableDatabase, "tombstone", cursor.getString(0), null)
            }
            writableDatabase.delete("items", null, null)
            writableDatabase.setTransactionSuccessful()
        } finally {
            writableDatabase.endTransaction()
        }
    }

    private fun query(selection: String?, args: Array<String>?): List<ClipRecord> {
        val records = mutableListOf<ClipRecord>()
        readableDatabase.query("items", null, selection, args, null, null, "ts DESC").use { cursor ->
            while (cursor.moveToNext()) records += cursor.toRecord()
        }
        return records
    }

    private fun Cursor.toRecord(): ClipRecord {
        val type = getString(getColumnIndexOrThrow("type"))
        return ClipRecord(
            id = getLong(getColumnIndexOrThrow("id")),
            content = getString(getColumnIndexOrThrow("content")),
            kind = runCatching { ClipKind.valueOf(type) }.getOrDefault(ClipKind.TEXT),
            timestamp = getLong(getColumnIndexOrThrow("ts")),
            source = getString(getColumnIndexOrThrow("source")),
            originDevice = getString(getColumnIndexOrThrow("origin_device")),
            favorite = getInt(getColumnIndexOrThrow("fav")) == 1,
            tags = getString(getColumnIndexOrThrow("tags"))
                .orEmpty().split(',').map(String::trim).filter(String::isNotEmpty).distinct(),
            sizeBytes = getLong(getColumnIndexOrThrow("size_bytes")),
            assetPaths = getString(getColumnIndexOrThrow("asset_paths")),
        )
    }

    private fun deleteManagedAsset(path: String) {
        runCatching {
            val file = File(path).canonicalFile
            if (file.path.startsWith(managedFilesRoot.path + File.separator)) file.delete()
        }
    }

    private fun createSyncTables(db: SQLiteDatabase) {
        db.execSQL(
            """
            CREATE TABLE IF NOT EXISTS sync_tombstones(
                id_hash TEXT PRIMARY KEY,
                origin_device TEXT NOT NULL,
                origin_lamport INTEGER NOT NULL,
                origin_wall_ms INTEGER NOT NULL
            )
            """.trimIndent(),
        )
        db.execSQL(
            """
            CREATE TABLE IF NOT EXISTS trusted_devices(
                device_id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                platform TEXT NOT NULL,
                color TEXT NOT NULL,
                public_key TEXT,
                shared_secret TEXT,
                last_host TEXT,
                last_port INTEGER,
                last_seen_ms INTEGER DEFAULT 0,
                trusted_at_ms INTEGER NOT NULL,
                revoked INTEGER DEFAULT 0
            )
            """.trimIndent(),
        )
        db.execSQL(
            """
            CREATE TABLE IF NOT EXISTS sync_outbox(
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                mutation TEXT NOT NULL,
                id_hash TEXT NOT NULL,
                favorite INTEGER,
                created_ms INTEGER NOT NULL
            )
            """.trimIndent(),
        )
    }

    private fun createCollectionsTable(db: SQLiteDatabase) {
        db.execSQL(
            "CREATE TABLE IF NOT EXISTS collections(name TEXT PRIMARY KEY COLLATE NOCASE)",
        )
    }

    private fun normalizeCollectionName(name: String): String? =
        name.trim().replace(',', ' ').replace(Regex("\\s+"), " ").take(MAX_COLLECTION_NAME_LENGTH).ifBlank { null }

    private fun enqueueMutation(db: SQLiteDatabase, mutation: String, idHash: String, favorite: Boolean?) {
        val values = ContentValues().apply {
            put("mutation", mutation)
            put("id_hash", idHash)
            if (favorite != null) put("favorite", if (favorite) 1 else 0)
            put("created_ms", System.currentTimeMillis())
        }
        db.insertOrThrow("sync_outbox", null, values)
    }

    private companion object {
        const val DATABASE_NAME = "clipdeck.db"
        const val DATABASE_VERSION = 7
        const val MAX_COLLECTION_NAME_LENGTH = 40
    }
}
