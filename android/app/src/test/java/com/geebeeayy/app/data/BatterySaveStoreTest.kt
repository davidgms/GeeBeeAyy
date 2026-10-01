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
 * is a way the old code lost it.
 */
class BatterySaveStoreTest {

    @get:Rule
    val folder = TemporaryFolder()

    private fun store(): Pair<BatterySaveStore, Pair<File, File>> {
        val primary = File(folder.root, "roms/game.sav")
        val fallback = File(folder.root, "app/saves/GAME.sav")
        return BatterySaveStore(primary, fallback) to (primary to fallback)
    }

    @Test
    fun `a flush writes the save beside the rom`() {
        val (store, files) = store()
        store.submit(byteArrayOf(1, 2, 3))
        assertTrue(store.flush())
        assertArrayEquals(byteArrayOf(1, 2, 3), files.first.readBytes())
        assertFalse(store.hasPending())
    }

    /** No cart has zero bytes of save memory: an empty read is a failure. */
    @Test
    fun `an empty save is never written over a real one`() {
        val (store, files) = store()
        files.first.parentFile!!.mkdirs()
        files.first.writeBytes(byteArrayOf(9, 9))
        store.submit(ByteArray(0))
        store.flush()
        assertArrayEquals(byteArrayOf(9, 9), files.first.readBytes())
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
        val primary = File(folder.root, "roms/game.sav")
        val fallback = File(folder.root, "app/saves/GAME.sav")
        lateinit var store: BatterySaveStore
        var raced = false
        store = BatterySaveStore(primary, fallback, afterWrite = {
            if (!raced) {
                raced = true
                store.submit(byteArrayOf(2)) // the game saves again, mid-flush
            }
        })

        store.submit(byteArrayOf(1))
        store.flush()
        assertTrue("the save made mid-flush must still be pending", store.hasPending())
        store.flush()
        assertArrayEquals(byteArrayOf(2), primary.readBytes())
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
        val (store, files) = store()
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

        val onDisk = files.first.readBytes()
        assertEquals(4096, onDisk.size)
        assertTrue(
            "every byte of the save must come from one write, not a mix of two",
            onDisk.all { it == onDisk[0] },
        )
        assertFalse(store.hasPending())
    }

    /**
     * The third bug: a write that fell back to app storage is the newest
     * save, and the primary it replaced must not win on the next launch.
     */
    @Test
    fun `the newer save wins, wherever it is`() {
        val (store, files) = store()
        files.first.parentFile!!.mkdirs()
        files.first.writeBytes(byteArrayOf(1)) // old, beside the rom
        files.first.setLastModified(1_000_000L)
        files.second.parentFile!!.mkdirs()
        files.second.writeBytes(byteArrayOf(2)) // newer, in app storage
        files.second.setLastModified(2_000_000L)

        assertArrayEquals(byteArrayOf(2), store.load())
        assertEquals(files.second, store.current)
    }

    @Test
    fun `an empty file is not a save`() {
        val (store, files) = store()
        files.first.parentFile!!.mkdirs()
        files.first.writeBytes(ByteArray(0))
        assertNull(store.load())
    }

    /** A folder the ROM sits in can stop being writable; the save must still land. */
    @Test
    fun `an unwritable rom folder falls back to app storage`() {
        val blocked = File(folder.root, "readonly")
        blocked.writeText("a file, so nothing can be made inside it")
        val primary = File(blocked, "game.sav")
        val fallback = File(folder.root, "app/saves/GAME.sav")
        val store = BatterySaveStore(primary, fallback)

        store.submit(byteArrayOf(5))
        assertTrue(store.flush())
        assertArrayEquals(byteArrayOf(5), fallback.readBytes())
        assertEquals(fallback, store.current)
    }

    @Test
    fun `no temp file is left behind`() {
        val (store, files) = store()
        store.submit(byteArrayOf(7))
        store.flush()
        assertFalse(File(files.first.parentFile, "game.sav.tmp").exists())
    }
}
