package app.clipdeck.desktop

import com.fasterxml.jackson.module.kotlin.jacksonObjectMapper
import org.junit.Assert.*
import org.junit.Test

class PairingProtocolTest {
    @Test fun qrDirectAddressAcceptsLanEndpointsAndRejectsOtherTargets() {
        val prefix = "clipmo://pair?v=3&device=mac-1&code=012345&address="
        assertEquals(PairingInvite("012345", address = "192.168.1.16:47634"), parsePairingInvite("012345@192.168.1.16:47634"))
        assertEquals("192.168.1.16:47634", parsePairingInvite(prefix + "192.168.1.16%3A47634")?.address)
        for (invalid in listOf("127.0.0.1:47634", "example.com:47634", "8.8.8.8:47634", "192.168.1.16:80", "192.168.999.1:47634")) {
            assertNull(parsePairingInvite(prefix + invalid))
        }
    }

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
