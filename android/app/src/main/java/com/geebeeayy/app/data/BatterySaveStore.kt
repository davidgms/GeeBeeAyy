package com.geebeeayy.app.data

import java.io.File
import java.io.FileOutputStream
import java.util.concurrent.atomic.AtomicReference

/**
 * Where one cart's battery save lives on disk, and how it gets there.
 *
 * The battery save is the one file in this app a player cannot get back. It
 * used to be handled by a handful of fields and functions inside the emulation
 * ViewModel, and a whole-codebase review found three ways that lost it:
 *
 * 1. **Newer bytes thrown away.** A flush read the pending bytes, wrote them,
 *    then cleared the slot - so a save the game made *during* the write was
 *    cleared without ever being written.
 * 2. **Two flushes on one temp file.** Cancelling the debounced flush does not
 *    stop one already inside blocking I/O, so an immediate flush could run
 *    beside it, both truncating the same `.tmp`.
 * 3. **A stale save winning.** When the ROM's folder stopped being writable a
 *    flush fell back to app storage; on the next launch the primary was still
 *    preferred because it existed, and the newer fallback was ignored.
 *
 * No Android types, so all three are pinned by JVM tests.
 */
class BatterySaveStore(
    /** Beside the ROM, where a player expects to find it. Null if unwritable. */
    private val primary: File?,
    /** App-private storage, used when the ROM's folder refuses the write. */
    private val fallback: File,
    /**
     * Runs after the bytes reach disk and before the pending slot is cleared.
     * A seam for tests only: it is the exact window in which the game saving
     * again used to lose the newer save, and a test cannot otherwise land
     * work inside it deterministically.
     */
    private val afterWrite: (() -> Unit)? = null,
) {
    private val pending = AtomicReference<ByteArray?>(null)

    /** Where the save currently lives - the file the next flush aims for first. */
    @Volatile
    var current: File = primary ?: fallback
        private set

    /**
     * The save to start the game with, or null if there is none.
     *
     * The **newer** of the two files, not merely the one that exists: a write
     * that fell back to app storage is the latest save, and the primary is the
     * stale one it replaced. Empty files are not saves - no cart has zero
     * bytes of save memory.
     */
    fun load(): ByteArray? {
        val candidates = listOfNotNull(primary, fallback)
            .filter { it.isFile && it.length() > 0 }
            .sortedByDescending { it.lastModified() }
        for (file in candidates) {
            val bytes = runCatching { file.readBytes() }.getOrNull() ?: continue
            if (bytes.isNotEmpty()) {
                current = file
                return bytes
            }
        }
        return null
    }

    /**
     * Hand over the latest save bytes from the emulator.
     *
     * Ignores an empty array: it is a failed read, and written it would rename
     * a 0-byte file over the player's real save.
     */
    fun submit(bytes: ByteArray) {
        if (bytes.isNotEmpty()) pending.set(bytes)
    }

    /** True while there are bytes nobody has written yet. */
    fun hasPending(): Boolean = pending.get() != null

    /**
     * Write whatever is pending. One flush at a time.
     *
     * `@Synchronized` is what keeps two writers off the same temp file. And the
     * slot is cleared with **compare-and-set** against the bytes that were
     * written, not unconditionally: if the game saved again during the write,
     * those newer bytes are still pending afterwards, for the next flush,
     * rather than cleared unwritten.
     *
     * @return true if the save on disk is now up to date.
     */
    @Synchronized
    fun flush(): Boolean {
        val bytes = pending.get() ?: return true
        val written = when {
            atomicWrite(current, bytes) -> true
            current != fallback && atomicWrite(fallback, bytes) -> {
                current = fallback
                true
            }
            else -> false
        }
        afterWrite?.invoke()
        if (written) pending.compareAndSet(bytes, null)
        return written
    }

    /**
     * Temp file, fsync, rename - so a crash or power loss mid-write never
     * leaves a half-written save. The fsync matters: without it the rename can
     * land while the bytes are still only in the page cache.
     */
}

/**
 * Write [bytes] to [target] so a crash or power loss leaves the old file or
 * the new one, never half of either: a tmp file beside it, fsync, rename.
 * The fsync matters - without it the rename can land while the bytes are
 * still only in the page cache, and a power loss leaves an empty file.
 */
internal fun atomicWrite(target: File, bytes: ByteArray): Boolean = runCatching {
    target.parentFile?.mkdirs()
    val tmp = File(target.parentFile, "${target.name}.tmp")
    FileOutputStream(tmp).use { out ->
        out.write(bytes)
        out.flush()
        out.fd.sync()
    }
    tmp.renameTo(target)
}.getOrDefault(false)
