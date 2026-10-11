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
import java.util.concurrent.CountDownLatch
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit

/**
 * The battery save is the one file a player cannot get back. Each test here
 * is a way the old code lost it, or a way the move to app storage could.
 */
class BatterySaveStoreTest {

    @get:Rule
    val folder = TemporaryFolder()

    private fun saveFile() = File(folder.root, "app/saves/GAME.sav")
    private fun legacyFile() = File(folder.root, "roms/game.sav")

    @Test
    fun `a flush writes the save to app storage`() {
        val store = BatterySaveStore(saveFile())
        store.submit(byteArrayOf(1, 2, 3))
        assertTrue(store.flush())
        assertArrayEquals(byteArrayOf(1, 2, 3), saveFile().readBytes())
        assertFalse(store.hasPending())
    }

    /** No cart has zero bytes of save memory: an empty read is a failure. */
    @Test
    fun `an empty save is never written over a real one`() {
        val store = BatterySaveStore(saveFile())
        saveFile().parentFile!!.mkdirs()
        saveFile().writeBytes(byteArrayOf(9, 9))
        store.submit(ByteArray(0))
        store.flush()
        assertArrayEquals(byteArrayOf(9, 9), saveFile().readBytes())
    }

    /**
     * The first bug: bytes submitted *during* a write were cleared unwritten.
     *
     * The game saves again in the exact window between the disk write and the
     * clear - landed there deterministically through the `afterWrite` seam.
     * Clearing unconditionally loses those bytes; compare-and-set keeps them.
     */
    @Test
    fun `a save made during a write is not lost`() {
        lateinit var store: BatterySaveStore
        var raced = false
        store = BatterySaveStore(saveFile(), afterWrite = {
            if (!raced) {
                raced = true
                store.submit(byteArrayOf(2)) // the game saves again, mid-flush
            }
        })

        store.submit(byteArrayOf(1))
        store.flush()
        assertTrue("the save made mid-flush must still be pending", store.hasPending())
        store.flush()
        assertArrayEquals(byteArrayOf(2), saveFile().readBytes())
    }

    /**
     * Hammered from many threads, every write must leave a whole save on disk.
     *
     * **A smoke test, not a proof.** It passed three runs in a row with the
     * `@Synchronized` removed: on a fast local disk two writers rarely
     * interleave inside one small write. The guarantee comes from the lock
     * itself; this catches a gross regression, like a flush that leaves bytes
     * pending or a save of the wrong length.
     */
    @Test
    fun `concurrent flushes never leave a torn save`() {
        val store = BatterySaveStore(saveFile())
        val pool = Executors.newFixedThreadPool(8)
        val start = CountDownLatch(1)
        val payloads = (1..200).map { n -> ByteArray(4096) { n.toByte() } }
        payloads.forEach { bytes ->
            pool.execute {
                start.await()
                store.submit(bytes)
                store.flush()
            }
        }
        start.countDown()
        pool.shutdown()
        assertTrue(pool.awaitTermination(30, TimeUnit.SECONDS))
        store.flush()

        val onDisk = saveFile().readBytes()
        assertEquals(4096, onDisk.size)
        assertTrue(
            "every byte of the save must come from one write, not a mix of two",
            onDisk.all { it == onDisk[0] },
        )
        assertFalse(store.hasPending())
    }

    @Test
    fun `an empty file is not a save`() {
        saveFile().parentFile!!.mkdirs()
        saveFile().writeBytes(ByteArray(0))
        assertNull(BatterySaveStore(saveFile()).load())
    }

    private fun write(file: File, bytes: ByteArray, modified: Long) {
        file.parentFile!!.mkdirs()
        file.writeBytes(bytes)
        file.setLastModified(modified)
    }

