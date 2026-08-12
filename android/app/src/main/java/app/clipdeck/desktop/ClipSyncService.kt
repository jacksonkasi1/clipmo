package app.clipdeck.desktop

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.Service
import android.content.Context
import android.content.Intent
import android.database.sqlite.SQLiteDatabase
import android.os.Build
import android.os.IBinder
import android.util.Log
import android.net.wifi.WifiManager
import com.fasterxml.jackson.annotation.JsonInclude
import com.fasterxml.jackson.annotation.JsonProperty
import com.fasterxml.jackson.annotation.JsonSubTypes
import com.fasterxml.jackson.annotation.JsonTypeInfo
import com.fasterxml.jackson.module.kotlin.jacksonObjectMapper
import com.fasterxml.jackson.module.kotlin.registerKotlinModule
import kotlinx.coroutines.*
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import java.io.*
import java.net.*
import java.security.MessageDigest
import java.util.*
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.atomic.AtomicLong

const val PROTOCOL = "clipmo-lan-v2"
const val DISCOVERY_PORT = 47633
const val FIRST_SYNC_PORT = 47634
const val LAST_SYNC_PORT = 47644
const val DISCOVERY_TICK_MS = 3000L
const val IO_TIMEOUT_MS = 20_000L
const val CHUNK_SIZE = 64 * 1024
const val MAX_HEADER_BYTES = 256 * 1024
const val MAX_IMAGE_BYTES = 512 * 1024L
const val HARD_MAX_MESSAGE_BYTES = 128L * 1024 * 1024
const val LIVE_CLIP_MAX_AGE_MS = 30_000L
const val LIVE_CLIP_FUTURE_TOLERANCE_MS = 5_000L
const val KEY_COPY_LIVE_SYNC_TO_CLIPBOARD = "copy_live_sync_to_clipboard"
const val KEY_LAST_REMOTE_CLIPBOARD_AT = "last_remote_clipboard_at"

internal fun shouldCopyRemoteClipToSystemClipboard(
	isLive: Boolean,
	autoCopyEnabled: Boolean,
	copiedAt: Long,
	lastCopiedAt: Long,
	now: Long,
): Boolean = isLive && autoCopyEnabled && copiedAt > lastCopiedAt &&
	copiedAt >= now - LIVE_CLIP_MAX_AGE_MS && copiedAt <= now + LIVE_CLIP_FUTURE_TOLERANCE_MS

enum class PlatformKind { windows, macos, linux, android, ios, unknown }
enum class ItemKind { text, link, email, color, image, files }
enum class SyncStatus { local, synced, pending, offline }

data class DeviceIdentity(
	val id: String,
	val name: String,
	val platform: PlatformKind,
	val color: String
)

data class SyncVersion(
	@JsonProperty("deviceId") val device_id: String,
	val lamport: Long,
	@JsonProperty("wallMs") val wall_ms: Long
)

data class DiscoveryMessage(
	val protocol: String,
	@JsonProperty("pairingCode") val pairing_code: String,
	val device: DeviceIdentity,
	@JsonProperty("tcpPort") val tcp_port: Int
)

data class SyncEnvelope(
	val protocol: String,
	@JsonProperty("pairingCode") val pairing_code: String,
	val device: DeviceIdentity,
	@JsonProperty("tcpPort") val tcp_port: Int,
	val body: SyncBody
)

@JsonTypeInfo(use = JsonTypeInfo.Id.NAME, include = JsonTypeInfo.As.PROPERTY, property = "type")
@JsonSubTypes(
	JsonSubTypes.Type(value = SyncBody.ClipUpsert::class, name = "clipUpsert"),
	JsonSubTypes.Type(value = SyncBody.ImageUpsert::class, name = "imageUpsert"),
	JsonSubTypes.Type(value = SyncBody.FilesUpsert::class, name = "filesUpsert"),
	JsonSubTypes.Type(value = SyncBody.ClipEdit::class, name = "clipEdit"),
	JsonSubTypes.Type(value = SyncBody.FavoriteToggle::class, name = "favoriteToggle"),
	JsonSubTypes.Type(value = SyncBody.Tombstone::class, name = "tombstone"),
)
sealed class SyncBody {
	data class ClipUpsert(val clip: ClipSnapshot) : SyncBody()
	data class ImageUpsert(val clip: ClipSnapshot, val image: ImageSnapshot) : SyncBody()
	data class FilesUpsert(val clip: ClipSnapshot, val files: List<FileSnapshot>) : SyncBody()
	data class ClipEdit(
		@JsonProperty("idHash") val id_hash: String,
		val kind: ItemKind,
		val content: String,
		@JsonProperty("contentHash") val content_hash: String,
		val version: SyncVersion
	) : SyncBody()

	data class FavoriteToggle(
		@JsonProperty("idHash") val id_hash: String,
		val favorite: Boolean,
		val version: SyncVersion
	) : SyncBody()

	data class Tombstone(@JsonProperty("idHash") val id_hash: String, val version: SyncVersion) : SyncBody()

	fun idHash(): String = when (this) {
		is ClipUpsert -> clip.id_hash
		is ImageUpsert -> clip.id_hash
		is FilesUpsert -> clip.id_hash
		is ClipEdit -> id_hash
		is FavoriteToggle -> id_hash
		is Tombstone -> id_hash
	}

	fun version(): SyncVersion = when (this) {
		is ClipUpsert -> clip.version
		is ImageUpsert -> clip.version
		is FilesUpsert -> clip.version
		is ClipEdit -> version
		is FavoriteToggle -> version
		is Tombstone -> version
	}
}

data class ClipSnapshot(
	@JsonProperty("idHash") val id_hash: String,
	val kind: ItemKind,
	val preview: String,
	val content: String,
	@JsonProperty("contentHash") val content_hash: String,
	val favorite: Boolean,
	val tags: List<String> = emptyList(),
	val live: Boolean = false,
	@JsonProperty("copiedAt") val copied_at: Long,
	val version: SyncVersion
)

