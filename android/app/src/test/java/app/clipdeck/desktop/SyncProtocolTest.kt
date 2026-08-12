package app.clipdeck.desktop

import com.fasterxml.jackson.module.kotlin.jacksonObjectMapper
import com.fasterxml.jackson.module.kotlin.registerKotlinModule
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class SyncProtocolTest {
    private val mapper = jacksonObjectMapper().registerKotlinModule()

    @Test
    fun androidClipUpsertUsesRustCamelCaseWireShape() {
        val envelope = SyncEnvelope(
            protocol = PROTOCOL,
            pairing_code = "123456",
            device = DeviceIdentity("android-device", "Pixel", PlatformKind.android, "#78F13D"),
            tcp_port = 47634,
            body = SyncBody.ClipUpsert(
                ClipSnapshot(
                    id_hash = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                    kind = ItemKind.text,
                    preview = "hello",
                    content = "hello",
                    content_hash = "2cf24dba5fb0a30e26e83b2ac5b9e29e",
                    favorite = false,
                    copied_at = 1_700_000_000_000,
                    version = SyncVersion("android-device", 42, 1_700_000_000_000),
                ),
            ),
        )

        val tree = mapper.readTree(mapper.writeValueAsBytes(envelope))
        assertEquals("123456", tree["pairingCode"].asText())
        assertEquals(47634, tree["tcpPort"].asInt())
        assertFalse(tree.has("pairing_code"))
        assertEquals("clipUpsert", tree["body"]["type"].asText())
        assertEquals("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", tree["body"]["clip"]["idHash"].asText())
        assertEquals("android-device", tree["body"]["clip"]["version"]["deviceId"].asText())
    }

    @Test
    fun androidDeserializesRustClipUpsertGoldenJson() {
        val json = """
            {
              "protocol":"clipmo-lan-v2",
              "pairingCode":"123456",
              "device":{"id":"windows-device","name":"Desktop","platform":"windows","color":"#1677FF"},
              "tcpPort":47634,
              "body":{
                "type":"clipUpsert",
                "clip":{
                  "idHash":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                  "kind":"text",
                  "preview":"from Windows",
                  "content":"from Windows",
                  "contentHash":"11111111111111111111111111111111",
                  "favorite":true,
                  "copiedAt":1700000000001,
                  "version":{"deviceId":"windows-device","lamport":7,"wallMs":1700000000001}
                }
              }
            }
        """.trimIndent()

        val envelope = mapper.readValue(json, SyncEnvelope::class.java)
        assertEquals("windows-device", envelope.device.id)
        assertTrue(envelope.body is SyncBody.ClipUpsert)
        val clip = (envelope.body as SyncBody.ClipUpsert).clip
        assertEquals("from Windows", clip.content)
        assertEquals("windows-device", clip.version.device_id)
        assertFalse(clip.live)
    }

    @Test
    fun systemClipboardAutoCopyRequiresFreshExplicitLiveDelivery() {
        val now = 1_700_000_000_000
        assertFalse(shouldCopyRemoteClipToSystemClipboard(false, true, now, 0, now))
        assertFalse(shouldCopyRemoteClipToSystemClipboard(true, false, now, 0, now))
        assertFalse(
            shouldCopyRemoteClipToSystemClipboard(
                true,
                true,
                now - LIVE_CLIP_MAX_AGE_MS - 1,
                0,
                now,
            ),
        )
        assertFalse(shouldCopyRemoteClipToSystemClipboard(true, true, now, now, now))
        assertTrue(shouldCopyRemoteClipToSystemClipboard(true, true, now, now - 1, now))
    }

    @Test
    fun binaryAndMutationBodiesUseRustNames() {
        val clip = ClipSnapshot(
            id_hash = "cccccccccccccccccccccccccccccccc",
            kind = ItemKind.image,
            preview = "Image",
            content = "Image",
            content_hash = "dddddddddddddddddddddddddddddddd",
            favorite = false,
            copied_at = 10,
            version = SyncVersion("android-device", 8, 10),
        )
        val image = mapper.valueToTree<com.fasterxml.jackson.databind.JsonNode>(
            SyncBody.ImageUpsert(clip, ImageSnapshot("png", 320, 200, 100, 20, 1)),
        )
        assertEquals("imageUpsert", image["type"].asText())
        assertEquals(100, image["image"]["imageSize"].asLong())
        assertEquals(20, image["image"]["thumbSize"].asLong())
        assertEquals(1, image["image"]["chunkCount"].asInt())

        val edit = mapper.valueToTree<com.fasterxml.jackson.databind.JsonNode>(
            SyncBody.ClipEdit(
                "cccccccccccccccccccccccccccccccc",
                ItemKind.text,
                "edited",
                "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
                SyncVersion("android-device", 9, 11),
            ),
        )
        assertEquals("clipEdit", edit["type"].asText())
        assertEquals("cccccccccccccccccccccccccccccccc", edit["idHash"].asText())
        assertEquals("android-device", edit["version"]["deviceId"].asText())

        val favorite = mapper.valueToTree<com.fasterxml.jackson.databind.JsonNode>(
            SyncBody.FavoriteToggle(
                "cccccccccccccccccccccccccccccccc",
                true,
                SyncVersion("android-device", 10, 12),
            ),
        )
        assertEquals("favoriteToggle", favorite["type"].asText())

        val tombstone = mapper.valueToTree<com.fasterxml.jackson.databind.JsonNode>(
            SyncBody.Tombstone(
                "cccccccccccccccccccccccccccccccc",
                SyncVersion("android-device", 11, 13),
            ),
        )
        assertEquals("tombstone", tombstone["type"].asText())
    }
}
