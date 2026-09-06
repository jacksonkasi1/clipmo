package app.clipdeck.desktop

import com.fasterxml.jackson.module.kotlin.jacksonObjectMapper
import org.junit.Assert.*
import org.junit.Test

class PairingProtocolTest {
    @Test fun acceptsCodesAndVersionedQrInvitesWithoutChangingTheirMeaning() {
        assertEquals(PairingInvite("012345"), parsePairingInvite("012345"))
        assertEquals(PairingInvite("012345", "mac-1"), parsePairingInvite("clipmo://pair?v=3&device=mac-1&code=012345"))
        assertNull(parsePairingInvite("https://other.test/?code=012345"))
        assertNull(parsePairingInvite("clipmo://pair?v=2&device=mac-1&code=012345"))
        assertNull(parsePairingInvite("clipmo://pair?v=3&code=012345"))
        assertNull(parsePairingInvite("1234567"))
        assertNull(parsePairingInvite("１２３４５６"))
    }

    @Test fun regenerationChangesImmediatelyAndPreservesLeadingZeros() {
        var previous = "000000"
        repeat(100) {
            val next = newPairingCode(previous)
            assertNotEquals(previous, next)
            assertTrue(next.matches(Regex("[0-9]{6}")))
            previous = next
        }
    }

    @Test fun controlFramesMatchDesktopWireContract() {
        val mapper = jacksonObjectMapper()
        val token = "a".repeat(64)
        val json = """{"protocol":"clipmo-lan-v3","kind":"pair","device":{"id":"mac","name":"Mac","platform":"macos","color":"#123456"},"tcpPort":47634,"code":"012345","token":"$token"}"""
        val request = mapper.readValue(json, PairControl::class.java)
        assertEquals(47634, request.tcp_port)
        assertEquals("012345", request.code)
        val serialized = mapper.readTree(mapper.writeValueAsBytes(request))
        assertTrue(serialized.has("tcpPort"))
        assertFalse(serialized.has("tcp_port"))
        assertTrue(validPeerToken(token))
        assertFalse(validPeerToken("012345"))
        val reply = mapper.readValue("""{"ok":true,"device":{"id":"mac","name":"Mac","platform":"macos","color":"#123456"},"token":"$token"}""", PairReply::class.java)
        assertTrue(reply.ok)
        assertEquals(token, reply.token)
    }
}