data class ImageSnapshot(
	val extension: String,
	val width: Int,
	val height: Int,
	@JsonProperty("imageSize") val imageSize: Long,
	@JsonProperty("thumbSize") val thumbSize: Long,
	@JsonProperty("chunkCount") val chunkCount: Int,
)

data class FileSnapshot(
	val name: String,
	val size: Long,
	val mime: String,
	@JsonProperty("chunkCount") val chunkCount: Int,
)

class ClipSyncService : Service() {
	companion object {
		const val CHANNEL_ID = "clipmo_sync_channel"
		const val NOTIF_ID = 2
		const val CONNECT_TIMEOUT_MS = 900L
	}

	private val binder = LocalBinder()
	private val serviceScope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
	private val syncMutex = Mutex()
	private val incomingMutex = Mutex()

	@Volatile private var running = false
	@Volatile private var listenPort = 0

	private val mapper = jacksonObjectMapper().registerKotlinModule().apply {
		setSerializationInclusion(JsonInclude.Include.NON_NULL)
	}

	private val peers = ConcurrentHashMap<String, PeerRecord>()
	private val jobQueue = LinkedList<SyncJob>()
	private val lamport = AtomicLong(System.currentTimeMillis())
	private val suppressions = ConcurrentHashMap<String, Suppression>()
	private var multicastLock: WifiManager.MulticastLock? = null

	data class PeerRecord(
		val device: DeviceIdentity,
		val address: InetSocketAddress,
		val lastSeenAt: Long
	)

	data class SyncJob(
		val body: SyncBody,
		val blobs: List<File> = emptyList(),
		val estimatedBytes: Long = 1024,
		var pendingPeers: MutableSet<String>? = null,
	)

	data class Suppression(
		var editHash: String? = null,
		var favorite: Boolean? = null,
		var assets: Boolean = false,
		var deleted: Boolean = false
	)

	inner class LocalBinder : android.os.Binder() {
		fun getService() = this@ClipSyncService
	}

	override fun onCreate() {
		super.onCreate()
		val nm = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
		if (nm.getNotificationChannel(CHANNEL_ID) == null) {
			val ch = NotificationChannel(CHANNEL_ID, "Clipmo Sync", NotificationManager.IMPORTANCE_LOW)
			ch.description = "LAN sync background service"
			ch.setShowBadge(false)
			nm.createNotificationChannel(ch)
		}
		multicastLock = (applicationContext.getSystemService(Context.WIFI_SERVICE) as WifiManager)
			.createMulticastLock("clipmo-lan-discovery").apply {
				setReferenceCounted(false)
				acquire()
			}
	}

	override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
		val notif = Notification.Builder(this, CHANNEL_ID)
			.setContentTitle("Clipmo Sync")
			.setContentText("LAN sync active")
			.setSmallIcon(R.mipmap.ic_launcher)
			.setOngoing(true)
			.build()
		startForeground(NOTIF_ID, notif)

		if (running) return START_STICKY
		val preferences = getSharedPreferences("clipmo_sync", Context.MODE_PRIVATE)
		if (!preferences.getBoolean("sync_enabled", false)) {
			stopSelf()
			return START_NOT_STICKY
		}
		val deviceId = intent?.getStringExtra("device_id") ?: getOrCreateDeviceId()
		val deviceName = intent?.getStringExtra("device_name")
			?: preferences.getString("device_name", null)
			?: "Android ${Build.MODEL}"
		val pairingCode = intent?.getStringExtra("pairing_code")
			?: preferences.getString("pairing_code", "").orEmpty()

		running = true

		serviceScope.launch {
			ensureSyncTables()
			loadTrustedPeers()
			launch { bindAndListenTcp() }
			launch { broadcastPresence(deviceId, deviceName) }
			launch { listenForPeers(pairingCode, deviceId) }
			launch { drainSendQueue(deviceId, deviceName, pairingCode) }
			launch { watchLocalChanges(deviceId) }
		}

