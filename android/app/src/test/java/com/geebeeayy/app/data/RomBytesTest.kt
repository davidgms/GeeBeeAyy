package com.geebeeayy.app.data

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import java.io.File
import java.util.zip.ZipEntry
import java.util.zip.ZipOutputStream

/**
 * Reading a cart out of a zip. `java.util.zip` is on the JVM as well as on
 * Android, so this runs for real rather than against a mock.
 */
class RomBytesTest {

    @get:Rule
    val folder = TemporaryFolder()

    private fun zipOf(vararg entries: Pair<String, ByteArray>): File {
        val file = folder.newFile("archive.zip")
        ZipOutputStream(file.outputStream()).use { out ->
            entries.forEach { (name, bytes) ->
                out.putNextEntry(ZipEntry(name))
                out.write(bytes)
                out.closeEntry()
            }
        }
        return file
    }

    @Test
    fun `a plain rom is read as it is`() {
        val rom = folder.newFile("game.gba")
        rom.writeBytes(byteArrayOf(1, 2, 3))
        assertArrayEquals(byteArrayOf(1, 2, 3), RomBytes.read(rom.path))
    }

    @Test
    fun `a zipped rom is pulled out`() {
        val zip = zipOf("game.gba" to byteArrayOf(9, 8, 7))
        assertArrayEquals(byteArrayOf(9, 8, 7), RomBytes.read(zip.path))
    }

    @Test
    fun `the cart is found past files that are not one`() {
        val zip = zipOf(
            "readme.txt" to byteArrayOf(0),
            "cover.png" to byteArrayOf(0),
            "game.agb" to byteArrayOf(4, 5),
        )
        assertArrayEquals(byteArrayOf(4, 5), RomBytes.read(zip.path))
    }

    /**
     * A zip with several carts must pick the same one every launch: save
     * states are keyed on the cart, so an arbitrary order would key them on a
     * different game between runs.
     */
    @Test
    fun `several carts pick the same one every time`() {
        val zip = zipOf(
            "zelda.gba" to byteArrayOf(2),
            "alpha.gba" to byteArrayOf(1),
        )
        assertArrayEquals(byteArrayOf(1), RomBytes.read(zip.path))
        assertArrayEquals(byteArrayOf(1), RomBytes.read(zip.path))
    }

    @Test
    fun `a zip with no cart reports nothing rather than throwing`() {
        val zip = zipOf("readme.txt" to byteArrayOf(0))
        assertNull(RomBytes.read(zip.path))
    }

    @Test
    fun `a directory entry is not mistaken for a cart`() {
        val zip = zipOf("roms.gba/" to ByteArray(0), "roms.gba/game.gba" to byteArrayOf(7))
        assertArrayEquals(byteArrayOf(7), RomBytes.read(zip.path))
    }

    @Test
    fun `zip detection ignores case`() {
        assertTrue(RomBytes.isZip("/a/b/Game.ZIP"))
        assertTrue(RomBytes.isZip("/a/b/game.zip"))
        assertFalse(RomBytes.isZip("/a/b/game.gba"))
    }

    /** Saves are keyed on the archive, not the entry: the file is what a
     *  player renames and moves. */
    @Test
    fun `the display name comes from the archive`() {
        assertEquals("Sonic Advance", RomBytes.displayBaseName("/x/Sonic Advance.zip"))
        assertEquals("Sonic Advance", RomBytes.displayBaseName("/x/Sonic Advance.gba"))
    }
}
