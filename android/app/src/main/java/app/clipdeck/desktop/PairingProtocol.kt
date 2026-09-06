package app.clipdeck.desktop

import java.net.URI
import java.net.URLDecoder
import java.security.SecureRandom

data class PairingInvite(val code: String, val deviceId: String? = null, val address: String? = null)

internal fun parsePairingInvite(value: String): PairingInvite? {
    val text = value.trim()
    if (text.matches(Regex("[0-9]{6}"))) return PairingInvite(text)
    return runCatching {
        val uri = URI(text)
        require(uri.scheme == "clipmo" && uri.host == "pair")
        val fields = uri.rawQuery.orEmpty().split("&").associate {
            val parts = it.split("=", limit = 2)
            parts[0] to URLDecoder.decode(parts.getOrElse(1) { "" }, "UTF-8")
        }
        require(fields["v"] == "3")
        val code = fields["code"].orEmpty()
        val id = fields["device"].orEmpty()
        require(code.matches(Regex("[0-9]{6}")) && id.isNotBlank() && id.length <= 128)
        val address = fields["address"]
        if (address != null) require(validPairingAddress(address))
        PairingInvite(code, id, address)
    }.getOrNull()
}

internal fun newPairingCode(previous: String): String {
    val random = SecureRandom()
    while (true) {
        val code = random.nextInt(1_000_000).toString().padStart(6, '0')
        if (code != previous) return code
    }
}

internal fun validPeerToken(token: String): Boolean = token.matches(Regex("[a-fA-F0-9]{64}"))

internal fun validPairingAddress(value: String): Boolean {
    val parts = value.split(":")
    if (parts.size != 2 || parts[1].toIntOrNull() !in 47634..47644) return false
    val octets = parts[0].split(".").map { it.toIntOrNull() ?: return false }
    return octets.size == 4 && octets.all { it in 0..255 } &&
        (octets[0] == 10 || (octets[0] == 172 && octets[1] in 16..31) ||
            (octets[0] == 192 && octets[1] == 168) || (octets[0] == 169 && octets[1] == 254))
}