    @Test
    fun `the save beside the rom is imported on first sight`() {
        write(legacyFile(), byteArrayOf(4, 4), 1_000_000L)
        assertTrue(importLegacySave(legacyFile(), saveFile()))
        assertArrayEquals(byteArrayOf(4, 4), BatterySaveStore(saveFile()).load())
        assertTrue("the player's own copy is never deleted", legacyFile().isFile)
    }

    /**
     * The first import keeps the old rule, newer wins, so a save the old code
     * wrote to app storage when the ROM folder was read-only is not lost.
     */
    @Test
    fun `a newer private save survives the import`() {
        write(legacyFile(), byteArrayOf(1), 1_000_000L)
        write(saveFile(), byteArrayOf(2), 2_000_000L)
        importLegacySave(legacyFile(), saveFile())
        assertArrayEquals(byteArrayOf(2), saveFile().readBytes())
    }

    @Test
    fun `a newer save beside the rom wins once and the old private copy is kept`() {
        write(saveFile(), byteArrayOf(1), 1_000_000L)
        write(legacyFile(), byteArrayOf(2), 2_000_000L)
        importLegacySave(legacyFile(), saveFile())
        assertArrayEquals(byteArrayOf(2), saveFile().readBytes())
        assertArrayEquals(byteArrayOf(1), File(saveFile().parentFile, "GAME.sav.before-import").readBytes())
    }

    /** Audit M5: after the import, a file dropped beside the ROM never wins. */
    @Test
    fun `a file planted beside the rom after the import is ignored`() {
        write(legacyFile(), byteArrayOf(1), 1_000_000L)
        importLegacySave(legacyFile(), saveFile())
        write(legacyFile(), byteArrayOf(6, 6, 6), System.currentTimeMillis() + 60_000)
        importLegacySave(legacyFile(), saveFile())
        assertArrayEquals(byteArrayOf(1), BatterySaveStore(saveFile()).load())
    }

    @Test
    fun `an empty or oversized file beside the rom is not imported`() {
        write(legacyFile(), ByteArray(0), 1_000_000L)
        importLegacySave(legacyFile(), saveFile())
        assertFalse(saveFile().exists())
        val other = File(folder.root, "app/saves/OTHER.sav")
        val big = File(folder.root, "roms/big.sav")
        java.io.RandomAccessFile(big.also { it.parentFile!!.mkdirs() }, "rw").use { it.setLength(2L shl 20) }
        importLegacySave(big, other)
        assertFalse(other.exists())
    }

    @Test
    fun `an unreadable save beside the rom leaves no marker, so it is retried`() {
        // A directory where the file should be: isFile is false, so nothing
        // to import - but a folder that cannot be read must not block a retry.
        val unwritable = File(folder.root, "blocked")
        unwritable.writeText("a file, so nothing can be made inside it")
        write(legacyFile(), byteArrayOf(3), 1_000_000L)
        assertFalse(importLegacySave(legacyFile(), File(unwritable, "GAME.sav")))
    }

    @Test
    fun `no file beside the rom and no save yet leaves no marker`() {
        assertTrue(importLegacySave(legacyFile(), saveFile()))
        write(legacyFile(), byteArrayOf(8), 1_000_000L)
        importLegacySave(legacyFile(), saveFile())
        assertArrayEquals("a save that shows up later is still brought in", byteArrayOf(8), saveFile().readBytes())
    }

    @Test
    fun `no file beside the rom but a private save marks it, so nothing planted later wins`() {
        write(saveFile(), byteArrayOf(1), 1_000_000L)
        importLegacySave(legacyFile(), saveFile())
        write(legacyFile(), byteArrayOf(6), System.currentTimeMillis() + 60_000)
        importLegacySave(legacyFile(), saveFile())
        assertArrayEquals(byteArrayOf(1), saveFile().readBytes())
    }

    @Test
    fun `no temp file is left behind`() {
        val store = BatterySaveStore(saveFile())
        store.submit(byteArrayOf(7))
        store.flush()
        assertFalse(File(saveFile().parentFile, "GAME.sav.tmp").exists())
    }
}