		Log.i("ClipSyncService", "started on port=$listenPort deviceId=$deviceId")
		return START_STICKY
	}

	override fun onBind(intent: Intent): IBinder = binder

	override fun onDestroy() {
		running = false
		multicastLock?.let { if (it.isHeld) it.release() }
		multicastLock = null
		serviceScope.cancel()
		super.onDestroy()
	}

	private fun getOrCreateDeviceId(): String {
		val prefs = getSharedPreferences("clipmo_sync", Context.MODE_PRIVATE)
		val id = prefs.getString("device_id", null)
		if (!id.isNullOrEmpty()) return id
		val newId = UUID.randomUUID().toString().replace("-", "")
		prefs.edit().putString("device_id", newId).apply()
		return newId
	}

	private fun currentDevice(deviceId: String, deviceName: String): DeviceIdentity {
		val prefs = getSharedPreferences("clipmo_sync", Context.MODE_PRIVATE)
		val color = prefs.getString("device_color", null)
		if (color == null) {
			val palette = listOf("#E91E63", "#9C27B0", "#673AB7", "#3F51B5", "#2196F3",
				"#009688", "#4CAF50", "#FF9800", "#795548")
			val pick = palette.random()
			prefs.edit().putString("device_color", pick).apply()
		}
		return DeviceIdentity(
			id = deviceId,
			name = deviceName,
			platform = PlatformKind.android,
			color = prefs.getString("device_color", "#E91E63")!!
		)
	}

	private suspend fun ensureSyncTables() = withContext(Dispatchers.IO) {
		val db = openOrCreateDatabase("clipdeck.db", Context.MODE_PRIVATE, null)
		db.execSQL("""
			CREATE TABLE IF NOT EXISTS sync_tombstones (
				id_hash TEXT PRIMARY KEY,
				origin_device TEXT NOT NULL,
				origin_lamport INTEGER NOT NULL,
				origin_wall_ms INTEGER NOT NULL
			)
		""".trimIndent())
		db.execSQL("""
			CREATE TABLE IF NOT EXISTS trusted_devices (
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
		""".trimIndent())
		db.close()
	}

	private fun contentHash(content: String): String {
		val md = MessageDigest.getInstance("SHA-256")
		val bytes = md.digest(content.toByteArray(Charsets.UTF_8))
		return bytes.take(16).joinToString("") { "%02x".format(it) }
	}

	private fun generateIdHash(content: String, timestamp: Long): String {
		val input = "$content|$timestamp"
		val md = MessageDigest.getInstance("SHA-256")
		val bytes = md.digest(input.toByteArray(Charsets.UTF_8))
		return bytes.take(20).joinToString("") { "%02x".format(it) }
	}

	private suspend fun bindAndListenTcp() = withContext(Dispatchers.IO) {
		for (port in FIRST_SYNC_PORT..LAST_SYNC_PORT) {
			try {
				val listener = ServerSocket()
				listener.reuseAddress = true
				listener.bind(InetSocketAddress(port))
				listener.soTimeout = 500
				listenPort = port
				Log.i("ClipSyncService", "TCP listening on port $port")
				while (running) {
					try {
						val client = listener.accept() ?: continue
						serviceScope.launch {
							// Desktop reconnect backfill can open hundreds of TCP frames in
							// quick succession. Apply them sequentially so separate Android
							// SQLite handles never race journal-mode setup or row writes.
							incomingMutex.withLock { handleIncoming(client) }
						}
					} catch (e: SocketTimeoutException) {
						// normal, just loop
					}
				}
				return@withContext
			} catch (e: IOException) {
				// try next port
			}
		}
		Log.w("ClipSyncService", "no TCP port available in range")
	}

	private suspend fun handleIncoming(client: Socket) = withContext(Dispatchers.IO) {
		try {
			client.soTimeout = IO_TIMEOUT_MS.toInt()
			val input = DataInputStream(BufferedInputStream(client.getInputStream()))
			val header = readFrameHeader(input) ?: return@withContext
			val envelope = mapper.readValue(header, SyncEnvelope::class.java)

			val pairingPrefs = getSharedPreferences("clipmo_sync", Context.MODE_PRIVATE)
			val myPairing = pairingPrefs.getString("pairing_code", "") ?: ""

			if (!running || envelope.protocol != PROTOCOL
				|| envelope.pairing_code != myPairing
				|| envelope.pairing_code.isBlank()
				|| envelope.device.id == currentDevice(
					getSharedPreferences("clipmo_sync", Context.MODE_PRIVATE).getString("device_id", "")!!,
					""
				).id
				|| envelope.body.version().device_id != envelope.device.id
				|| !validIdHash(envelope.body.idHash())
				|| (isRevoked(envelope.device.id) && !pairingWindowOpen())
			) {
				client.close()
				return@withContext
			}

			val peerAddr = InetSocketAddress(client.inetAddress.hostAddress, envelope.tcp_port)
			peers[envelope.device.id] = PeerRecord(envelope.device, peerAddr, System.currentTimeMillis())
			rememberTrustedDevice(envelope.device, peerAddr)

			lamport.updateAndGet { maxOf(it, envelope.body.version().lamport) + 1 }

			when (val body = envelope.body) {
				is SyncBody.ClipUpsert -> applyClipUpsert(envelope, body.clip)
				is SyncBody.ImageUpsert -> applyImageUpsert(envelope, body, input)
				is SyncBody.FilesUpsert -> applyFilesUpsert(envelope, body, input)
				is SyncBody.ClipEdit -> applyClipEdit(body)
				is SyncBody.FavoriteToggle -> applyFavoriteToggle(body)
				is SyncBody.Tombstone -> applyTombstone(body)
			}
			client.close()
		} catch (e: Exception) {
			Log.w("ClipSyncService", "handleIncoming error", e)
			try { client.close() } catch (_: Exception) {}
		}
	}

	private suspend fun sendToPeer(addr: InetSocketAddress, envelope: SyncEnvelope, blobs: List<File>): Boolean =
		withContext(Dispatchers.IO) {
			try {
				val headerBytes = mapper.writeValueAsBytes(envelope)
				if (headerBytes.size > MAX_HEADER_BYTES) return@withContext false
				Socket().use { sock ->
					sock.connect(addr, CONNECT_TIMEOUT_MS.toInt())
					sock.soTimeout = IO_TIMEOUT_MS.toInt()
					DataOutputStream(BufferedOutputStream(sock.getOutputStream())).use { dos ->
						dos.writeInt(headerBytes.size)
						dos.write(headerBytes)
						val buffer = ByteArray(CHUNK_SIZE)
						blobs.forEach { file ->
							file.inputStream().buffered().use { input ->
								while (true) {
									val count = input.read(buffer)
									if (count < 0) break
									dos.write(buffer, 0, count)
								}
							}
						}
						dos.flush()
					}
				}
				true
			} catch (e: Exception) {
				Log.d("ClipSyncService", "send failed to $addr: ${e.message}")
				false
			}
		}

	private suspend fun enqueueItem(itemId: Long, kind: ItemKind, live: Boolean = true) = syncMutex.withLock {
		val db = openOrCreateDatabase("clipdeck.db", Context.MODE_PRIVATE, null)
		val cursor = db.query("items", null, "id=?", arrayOf(itemId.toString()), null, null, null)
		cursor.use { c ->
			if (!c.moveToFirst()) return@withLock
			val content = c.getString(c.getColumnIndexOrThrow("content"))
			val ts = c.getLong(c.getColumnIndexOrThrow("ts"))
			val idHashColumn = c.getColumnIndexOrThrow("id_hash")
			val existingIdHash = if (c.isNull(idHashColumn)) null else c.getString(idHashColumn)
			val idHash = existingIdHash ?: generateIdHash(content, ts).also { generated ->
				val values = android.content.ContentValues().apply {
					put("id_hash", generated)
					put("origin_device", getSharedPreferences("clipmo_sync", Context.MODE_PRIVATE).getString("device_id", "local"))
					put("origin_wall_ms", ts)
				}
				db.update("items", values, "id=?", arrayOf(itemId.toString()))
			}
			val contentHash = contentHash(content)
			val version = SyncVersion(
				device_id = getSharedPreferences("clipmo_sync", Context.MODE_PRIVATE)
					.getString("device_id", "local")!!,
				lamport = lamport.getAndIncrement(),
				wall_ms = ts
			)
			val snapshot = ClipSnapshot(
				id_hash = idHash,
				kind = kind,
				preview = content.take(120),
				content = content,
				content_hash = contentHash,
				favorite = c.getInt(c.getColumnIndexOrThrow("fav")) != 0,
				tags = c.getColumnIndex("tags").takeIf { it >= 0 && !c.isNull(it) }
					?.let(c::getString)?.split(',')?.map(String::trim)?.filter(String::isNotEmpty).orEmpty(),
				live = live,
				copied_at = ts,
				version = version
			)
			val assetColumn = c.getColumnIndex("asset_paths")
			val assetPaths = if (assetColumn >= 0 && !c.isNull(assetColumn)) {
				c.getString(assetColumn).lineSequence().filter(String::isNotBlank).map(::File).toList()
			} else emptyList()
			val job = when (kind) {
				ItemKind.image -> buildImageJob(snapshot, assetPaths)
				ItemKind.files -> buildFilesJob(snapshot, assetPaths)
				else -> SyncJob(SyncBody.ClipUpsert(snapshot))
			}
			if (job != null) jobQueue.add(job)
		}
		db.close()
	}

	private suspend fun drainSendQueue(deviceId: String, deviceName: String, pairingCode: String) {
		while (running) {
			val job = syncMutex.withLock { if (jobQueue.isEmpty()) null else jobQueue.removeFirst() }
			if (job == null) {
				delay(500)
				continue
			}

			val currentDev = currentDevice(deviceId, deviceName)
			val envelope = SyncEnvelope(
				protocol = PROTOCOL,
				pairing_code = pairingCode,
				device = currentDev,
				tcp_port = listenPort,
				body = job.body
			)

			val targets = job.pendingPeers ?: peers.keys.toMutableSet().also { job.pendingPeers = it }
			if (targets.isEmpty()) {
				syncMutex.withLock { jobQueue.addFirst(job) }
				delay(1_000)
				continue
			}
			for (peerId in targets.toList()) {
				if (!running) break
				val peer = peers[peerId] ?: continue
				if (peer.address.address.isAnyLocalAddress) continue
				if (sendToPeer(peer.address, envelope, job.blobs)) {
					targets.remove(peerId)
					Log.i(
						"ClipSyncService",
						"sent ${job.body.javaClass.simpleName} version=${job.body.version().lamport} to ${peer.device.platform}",
					)
				}
			}
			if (targets.isNotEmpty()) {
				syncMutex.withLock { jobQueue.addFirst(job) }
				delay(1_000)
			}
		}
	}

	private suspend fun watchLocalChanges(deviceId: String) {
		val prefs = getSharedPreferences("clipmo_sync", Context.MODE_PRIVATE)
		var lastSeenId = prefs.getLong("sync_last_seen_id", 0L)
		while (running) {
			delay(1000L)
			if (!prefs.getBoolean("sync_enabled", false)) continue
			try {
				processMutationOutbox(deviceId)
				val db = openOrCreateDatabase("clipdeck.db", Context.MODE_PRIVATE, null)
				val c = db.query("items", null, "id > ?", arrayOf(lastSeenId.toString()),
					null, null, "id ASC")
				c.use { cur ->
					while (cur.moveToNext()) {
						val id = cur.getLong(cur.getColumnIndexOrThrow("id"))
						val syncStatus = cur.getString(cur.getColumnIndexOrThrow("sync_status"))
						val type = cur.getString(cur.getColumnIndexOrThrow("type"))
						val kind = when (type) {
							"URL" -> ItemKind.link
							"IMAGE" -> ItemKind.image
							"FILE" -> ItemKind.files
							else -> ItemKind.text
						}
						lastSeenId = maxOf(lastSeenId, id)
						if (syncStatus == "local") enqueueItem(id, kind)
					}
				}
				db.close()
				prefs.edit().putLong("sync_last_seen_id", lastSeenId).apply()
			} catch (error: Exception) {
				Log.w("ClipSyncService", "local sync watcher iteration failed", error)
			}
		}
	}

	private fun loadTrustedPeers() {
		val db = openOrCreateDatabase("clipdeck.db", Context.MODE_PRIVATE, null)
		db.rawQuery(
			"""
			SELECT device_id, name, platform, color, last_host, last_port, last_seen_ms
			FROM trusted_devices
			WHERE revoked=0 AND last_host IS NOT NULL AND last_port IS NOT NULL
			""".trimIndent(),
			null,
		).use { cursor ->
			while (cursor.moveToNext()) {
				val platform = runCatching { PlatformKind.valueOf(cursor.getString(2)) }.getOrDefault(PlatformKind.unknown)
				val device = DeviceIdentity(cursor.getString(0), cursor.getString(1), platform, cursor.getString(3))
				val address = InetSocketAddress(cursor.getString(4), cursor.getInt(5))
				peers[device.id] = PeerRecord(device, address, cursor.getLong(6))
			}
		}
		db.close()
	}

	private suspend fun processMutationOutbox(deviceId: String) {
		val db = openOrCreateDatabase("clipdeck.db", Context.MODE_PRIVATE, null)
		val processed = mutableListOf<Long>()
		db.query("sync_outbox", null, null, null, null, null, "id ASC", "100").use { cursor ->
			while (cursor.moveToNext()) {
				val rowId = cursor.getLong(cursor.getColumnIndexOrThrow("id"))
				val idHash = cursor.getString(cursor.getColumnIndexOrThrow("id_hash"))
				if (!validIdHash(idHash)) {
					processed += rowId
					continue
				}
				val version = SyncVersion(
					device_id = deviceId,
					lamport = lamport.incrementAndGet(),
					wall_ms = cursor.getLong(cursor.getColumnIndexOrThrow("created_ms")),
				)
				val body = when (cursor.getString(cursor.getColumnIndexOrThrow("mutation"))) {
					"favorite" -> SyncBody.FavoriteToggle(
						id_hash = idHash,
						favorite = cursor.getInt(cursor.getColumnIndexOrThrow("favorite")) != 0,
						version = version,
					)
					"tombstone" -> SyncBody.Tombstone(id_hash = idHash, version = version)
					"upsert" -> {
						var itemId: Long? = null
						var kind = ItemKind.text
						db.rawQuery("SELECT id, type FROM items WHERE id_hash=?", arrayOf(idHash)).use { item ->
							if (item.moveToFirst()) {
								itemId = item.getLong(0)
								kind = when (item.getString(1)) {
									"URL" -> ItemKind.link
									"IMAGE" -> ItemKind.image
									"FILE" -> ItemKind.files
									else -> ItemKind.text
								}
							}
						}
						itemId?.let { enqueueItem(it, kind, live = false) }
						null
					}
					else -> null
				}
				if (body != null) {
					syncMutex.withLock { jobQueue.add(SyncJob(body)) }
					Log.i("ClipSyncService", "queued local ${body.javaClass.simpleName} mutation")
					if (body is SyncBody.Tombstone) {
						val values = android.content.ContentValues().apply {
							put("id_hash", idHash)
							put("origin_device", deviceId)
							put("origin_lamport", version.lamport)
							put("origin_wall_ms", version.wall_ms)
						}
						db.insertWithOnConflict("sync_tombstones", null, values, SQLiteDatabase.CONFLICT_REPLACE)
					}
				}
				processed += rowId
			}
		}
		processed.forEach { db.delete("sync_outbox", "id=?", arrayOf(it.toString())) }
		db.close()
	}

	private suspend fun broadcastPresence(deviceId: String, deviceName: String) {
		var pairingCode = ""
		val prefs = getSharedPreferences("clipmo_sync", Context.MODE_PRIVATE)
		val dev = currentDevice(deviceId, deviceName)
		val socket = DatagramSocket()
		socket.broadcast = true
		while (running) {
			val code = prefs.getString("pairing_code", "") ?: ""
			if (code != pairingCode) pairingCode = code
			if (pairingCode.isNotEmpty() && prefs.getBoolean("sync_enabled", false)) {
				val msg = DiscoveryMessage(
					protocol = PROTOCOL,
					pairing_code = pairingCode,
					device = dev,
					tcp_port = listenPort
				)
				try {
					val bytes = mapper.writeValueAsBytes(msg)
					val packet = DatagramPacket(bytes, bytes.size,
						InetAddress.getByName("255.255.255.255"), DISCOVERY_PORT)
					socket.send(packet)
				} catch (e: Exception) {
					Log.d("ClipSyncService", "broadcast error: ${e.message}")
				}
			}
			delay(DISCOVERY_TICK_MS)
		}
		socket.close()
	}

	private val buf = ByteArray(4096)

	private suspend fun listenForPeers(myPairing: String, myDeviceId: String) =
		withContext(Dispatchers.IO) {
			try {
				val sock = DatagramSocket(null)
				sock.reuseAddress = true
				sock.broadcast = true
				sock.bind(InetSocketAddress(DISCOVERY_PORT))
				sock.soTimeout = 3000
				val packet = DatagramPacket(buf, buf.size)
				while (running) {
					try {
						sock.receive(packet)
						val data = String(packet.data, 0, packet.length, Charsets.UTF_8)
						val msg = try {
							mapper.readValue(data, DiscoveryMessage::class.java)
						} catch (_: Exception) { continue }
						if (msg.protocol != PROTOCOL) continue
						if (msg.pairing_code != myPairing) continue
						if (msg.device.id == myDeviceId) continue
						if (isRevoked(msg.device.id) && !pairingWindowOpen()) continue
						peers[msg.device.id] = PeerRecord(
							device = msg.device,
							address = InetSocketAddress(
								packet.address.hostAddress,
								msg.tcp_port
							),
							lastSeenAt = System.currentTimeMillis()
						)
						rememberTrustedDevice(msg.device, InetSocketAddress(packet.address.hostAddress, msg.tcp_port))
						lamport.updateAndGet { it + 1 }
					} catch (e: SocketTimeoutException) {
						// normal, continue loop
					} catch (e: Exception) {
						Log.d("ClipSyncService", "listenForPeers error: ${e.message}")
					}
				}
				sock.close()
			} catch (e: Exception) {
				Log.w("ClipSyncService", "listenForPeers fatal: ${e.message}")
			}
		}

	private fun applyClipUpsert(envelope: SyncEnvelope, clip: ClipSnapshot) {
		val db = openOrCreateDatabase("clipdeck.db", Context.MODE_PRIVATE, null)
		if (!shouldAccept(db, clip.id_hash, clip.version)) {
			db.close()
			return
		}
		val existing = db.rawQuery(
			"SELECT id FROM items WHERE id_hash=?",
			arrayOf(clip.id_hash)
		)
		existing.use { c ->
			if (c.moveToFirst()) {
				val existingId = c.getLong(0)
				val cv = android.content.ContentValues()
				cv.put("content", clip.content)
				cv.put("type", localType(clip.kind))
				cv.put("fav", if (clip.favorite) 1 else 0)
				cv.put("tags", clip.tags.joinToString(","))
				cv.put("id_hash", clip.id_hash)
				cv.put("origin_device", envelope.device.id)
				cv.put("origin_lamport", clip.version.lamport)
				cv.put("origin_wall_ms", clip.version.wall_ms)
				cv.put("sync_status", "synced")
				db.update("items", cv, "id=?", arrayOf(existingId.toString()))
				Unit
			} else {
				val cv = android.content.ContentValues()
				cv.put("content", clip.content)
				cv.put("type", localType(clip.kind))
				cv.put("ts", clip.copied_at)
				cv.put("source", envelope.device.name)
				cv.put("fav", if (clip.favorite) 1 else 0)
				cv.put("tags", clip.tags.joinToString(","))
				cv.put("id_hash", clip.id_hash)
				cv.put("origin_device", envelope.device.id)
				cv.put("origin_lamport", clip.version.lamport)
				cv.put("origin_wall_ms", clip.version.wall_ms)
				cv.put("sync_status", "synced")
				db.insert("items", null, cv)
			}
		}
		db.close()
		copyNewRemoteTextToClipboard(clip)
	}

	override fun onTimeout(startId: Int, fgsType: Int) {
		Log.w("ClipSyncService", "Android dataSync foreground-service time budget expired")
		running = false
		stopSelf(startId)
	}

	private fun applyClipEdit(body: SyncBody.ClipEdit) {
		val db = openOrCreateDatabase("clipdeck.db", Context.MODE_PRIVATE, null)
		if (!shouldAccept(db, body.id_hash, body.version)) {
			db.close()
			return
		}
		val c = db.rawQuery("SELECT id FROM items WHERE id_hash=?", arrayOf(body.id_hash))
		c.use { cur ->
			if (cur.moveToFirst()) {
				val id = cur.getLong(0)
				val cv = android.content.ContentValues()
				cv.put("content", body.content)
				cv.put("type", localType(body.kind))
				cv.put("origin_device", body.version.device_id)
				cv.put("origin_lamport", body.version.lamport)
				cv.put("origin_wall_ms", body.version.wall_ms)
				cv.put("sync_status", "synced")
				db.update("items", cv, "id=?", arrayOf(id.toString()))
			}
		}
		db.close()
	}

	private fun applyFavoriteToggle(body: SyncBody.FavoriteToggle) {
		val db = openOrCreateDatabase("clipdeck.db", Context.MODE_PRIVATE, null)
		if (!shouldAccept(db, body.id_hash, body.version)) {
			db.close()
			return
		}
		val c = db.rawQuery("SELECT id FROM items WHERE id_hash=?", arrayOf(body.id_hash))
		c.use { cur ->
			if (cur.moveToFirst()) {
				val id = cur.getLong(0)
				val cv = android.content.ContentValues()
				cv.put("fav", if (body.favorite) 1 else 0)
				cv.put("origin_device", body.version.device_id)
				cv.put("origin_lamport", body.version.lamport)
				cv.put("origin_wall_ms", body.version.wall_ms)
				cv.put("sync_status", "synced")
				db.update("items", cv, "id=?", arrayOf(id.toString()))
			}
		}
		db.close()
	}

	private fun applyTombstone(body: SyncBody.Tombstone) {
		val db = openOrCreateDatabase("clipdeck.db", Context.MODE_PRIVATE, null)
		if (!shouldAccept(db, body.id_hash, body.version)) {
			db.close()
			return
		}
		val c = db.rawQuery("SELECT id FROM items WHERE id_hash=?", arrayOf(body.id_hash))
		c.use { cur ->
			if (cur.moveToFirst()) {
				db.delete("items", "id=?", arrayOf(cur.getLong(0).toString()))
			}
		}
		val values = android.content.ContentValues().apply {
			put("id_hash", body.id_hash)
			put("origin_device", body.version.device_id)
			put("origin_lamport", body.version.lamport)
			put("origin_wall_ms", body.version.wall_ms)
		}
		db.insertWithOnConflict("sync_tombstones", null, values, SQLiteDatabase.CONFLICT_REPLACE)
		db.close()
	}

	private fun readFrameHeader(dis: DataInputStream): String? {
		try {
			val len = dis.readInt()
			if (len <= 0 || len > MAX_HEADER_BYTES) return null
			val buf = ByteArray(len)
			dis.readFully(buf)
			return String(buf, Charsets.UTF_8)
		} catch (e: Exception) {
			return null
		}
	}

	private fun applyImageUpsert(envelope: SyncEnvelope, body: SyncBody.ImageUpsert, input: DataInputStream) {
		val total = body.image.imageSize + body.image.thumbSize
		if (body.clip.kind != ItemKind.image || total <= 0 || total > MAX_IMAGE_BYTES) return
		if (body.image.chunkCount != chunkCount(total)) return
		val db = openOrCreateDatabase("clipdeck.db", Context.MODE_PRIVATE, null)
		if (!shouldAccept(db, body.clip.id_hash, body.clip.version)) {
			db.close()
			return
		}
		val imageBytes = readBlob(input, body.image.imageSize, MAX_IMAGE_BYTES) ?: run { db.close(); return }
		val thumbBytes = readBlob(input, body.image.thumbSize, MAX_IMAGE_BYTES) ?: run { db.close(); return }
		val directory = File(filesDir, "sync_assets/${safeComponent(body.clip.id_hash)}")
		val extension = safeExtension(body.image.extension)
		val imageFile = File(directory, "image.$extension")
		val thumbFile = File(directory, "thumb.$extension")
		if (!writeAtomic(imageFile, imageBytes) || !writeAtomic(thumbFile, thumbBytes)) {
			db.close()
			return
		}
		upsertRemoteClip(db, envelope.device, body.clip, "${imageFile.absolutePath}\n${thumbFile.absolutePath}", total)
		db.close()
	}

	private fun applyFilesUpsert(envelope: SyncEnvelope, body: SyncBody.FilesUpsert, input: DataInputStream) {
		if (body.clip.kind != ItemKind.files || body.files.isEmpty()) return
		val total = body.files.fold(0L) { sum, file -> if (Long.MAX_VALUE - sum < file.size) Long.MAX_VALUE else sum + file.size }
		if (total <= 0 || total > HARD_MAX_MESSAGE_BYTES || total > 100L * 1024 * 1024) return
		val db = openOrCreateDatabase("clipdeck.db", Context.MODE_PRIVATE, null)
		if (!shouldAccept(db, body.clip.id_hash, body.clip.version)) {
			db.close()
			return
		}
		val directory = File(filesDir, "sync_assets/${safeComponent(body.clip.id_hash)}/files")
		val stored = mutableListOf<String>()
		for (file in body.files) {
			val bytes = readBlob(input, file.size, HARD_MAX_MESSAGE_BYTES) ?: run { db.close(); return }
			if (file.size == 0L || file.size > 25L * 1024 * 1024 || file.chunkCount != chunkCount(file.size)) continue
			val name = safeFileName(file.name)
			if (!isAllowedFileName(name)) continue
			val target = File(directory, name)
			if (writeAtomic(target, bytes)) stored += target.absolutePath
		}
		if (stored.isNotEmpty()) upsertRemoteClip(db, envelope.device, body.clip, stored.joinToString("\n"), total)
		db.close()
	}

	private fun upsertRemoteClip(
		db: SQLiteDatabase,
		device: DeviceIdentity,
		clip: ClipSnapshot,
		assetPaths: String,
		sizeBytes: Long,
	) {
		val values = android.content.ContentValues().apply {
			put("content", clip.content)
			put("type", localType(clip.kind))
			put("ts", clip.copied_at)
			put("source", device.name)
			put("fav", if (clip.favorite) 1 else 0)
			put("tags", clip.tags.joinToString(","))
			put("id_hash", clip.id_hash)
			put("origin_device", device.id)
			put("origin_lamport", clip.version.lamport)
			put("origin_wall_ms", clip.version.wall_ms)
			put("size_bytes", sizeBytes)
			put("asset_paths", assetPaths)
			put("sync_status", "synced")
		}
		val existing = db.rawQuery("SELECT id FROM items WHERE id_hash=?", arrayOf(clip.id_hash))
		existing.use { cursor ->
			if (cursor.moveToFirst()) db.update("items", values, "id=?", arrayOf(cursor.getLong(0).toString()))
			else db.insertOrThrow("items", null, values)
		}
	}

	private fun readBlob(input: DataInputStream, size: Long, limit: Long): ByteArray? {
		if (size < 0 || size > limit || size > Int.MAX_VALUE) return null
		return try { ByteArray(size.toInt()).also(input::readFully) } catch (_: IOException) { null }
	}

	private fun chunkCount(size: Long): Int = ((size + CHUNK_SIZE - 1) / CHUNK_SIZE).coerceAtMost(Int.MAX_VALUE.toLong()).toInt()

	private fun safeComponent(value: String): String = value.map { if (it.isLetterOrDigit() || it == '-' || it == '_') it else '_' }.joinToString("").take(96).ifBlank { "unknown" }

	private fun safeExtension(value: String): String = value.lowercase().filter(Char::isLetterOrDigit).take(8).ifBlank { "bin" }

	private fun safeFileName(value: String): String {
		val base = value.replace('\\', '/').substringAfterLast('/').take(180)
		return base.map { if (it.isLetterOrDigit() || it in ".-_ ") it else '_' }.joinToString("").trim('.', ' ').ifBlank { "clipboard-file" }
	}

	private fun isAllowedFileName(name: String): Boolean {
		val blocked = setOf("exe", "bat", "cmd", "msi", "scr", "com", "cpl", "dll", "sys", "inf", "vbs", "js", "jse", "wsf", "ps1", "reg", "mp4", "mov", "avi", "mkv", "webm", "flv", "wmv", "iso", "vhd", "vhdx", "img", "dmg", "zip", "rar", "7z", "tar", "gz")
		return name.substringAfterLast('.', "").lowercase() !in blocked
	}

	private fun writeAtomic(target: File, bytes: ByteArray): Boolean {
		return try {
			target.parentFile?.mkdirs()
			val temporary = File(target.parentFile, "${target.name}.part")
			temporary.outputStream().use { it.write(bytes) }
			if (target.exists() && !target.delete()) return false
			if (!temporary.renameTo(target)) {
				temporary.delete()
				false
			} else true
		} catch (_: IOException) { false }
	}

	private fun buildImageJob(snapshot: ClipSnapshot, assets: List<File>): SyncJob? {
		if (assets.size < 2 || assets.take(2).any { !it.isFile }) return null
		val image = assets[0]
		val thumb = assets[1]
		val total = image.length() + thumb.length()
		if (total <= 0 || total > MAX_IMAGE_BYTES) return null
		val bounds = android.graphics.BitmapFactory.Options().apply { inJustDecodeBounds = true }
		android.graphics.BitmapFactory.decodeFile(image.absolutePath, bounds)
		return SyncJob(
			body = SyncBody.ImageUpsert(
				snapshot,
				ImageSnapshot(
					extension = image.extension.ifBlank { "png" },
					width = bounds.outWidth.coerceAtLeast(0),
					height = bounds.outHeight.coerceAtLeast(0),
					imageSize = image.length(),
					thumbSize = thumb.length(),
					chunkCount = chunkCount(total),
				),
			),
			blobs = listOf(image, thumb),
			estimatedBytes = total + 8 * 1024,
		)
	}

	private fun buildFilesJob(snapshot: ClipSnapshot, assets: List<File>): SyncJob? {
		val allowed = assets.filter { it.isFile && it.length() in 1..(25L * 1024 * 1024) && isAllowedFileName(it.name) }
		val bounded = mutableListOf<File>()
		var total = 0L
		for (file in allowed) {
			if (total + file.length() > 100L * 1024 * 1024) break
			total += file.length()
			bounded += file
		}
		if (bounded.isEmpty()) return null
		return SyncJob(
			body = SyncBody.FilesUpsert(
				snapshot,
				bounded.map { FileSnapshot(it.name, it.length(), mimeForName(it.name), chunkCount(it.length())) },
			),
			blobs = bounded,
			estimatedBytes = total + 16 * 1024,
		)
	}

	private fun mimeForName(name: String): String = when (name.substringAfterLast('.', "").lowercase()) {
		"txt", "md", "csv", "log" -> "text/plain"
		"pdf" -> "application/pdf"
		"png" -> "image/png"
		"jpg", "jpeg" -> "image/jpeg"
		"webp" -> "image/webp"
		"json" -> "application/json"
		else -> "application/octet-stream"
	}

	private fun localType(kind: ItemKind): String = when (kind) {
		ItemKind.link -> "URL"
		ItemKind.image -> "IMAGE"
		ItemKind.files -> "FILE"
		else -> "TEXT"
	}

	private fun copyNewRemoteTextToClipboard(clip: ClipSnapshot) {
		if (clip.kind !in setOf(ItemKind.text, ItemKind.link, ItemKind.email, ItemKind.color)) return
		val preferences = getSharedPreferences("clipmo_sync", Context.MODE_PRIVATE)
		val lastCopiedAt = preferences.getLong(KEY_LAST_REMOTE_CLIPBOARD_AT, 0L)
		if (!shouldCopyRemoteClipToSystemClipboard(
			clip.live,
			preferences.getBoolean(KEY_COPY_LIVE_SYNC_TO_CLIPBOARD, false),
			clip.copied_at,
			lastCopiedAt,
			System.currentTimeMillis(),
		)) return
		preferences.edit()
			.putLong(KEY_LAST_REMOTE_CLIPBOARD_AT, clip.copied_at)
			.putString("clipboard_suppression_hash", clip.content_hash)
			.apply()
		val clipboard = getSystemService(Context.CLIPBOARD_SERVICE) as android.content.ClipboardManager
		clipboard.setPrimaryClip(android.content.ClipData.newPlainText("Clipmo sync", clip.content))
	}

	private fun validIdHash(value: String): Boolean =
		value.length in 16..128 && value.all { it.isLetterOrDigit() || it == '-' || it == '_' }

	private fun rememberTrustedDevice(device: DeviceIdentity, address: InetSocketAddress) {
		val db = openOrCreateDatabase("clipdeck.db", Context.MODE_PRIVATE, null)
		val now = System.currentTimeMillis()
		db.execSQL(
			"""
			INSERT INTO trusted_devices(
				device_id, name, platform, color, last_host, last_port, last_seen_ms, trusted_at_ms, revoked
			) VALUES(?, ?, ?, ?, ?, ?, ?, ?, 0)
			ON CONFLICT(device_id) DO UPDATE SET
				name=excluded.name, platform=excluded.platform, color=excluded.color,
				last_host=excluded.last_host, last_port=excluded.last_port,
				last_seen_ms=excluded.last_seen_ms, revoked=0
			""".trimIndent(),
			arrayOf(device.id, device.name, device.platform.name, device.color, address.hostString, address.port, now, now),
		)
		db.close()
	}

	private fun isRevoked(deviceId: String): Boolean {
		val db = openOrCreateDatabase("clipdeck.db", Context.MODE_PRIVATE, null)
		val revoked = db.rawQuery(
			"SELECT revoked FROM trusted_devices WHERE device_id=?",
			arrayOf(deviceId),
		).use { cursor -> cursor.moveToFirst() && cursor.getInt(0) != 0 }
		db.close()
		return revoked
	}

	private fun pairingWindowOpen(): Boolean =
		getSharedPreferences("clipmo_sync", Context.MODE_PRIVATE)
			.getLong("pairing_until", 0L) > System.currentTimeMillis()

	private fun shouldAccept(db: SQLiteDatabase, idHash: String, incoming: SyncVersion): Boolean {
		val versions = mutableListOf<SyncVersion>()
		db.rawQuery(
			"SELECT origin_device, origin_lamport, origin_wall_ms FROM items WHERE id_hash=?",
			arrayOf(idHash),
		).use { cursor ->
			if (cursor.moveToFirst()) versions += SyncVersion(cursor.getString(0).orEmpty(), cursor.getLong(1), cursor.getLong(2))
		}
		db.rawQuery(
			"SELECT origin_device, origin_lamport, origin_wall_ms FROM sync_tombstones WHERE id_hash=?",
			arrayOf(idHash),
		).use { cursor ->
			if (cursor.moveToFirst()) versions += SyncVersion(cursor.getString(0).orEmpty(), cursor.getLong(1), cursor.getLong(2))
		}
		val current = versions.maxWithOrNull(compareBy<SyncVersion>({ it.lamport }, { it.wall_ms }, { it.device_id }))
		return current == null || compareValuesBy(incoming, current, { it.lamport }, { it.wall_ms }, { it.device_id }) > 0
	}

	fun getPeers(): List<DeviceIdentity> = peers.values.map { it.device }
	fun isRunning(): Boolean = running
}
