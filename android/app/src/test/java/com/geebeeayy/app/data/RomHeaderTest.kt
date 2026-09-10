package com.geebeeayy.app.data

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/**
 * The state key decides the filename every save state and battery save is
 * written to. Changing how it is built orphans every save already on disk, so
 * these cases pin the exact output rather than describing it.
 */
class RomHeaderTest {

    private fun cart(title: String, code: String): ByteArray {
        val bytes = ByteArray(RomHeader.MIN_BYTES)
        title.toByteArray(Charsets.US_ASCII).copyInto(bytes, 0xA0)
        code.toByteArray(Charsets.US_ASCII).copyInto(bytes, 0xAC)
        return bytes
    }

    @Test
    fun `title and game code are read from their header offsets`() {
        val header = RomHeader.from(cart("YGGDRA UNION", "BYUE"))!!
        assertEquals("YGGDRA UNION", header.title)
        assertEquals("BYUE", header.gameCode)
    }

    @Test
    fun `the key joins title and code and keeps only safe characters`() {
        assertEquals("YGGDRA_UNION_BYUE", RomHeader.from(cart("YGGDRA UNION", "BYUE"))!!.stateKey())
    }

    @Test
    fun `a cart with no title still keys on its code`() {
        assertEquals("BYUE", RomHeader.from(cart("", "BYUE"))!!.stateKey())
    }

    @Test
    fun `a cart with no code still keys on its title`() {
        assertEquals("FANTASY_KNIT", RomHeader.from(cart("Fantasy Knit", ""))!!.stateKey())
    }

    @Test
    fun `a cart with neither falls back to a fixed name`() {
        // Upper-cased like every other key, which is why the device has a
        // `ROM_slot0.state` sitting next to the real ones.
        assertEquals("ROM", RomHeader.from(cart("", ""))!!.stateKey())
    }

    @Test
    fun `a file too short to hold a header is not one`() {
        assertNull(RomHeader.from(ByteArray(0xA0)))
    }
}
