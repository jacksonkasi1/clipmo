package app.clipdeck.desktop

import android.util.Base64
import com.fasterxml.jackson.databind.ObjectMapper
import java.net.HttpURLConnection
import java.net.URL
import java.net.URLEncoder
import java.security.MessageDigest
import java.security.SecureRandom
import javax.crypto.Cipher
import javax.crypto.SecretKeyFactory
import javax.crypto.spec.GCMParameterSpec
import javax.crypto.spec.PBEKeySpec
import javax.crypto.spec.SecretKeySpec

/**
 * Optional end-to-end-encrypted relay mailbox owned by the user (their own HTTP
 * endpoint such as the Cloudflare Worker in `relay/`, or their own Firebase Realtime
 * Database). Wire format and key derivation match the desktop app exactly.
 */
data class RelayConfig(
	val enabled: Boolean,
	val backend: String,
	val url: String,
	val secret: String,
	val keyHex: String,
	val channel: String,
)

data class MailItem(val id: String, val device: String, val blob: String)

object RelayCrypto {
	private const val ROUNDS = 200_000
	private val SALT = "clipmo-relay-v1".toByteArray()

	/** Returns (key hex, channel). */
	fun derive(passphrase: String): Pair<String, String> {
		val key = SecretKeyFactory.getInstance("PBKDF2WithHmacSHA256")
			.generateSecret(PBEKeySpec(passphrase.toCharArray(), SALT, ROUNDS, 256)).encoded
		val digest = MessageDigest.getInstance("SHA-256")
		digest.update(key)
		digest.update("clipmo-relay-channel".toByteArray())
		val channel = digest.digest().take(16).joinToString("") { "%02x".format(it) }
		return key.joinToString("") { "%02x".format(it) } to channel
	}

	private fun keyBytes(hex: String) = ByteArray(hex.length / 2) { hex.substring(it * 2, it * 2 + 2).toInt(16).toByte() }

	fun encrypt(keyHex: String, plain: ByteArray): String {
		val nonce = ByteArray(12).also { SecureRandom().nextBytes(it) }
		val cipher = Cipher.getInstance("AES/GCM/NoPadding")
		cipher.init(Cipher.ENCRYPT_MODE, SecretKeySpec(keyBytes(keyHex), "AES"), GCMParameterSpec(128, nonce))
		return Base64.encodeToString(nonce + cipher.doFinal(plain), Base64.NO_WRAP)
	}

	fun decrypt(keyHex: String, blob: String): ByteArray? = runCatching {
		val bytes = Base64.decode(blob, Base64.DEFAULT)
		require(bytes.size > 12)
		val cipher = Cipher.getInstance("AES/GCM/NoPadding")
		cipher.init(Cipher.DECRYPT_MODE, SecretKeySpec(keyBytes(keyHex), "AES"), GCMParameterSpec(128, bytes.copyOfRange(0, 12)))
		cipher.doFinal(bytes, 12, bytes.size - 12)
	}.getOrNull()

	fun newItemId(): String =
		"%016d-%s".format(System.currentTimeMillis(), ByteArray(4).also { SecureRandom().nextBytes(it) }.joinToString("") { "%02x".format(it) })
}

class RelayClient(private val mapper: ObjectMapper) {
	private fun open(url: String, method: String, bearer: String? = null): HttpURLConnection =
		(URL(url).openConnection() as HttpURLConnection).apply {
			requestMethod = method
			connectTimeout = 10_000
			readTimeout = 15_000
			if (bearer != null) setRequestProperty("Authorization", "Bearer $bearer")
		}

	private fun enc(value: String) = URLEncoder.encode(value, "UTF-8")

	fun push(config: RelayConfig, item: MailItem) {
		val base = config.url.trimEnd('/')
		val firebase = config.backend == "firebase"
		val conn = if (firebase) open("$base/clipmo/${config.channel}/${item.id}.json?auth=${enc(config.secret)}", "PUT")
		else open("$base/v1/${config.channel}", "POST", config.secret)
		conn.doOutput = true
		conn.setRequestProperty("Content-Type", "application/json")
		val body = if (firebase) mapper.writeValueAsBytes(mapOf("d" to item.device, "b" to item.blob))
		else mapper.writeValueAsBytes(item)
		conn.outputStream.use { it.write(body) }
		check(conn.responseCode in 200..299) { "relay returned HTTP ${conn.responseCode}" }
		conn.disconnect()
	}

	fun pull(config: RelayConfig, after: String): List<MailItem> {
		val base = config.url.trimEnd('/')
		val items: List<MailItem>
		if (config.backend == "firebase") {
			val url = "$base/clipmo/${config.channel}.json?auth=${enc(config.secret)}&orderBy=${enc("\"\$key\"")}&startAt=${enc("\"$after\"")}&limitToFirst=200"
			val conn = open(url, "GET")
			check(conn.responseCode in 200..299) { "relay returned HTTP ${conn.responseCode}" }
			val tree = conn.inputStream.use { mapper.readTree(it) }
			conn.disconnect()
			items = tree.fields().asSequence().mapNotNull { (id, entry) ->
				val d = entry.get("d")?.asText() ?: return@mapNotNull null
				val b = entry.get("b")?.asText() ?: return@mapNotNull null
				MailItem(id, d, b)
			}.toList()
		} else {
			val conn = open("$base/v1/${config.channel}?after=${enc(after)}", "GET", config.secret)
			check(conn.responseCode in 200..299) { "relay returned HTTP ${conn.responseCode}" }
			val tree = conn.inputStream.use { mapper.readTree(it) }
			conn.disconnect()
			items = tree.get("items").map { MailItem(it.get("id").asText(), it.get("device").asText(), it.get("blob").asText()) }
		}
		return items.sortedBy { it.id }.filter { it.id > after }
	}
}
