package app.clipdeck.desktop

import java.net.URI
import java.net.URLDecoder
import java.security.SecureRandom

data class PairingInvite(val code: String, val deviceId: String? = null)

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
        PairingInvite(code, id)
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
