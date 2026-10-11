package com.geebeeayy.app.data

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** The folder migration and the document-tree scan, without a device. */
class RomFoldersTest {

    @Test
    fun `primary storage paths map to primary document ids`() {
        assertEquals("primary:Roms/GBA", legacyDocId("/storage/emulated/0/Roms/GBA"))
        assertEquals("primary:Roms/GBA", legacyDocId("/sdcard/Roms/GBA/"))
        assertEquals("primary:", legacyDocId("/storage/emulated/0"))
        assertEquals("1A2B-3C4D:Roms", legacyDocId("/storage/1A2B-3C4D/Roms"))
    }

    @Test
    fun `paths the provider does not serve do not map`() {
        assertNull(legacyDocId("/data/user/0/app/files"))
        assertNull(legacyDocId("/storage/emulated/10/Roms"))
        assertNull(legacyDocId("/sdcardfoo/Roms"))
    }

    @Test
    fun `a path under the folder gets its own document id, whichever alias either uses`() {
        val folder = "/storage/emulated/0/Roms"
        assertEquals("primary:Roms/x.gba", rekeyedDocId("/sdcard/Roms/x.gba", folder))
        assertEquals("primary:Roms/Sub/y.gba", rekeyedDocId("/storage/emulated/0/Roms/Sub/y.gba", "/sdcard/Roms"))
    }

    @Test
    fun `a path outside the folder is not rekeyed`() {
        val folder = "/storage/emulated/0/Roms"
        assertNull(rekeyedDocId("/storage/emulated/0/Roms2/x.gba", folder))
        assertNull(rekeyedDocId("/storage/emulated/0/Other/x.gba", folder))
        assertNull(rekeyedDocId(folder, folder))
    }

    @Test
    fun `rekeying a whole library maps exact paths and leaves the rest`() {
        val moved = rekeyPaths(
            listOf("/sdcard/Roms/a.gba", "/sdcard/Roms/Sub/b.zip", "/sdcard/Else/c.gba"),
            "/storage/emulated/0/Roms",
        ) { "tree:$it" }
        assertEquals(
            mapOf("/sdcard/Roms/a.gba" to "tree:primary:Roms/a.gba", "/sdcard/Roms/Sub/b.zip" to "tree:primary:Roms/Sub/b.zip"),
            moved,
        )
    }

    @Test
    fun `the root of a volume is a folder too`() {
        assertEquals("primary:x.gba", rekeyedDocId("/sdcard/x.gba", "/sdcard"))
    }

    @Test
    fun `labels read as a volume and a path`() {
        assertEquals("Internal storage/Roms/GBA", docIdLabel("primary:Roms/GBA"))
        assertEquals("Internal storage", docIdLabel("primary:"))
        assertEquals("1A2B-3C4D/Roms", docIdLabel("1A2B-3C4D:Roms"))
        assertEquals("x.gba", docIdName("primary:Roms/x.gba"))
    }

    // scanTree: a fake provider as a map of directory id to listing.

    private fun file(dir: String, name: String, size: Long? = 10L) =
        DocChild("$dir/$name", name, "application/octet-stream", size, 5L)

    private fun dir(parent: String, name: String) = DocChild("$parent/$name", name, DIR_MIME, null, 0L)

    private class Provider(val dirs: Map<String, List<DocChild>?>) {
        val queries = mutableListOf<String>()
        fun children(id: String): List<DocChild>? {
            queries += id
            return dirs[id]
        }
    }

    @Test
    fun `the scan recurses and finds the save and cover beside each rom in the same pass`() {
        val root = "primary:Roms"
        val p = Provider(
            mapOf(
                root to listOf(
                    file(root, "Game.gba"),
                    file(root, "Game.sav"),
                    file(root, "Game.PNG"),
                    file(root, "notes.txt"),
                    dir(root, "Sub"),
                    dir(root, "covers"),
                ),
                "$root/Sub" to listOf(file("$root/Sub", "Deep.zip"), file("$root/Sub", "Deep.jpg")),
                "$root/covers" to listOf(file("$root/covers", "Lone.webp")),
            ),
        )
        val found = scanTree(root, children = p::children)!!.associateBy { it.rom.name }
        assertEquals(setOf("Game.gba", "Deep.zip"), found.keys)
        assertEquals("primary:Roms/Game.sav", found.getValue("Game.gba").save?.docId)
        assertEquals("primary:Roms/Game.PNG", found.getValue("Game.gba").cover?.docId)
        assertNull(found.getValue("Deep.zip").save)
        assertEquals("primary:Roms/Sub/Deep.jpg", found.getValue("Deep.zip").cover?.docId)
        assertEquals("Internal storage/Roms/Sub", found.getValue("Deep.zip").folderLabel)
        assertEquals("one query per directory", p.queries.size, p.queries.toSet().size)
    }

    @Test
    fun `a cover in covers beside the rom is found when none sits next to it`() {
        val root = "primary:Roms"
        val p = Provider(
            mapOf(
                root to listOf(file(root, "Lone.gba"), dir(root, "covers")),
                "$root/covers" to listOf(file("$root/covers", "Lone.webp")),
            ),
        )
        assertEquals("primary:Roms/covers/Lone.webp", scanTree(root, children = p::children)!!.single().cover?.docId)
        assertEquals("covers is listed once, not again when the walk reaches it", 2, p.queries.size)
    }

    @Test
    fun `a root that cannot be read is reported, a subfolder that cannot is skipped`() {
        val root = "primary:Roms"
        assertNull(scanTree(root) { null })
        val p = Provider(mapOf(root to listOf(file(root, "A.gba"), dir(root, "Gone")), "$root/Gone" to null))
        assertEquals(1, scanTree(root, children = p::children)!!.size)
    }

    @Test
    fun `the depth cap stops a folder loop`() {
        val p = Provider(emptyMap())
        val found = scanTree("d0", maxDepth = 3) { id ->
            p.queries += id
            listOf(dir(id, "d"), file(id, "x.gba"))
        }!!
        assertEquals(4, found.size)
        assertTrue(p.queries.size == 4)
    }
}
