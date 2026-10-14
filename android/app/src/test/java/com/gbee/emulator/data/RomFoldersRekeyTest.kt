package com.gbee.emulator.data

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/** Which folders a picked tree covers, and how times and stars follow their games. */
class RomFoldersRekeyTest {

    private val keys = listOf(
        "/sdcard/Roms/a.gba",
        "/storage/emulated/0/Roms/Sub/b.zip",
        "/sdcard/Roms2/c.gba",
        "/sdcard/Other/d.gba",
        "content://x/doc",
    )

    private fun rekey(root: String) = rekeyUnder(root, keys) { "tree:$it" }

    @Test
    fun `an equal tree covers its games and nothing beside them`() {
        assertEquals(
            mapOf(
                "/sdcard/Roms/a.gba" to "tree:primary:Roms/a.gba",
                "/storage/emulated/0/Roms/Sub/b.zip" to "tree:primary:Roms/Sub/b.zip",
            ),
            rekey("primary:Roms"),
        )
    }

    @Test
    fun `a parent tree covers every game under it, whichever folders are configured`() {
        assertEquals(
            setOf(
                "/sdcard/Roms/a.gba",
                "/storage/emulated/0/Roms/Sub/b.zip",
                "/sdcard/Roms2/c.gba",
                "/sdcard/Other/d.gba",
            ),
            rekey("primary:").keys,
        )
    }

    @Test
    fun `a child tree covers only its own games`() {
        assertEquals(setOf("/storage/emulated/0/Roms/Sub/b.zip"), rekey("primary:Roms/Sub").keys)
    }

    @Test
    fun `containment needs a slash boundary`() {
        assertTrue(docIdContains("primary:Roms", "primary:Roms"))
        assertTrue(docIdContains("primary:Roms", "primary:Roms/a"))
        assertFalse(docIdContains("primary:Roms", "primary:Roms2/a"))
        assertTrue(docIdContains("primary:", "primary:Roms/a"))
        assertFalse(docIdContains("primary:Roms", "1A2B:Roms/a"))
    }

    @Test
    fun `two spellings of one path keep the later time`() {
        val map = mapOf("/sdcard/Roms/a.gba" to "N", "/storage/emulated/0/Roms/a.gba" to "N")
        val times = mapOf("/sdcard/Roms/a.gba" to 5L, "/storage/emulated/0/Roms/a.gba" to 9L)
        assertEquals(mapOf("N" to 9L), rekeyedTimes(times, map))
        assertEquals(mapOf("N" to 9L), rekeyedTimes(times.mapValues { 14L - it.value }, map))
    }

    @Test
    fun `two spellings of one path are one star`() {
        val map = mapOf("/sdcard/Roms/a.gba" to "N", "/storage/emulated/0/Roms/a.gba" to "N")
        val out = rekeyedSet(setOf("/sdcard/Roms/a.gba", "/storage/emulated/0/Roms/a.gba", "/keep"), map)
        assertEquals(setOf("N", "/keep"), out)
    }
}
