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

    private fun read(zip: File) =
        java.util.zip.ZipFile(zip).use { z -> RomBytes.cartIn(z) { it.readCapped(RomBytes.MAX_ROM_BYTES) } }

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
        assertArrayEquals(byteArrayOf(1, 2, 3), RomBytes.readPlainFile(rom))
    }

    @Test
    fun `a zipped rom is pulled out`() {
        val zip = zipOf("game.gba" to byteArrayOf(9, 8, 7))
        assertArrayEquals(byteArrayOf(9, 8, 7), read(zip))
    }

    @Test
    fun `the cart is found past files that are not one`() {
        val zip = zipOf(
            "readme.txt" to byteArrayOf(0),
            "cover.png" to byteArrayOf(0),
            "game.agb" to byteArrayOf(4, 5),
        )
        assertArrayEquals(byteArrayOf(4, 5), read(zip))
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
        assertArrayEquals(byteArrayOf(1), read(zip))
        assertArrayEquals(byteArrayOf(1), read(zip))
    }

    /**
     * Save-state keys follow this choice, so it must stay what it was:
     * sorted by lowercase name, directories skipped, first ROM extension.
     * Entries are stored out of order on purpose.
     */
    @Test
    fun `a multi-cart zip picks the same entry as the old ZipFile rule`() {
        val zip = zipOf(
            "notes.txt" to byteArrayOf(0),
            "Zelda.gba" to byteArrayOf(3),
            "dir.gba/" to ByteArray(0),
            "b/Mario.AGB" to byteArrayOf(2),
            "Alpha.bin" to byteArrayOf(1),
            "alpha2.gba" to byteArrayOf(4),
        )
        val old = java.util.zip.ZipFile(zip).use { z ->
            val entry = z.entries().asSequence().filterNot { it.isDirectory }
                .sortedBy { it.name.lowercase() }
                .first { e -> RomBytes.ROM_EXTENSIONS.any { e.name.endsWith(".$it", ignoreCase = true) } }
            z.getInputStream(entry).readBytes()
        }
        assertArrayEquals(old, read(zip))
        assertArrayEquals(byteArrayOf(1), read(zip))
    }

    @Test
    fun `a content id survives the nav route encoding`() {
        val id = "content://com.android.externalstorage.documents/tree/primary%3Aemulators%2FGBA" +
            "/document/primary%3Aemulators%2FGBA%2Fx.gba"
        assertEquals(id, RomBytes.decodeId(RomBytes.encodeId(id)))
        assertFalse(RomBytes.encodeId(id).contains('/'))
    }

    @Test
    fun `an id is a path only when it starts with a slash`() {
        assertTrue(RomBytes.isPath("/storage/emulated/0/GBA/x.gba"))
        assertFalse(RomBytes.isPath("content://x/document/1"))
    }

    @Test
    fun `a zip with no cart reports nothing rather than throwing`() {
        val zip = zipOf("readme.txt" to byteArrayOf(0))
        assertNull(read(zip))
    }

    @Test
    fun `a directory entry is not mistaken for a cart`() {
        val zip = zipOf("roms.gba/" to ByteArray(0), "roms.gba/game.gba" to byteArrayOf(7))
        assertArrayEquals(byteArrayOf(7), read(zip))
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

    @Test(expected = TooLargeException::class)
    fun `a zip entry past 32 MB is refused while inflating`() {
        // Zeros compress to almost nothing, so the archive is small and only
        // the inflated size crosses the cap - the zip bomb case.
        val file = folder.newFile("bomb.zip")
        ZipOutputStream(file.outputStream()).use { out ->
            out.putNextEntry(ZipEntry("big.gba"))
            val chunk = ByteArray(1 shl 20)
            repeat(40) { out.write(chunk) }
            out.closeEntry()
        }
        read(file)
    }

    @Test(expected = TooLargeException::class)
    fun `a plain file past 32 MB is refused before it is read`() {
        val rom = folder.newFile("huge.gba")
        java.io.RandomAccessFile(rom, "rw").use { it.setLength(RomBytes.MAX_ROM_BYTES + 1) }
        RomBytes.readPlainFile(rom)
    }

    @Test
    fun `the head of a zipped cart is read without the rest`() {
        val zip = zipOf("game.gba" to ByteArray(4096) { it.toByte() })
        assertArrayEquals(ByteArray(0xB0) { it.toByte() }, java.util.zip.ZipFile(zip).use { z -> RomBytes.cartIn(z) { RomBytes.readUpTo(it, 0xB0) } })
    }

    @Test
    fun `the head of a short cart is what there is`() {
        val rom = folder.newFile("tiny.gba")
        rom.writeBytes(byteArrayOf(7, 8))
        assertArrayEquals(byteArrayOf(7, 8), rom.inputStream().use { RomBytes.readUpTo(it, 0xB0) })
    }

    @Test
    fun `a capped copy takes exactly the limit and refuses one byte more`() {
        assertEquals(10, ByteArray(10).inputStream().readCapped(10).size)
        val over = runCatching { ByteArray(11).inputStream().readCapped(10) }.exceptionOrNull()
        assertTrue(over is TooLargeException)
    }
}
