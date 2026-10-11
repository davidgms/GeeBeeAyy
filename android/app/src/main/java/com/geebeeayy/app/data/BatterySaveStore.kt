package com.geebeeayy.app.data

import java.io.File
import java.io.FileOutputStream
import java.util.concurrent.atomic.AtomicReference

/**
 * Where one cart's battery save lives on disk, and how it gets there.
 *
 * The battery save is the one file in this app a player cannot get back. It
 * used to be handled by a handful of fields and functions inside the emulation
 * ViewModel, and a whole-codebase review found ways that lost it:
 *
 * 1. **Newer bytes thrown away.** A flush read the pending bytes, wrote them,
 *    then cleared the slot - so a save the game made *during* the write was
 *    cleared without ever being written.
 * 2. **Two flushes on one temp file.** Cancelling the debounced flush does not
 *    stop one already inside blocking I/O, so an immediate flush could run
 *    beside it, both truncating the same `.tmp`.
 *
 * The save lives in app-private storage only. It used to live beside the ROM
 * on shared storage, with the newer of the two copies winning - so any app
 * could drop a `.sav` with a later timestamp next to the cart and replace the
 * player's save (audit M5). [importLegacySave] brings the old copy in once.
 *
 * No Android types, so all of it is pinned by JVM tests.
 */
class BatterySaveStore(
    /** App-private, keyed by the cart header. */
    val file: File,
    /**
     * Runs after the bytes reach disk and before the pending slot is cleared.
     * A seam for tests only: it is the exact window in which the game saving
     * again used to lose the newer save, and a test cannot otherwise land
     * work inside it deterministically.
     */
    private val afterWrite: (() -> Unit)? = null,
) {
    private val pending = AtomicReference<ByteArray?>(null)

    /**
     * The save to start the game with, or null if there is none. Empty files
     * are not saves - no cart has zero bytes of save memory.
     */
    fun load(): ByteArray? =
        runCatching { file.readBytes() }.getOrNull()?.takeIf { it.isNotEmpty() }

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
        val written = atomicWrite(file, bytes)
        afterWrite?.invoke()
        if (written) pending.compareAndSet(bytes, null)
        return written
    }
}

/** The largest GBA save is 128 KB of flash; anything far past it is not one. */
private const val MAX_SAVE_BYTES = 1L shl 20

/**
 * Bring the `.sav` that used to live beside the ROM into app storage, once.
 *
 * The first time a cart is seen, the newer of [legacy] and [private] wins -
 * the rule the app used until now, so a save written to either place under
 * the old code is not lost in the move. A private copy that is replaced is
 * kept as `.before-import`. After that a marker is left, and the file beside
 * the ROM is never read again: that later read is exactly what let a planted
 * file win.
 *
 * The legacy file is never deleted - it is the player's own copy on shared
 * storage. A failed read or write leaves no marker, so the next launch tries
 * again rather than starting the game without its save.
 *
 * @return false if the import failed and should be retried.
 */
internal fun importLegacySave(legacy: File, private: File): Boolean {
    val marker = File(private.parentFile, "${private.name}.imported")
    if (marker.exists()) return true
    val size = legacy.takeIf { it.isFile }?.length() ?: 0L
    if (size in 1..MAX_SAVE_BYTES) {
        val privateIsNewer = private.isFile && private.length() > 0 &&
            private.lastModified() >= legacy.lastModified()
        if (!privateIsNewer) {
            val bytes = runCatching { legacy.readBytes() }.getOrNull() ?: return false
            if (private.isFile && private.length() > 0) {
                val kept = File(private.parentFile, "${private.name}.before-import")
                if (!atomicWrite(kept, private.readBytes())) return false
            }
            if (!atomicWrite(private, bytes)) return false
            if (!private.readBytes().contentEquals(bytes)) return false
        }
    }
    return runCatching { marker.parentFile?.mkdirs(); marker.createNewFile() || marker.exists() }
        .getOrDefault(false)
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
