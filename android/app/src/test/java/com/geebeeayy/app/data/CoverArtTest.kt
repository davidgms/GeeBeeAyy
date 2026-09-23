package com.geebeeayy.app.data

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The two pieces of cover fetching that can be checked without a device: the
 * index parser and the URL it builds.
 *
 * Both matter more than they look. A mis-parsed index or a mis-encoded name
 * does not fail loudly - it fetches **somebody else's box art** and shows it
 * as though it were yours.
 */
class CoverArtTest {

    @Test
    fun `the index parses the shape the generator writes`() {
        val index = CoverArt.parseIndex(
            """{
"BJBE":"007 - Everything or Nothing (USA, Europe) (En,Fr,De)",
"BTME":"Mario Tennis - Power Tour (USA) (En,Fr,De,Es,It) (Virtual Console)"
}"""
        )
        assertEquals(2, index.size)
        assertEquals("007 - Everything or Nothing (USA, Europe) (En,Fr,De)", index["BJBE"])
        assertTrue(index.getValue("BTME").startsWith("Mario Tennis - Power Tour"))
    }

    /** No-Intro names carry apostrophes, commas, brackets and plus signs. */
    @Test
    fun `an escaped name survives the parse`() {
        val index = CoverArt.parseIndex("""{"AXYE":"Yu-Gi-Oh! \"The Duel\" (USA)"}""")
        assertEquals("""Yu-Gi-Oh! "The Duel" (USA)""", index["AXYE"])
    }

    @Test
    fun `a truncated index does not throw`() {
        assertEquals(emptyMap<String, String>(), CoverArt.parseIndex(""))
        assertEquals(emptyMap<String, String>(), CoverArt.parseIndex("{"))
        assertEquals(mapOf("BTME" to "x"), CoverArt.parseIndex("""{"BTME":"x","BJBE"""))
    }

    /**
     * The server wants path encoding. `URLEncoder` does form encoding, which
     * writes a space as `+` and leaves `*` alone - both of which 404.
     */
    @Test
    fun `a space becomes percent twenty, not a plus`() {
        val url = CoverArt.urlFor("Mario Tennis - Power Tour (USA, Australia) (En,Fr,De,Es,It)")
        assertTrue("got $url", url.startsWith("https://thumbnails.libretro.com/"))
        assertTrue("got $url", url.endsWith(".png"))
        assertTrue("space was not encoded: $url", !url.contains("+"))
        assertTrue("got $url", url.contains("Mario%20Tennis"))
    }

    @Test
    fun `an apostrophe and brackets are encoded`() {
        val url = CoverArt.urlFor("Yggdra Union - We'll Never Fight Alone (USA)")
        assertTrue("got $url", url.contains("We%27ll"))
        assertTrue("got $url", url.contains("%28USA%29"))
    }
}
