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

class SaveImportTest {

    @get:Rule
    val folder = TemporaryFolder()

    private fun saveFile() = File(folder.root, "app/saves/GAME.sav")
    private fun bak(n: Int) = File(saveFile().parentFile, "GAME.sav.bak$n")

    private fun rom(fileName: String, exists: Boolean = true) =
        RomEntry(name = fileName, fileName = fileName, size = "", filePath = "/r/$fileName", exists = exists)

    // Names

    @Test
    fun `a name is reduced to lowercase letters and digits`() {
        assertEquals("pokemonemerald", normaliseSaveName("Pokemon - Emerald (USA, Europe) [!].sav"))
        assertEquals("pokemonemerald", normaliseSaveName("pokemon_emerald.gba"))
        assertEquals("", normaliseSaveName("(USA).sav"))
    }

    @Test
    fun `a save matches the one game with its name`() {
        val roms = listOf(rom("Metroid Fusion (U).gba"), rom("Zelda.gba"))
        assertEquals("Zelda.gba", matchRom("zelda [v1.1].srm", roms)?.fileName)
    }

    @Test
    fun `no game or several games is no match`() {
        val roms = listOf(rom("Zelda (U).gba"), rom("Zelda (E).gba"), rom("Gone.gba", exists = false))
        assertNull(matchRom("Zelda.sav", roms))
        assertNull(matchRom("Mario.sav", roms))
        assertNull(matchRom("Gone.sav", roms))
        assertNull(matchRom("(x).sav", roms))
    }

    @Test
    fun `only save extensions of a sane size are found, in any folder`() {
        fun f(id: String, name: String, size: Long?) = DocChild(id, name, "application/octet-stream", size, 0L)
        val tree = mapOf(
            "root" to listOf(
                DocChild("root/sub", "sub", DIR_MIME, null, 0L),
                f("a", "A.SAV", 32768), f("b", "b.txt", 10), f("c", "c.srm", 0), f("d", "d.sa1", null),
                f("e", "e.sav", (1L shl 20) + 1),
            ),
            "root/sub" to listOf(f("g", "g.Srm", 1)),
        )
        val found = findSaves("root") { tree[it] }!!
        assertEquals(listOf("A.SAV", "d.sa1", "g.Srm"), found.map { it.name })
        assertNull(findSaves("root") { null })
    }

    // Forced import

    @Test
    fun `a forced import trims the footer and marks the save imported`() {
        assertTrue(forceImportSave(saveFile(), ByteArray(32784) { 7 }, 32768))
        assertEquals(32768, saveFile().length())
        assertTrue(File(saveFile().parentFile, "GAME.sav.imported").exists())
    }

    @Test
    fun `a file that shrank since the check is refused and the save kept`() {
        saveFile().parentFile!!.mkdirs()
        saveFile().writeBytes(byteArrayOf(1))
        assertFalse(forceImportSave(saveFile(), ByteArray(100), 32768))
        assertArrayEquals(byteArrayOf(1), saveFile().readBytes())
        assertFalse(bak(1).exists())
    }

    @Test
    fun `a forced import ignores newer-wins and the marker`() {
        saveFile().parentFile!!.mkdirs()
        saveFile().writeBytes(byteArrayOf(1))
        File(saveFile().parentFile, "GAME.sav.imported").createNewFile()
        saveFile().setLastModified(System.currentTimeMillis() + 60_000)
        assertTrue(forceImportSave(saveFile(), byteArrayOf(2, 2), 2))
        assertArrayEquals(byteArrayOf(2, 2), saveFile().readBytes())
        assertArrayEquals(byteArrayOf(1), bak(1).readBytes())
    }

    @Test
    fun `three backups are kept, newest first`() {
        for (v in 1..5) assertTrue(forceImportSave(saveFile(), byteArrayOf(v.toByte()), 1))
        assertArrayEquals(byteArrayOf(5), saveFile().readBytes())
        assertArrayEquals(byteArrayOf(4), bak(1).readBytes())
        assertArrayEquals(byteArrayOf(3), bak(2).readBytes())
        assertArrayEquals(byteArrayOf(2), bak(3).readBytes())
        assertFalse(File(saveFile().parentFile, "GAME.sav.bak4").exists())
    }

    // Auto import

    @Test
    fun `a legacy save that does not fit is skipped but the import is done`() {
        saveFile().parentFile!!.mkdirs()
        saveFile().writeBytes(byteArrayOf(9, 9))
        val legacy = LegacySave(true, 3L, 1L) { byteArrayOf(1, 2, 3) }
        assertTrue(importLegacySave(legacy, saveFile()) { null })
        assertArrayEquals(byteArrayOf(9, 9), saveFile().readBytes())
        assertTrue(File(saveFile().parentFile, "GAME.sav.imported").exists())
        assertFalse(File(saveFile().parentFile, "GAME.sav.before-import").exists())
    }

    @Test
    fun `a legacy save with a footer is imported without it`() {
        val legacy = LegacySave(true, 4L, 1L) { byteArrayOf(1, 2, 3, 4) }
        assertTrue(importLegacySave(legacy, saveFile()) { 3 })
        assertArrayEquals(byteArrayOf(1, 2, 3), saveFile().readBytes())
    }

    @Test
    fun `a fit check that throws leaves no marker so the import is retried`() {
        val legacy = LegacySave(true, 1L, 1L) { byteArrayOf(1) }
        assertFalse(importLegacySave(legacy, saveFile()) { error("rom unreadable") })
        assertFalse(File(saveFile().parentFile, "GAME.sav.imported").exists())
    }

    // Reasons a file does not fit

    @Test
    fun `a chip size reads as the player knows it`() {
        assertEquals("32 KB SRAM", saveChipLabel(32768))
        assertEquals("64 KB Flash", saveChipLabel(65536))
        assertEquals("128 KB Flash", saveChipLabel(131072))
        assertEquals("EEPROM (512 B or 8 KB)", saveChipLabel(8192))
        assertNull(saveChipLabel(0))
        assertNull(saveChipLabel(-1))
        assertNull(saveChipLabel(1234))
    }

    @Test
    fun `a nocash compressed save is recognised by its header`() {
        val magic = "NocashGbaBackupMediaSavDataFile".toByteArray(Charsets.US_ASCII)
        assertTrue(isNocashSave(magic + ByteArray(40)))
        assertTrue(isNocashSave(magic))
        assertFalse(isNocashSave(magic.copyOf(magic.size - 1)))
        assertFalse(isNocashSave(ByteArray(65536)))
        assertFalse(isNocashSave(ByteArray(0)))
    }
}
