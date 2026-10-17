package com.gbee.emulator.data

import android.util.Log
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
     * [SAVE_WRITE_LOCK] is what keeps two writers off the same temp file -
     * another flush, or an import running from a library scan. And the
     * slot is cleared with **compare-and-set** against the bytes that were
     * written, not unconditionally: if the game saved again during the write,
     * those newer bytes are still pending afterwards, for the next flush,
     * rather than cleared unwritten.
     *
     * @return true if the save on disk is now up to date.
     */
    fun flush(): Boolean = synchronized(SAVE_WRITE_LOCK) {
        val bytes = pending.get() ?: return true
        val written = atomicWrite(file, bytes)
        afterWrite?.invoke()
        if (written) pending.compareAndSet(bytes, null)
        written
    }
}

/**
 * One writer at a time for every battery save: a flush, a scan-time import
 * and a game-start import all write `<key>.sav` through the same `.tmp`.
 * ponytail: one global lock, per-key locks if saves ever get large or many.
 */
internal val SAVE_WRITE_LOCK = Any()

/** The largest GBA save is 128 KB of flash; anything far past it is not one. */
internal const val MAX_SAVE_BYTES = 1L shl 20

/**
 * The `.sav` that used to live beside the ROM, as far as an import needs to
 * know it: a file, or a document whose size and time came from the scan.
 * [read] is only called once the size is known to be a plausible save.
 */
internal class LegacySave(
    val exists: Boolean,
    val size: Long,
    val lastModified: Long,
    val read: () -> ByteArray,
)

internal fun File.asLegacySave() =
    LegacySave(exists(), if (isFile) length() else 0L, lastModified()) { readBytes() }

internal fun importLegacySave(legacy: File, private: File, fit: (Int) -> Int? = { it }): Boolean =
    importLegacySave(legacy.asLegacySave(), private, fit)

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
 * [fit] maps the legacy file's length to the bytes of it to keep, or null when
 * it does not fit this cart's save chip (the core's rule, see `GbaEngine.saveFit`).
 * A misfit is skipped like a lost race: the private save stays, the marker is
 * written so it is not retried every launch, and the import counts as done -
 * blocking the launch over a wrong-size file would lock the player out.
 *
 * @return false if the import failed and should be retried.
 */
internal fun importLegacySave(
    legacy: LegacySave,
    private: File,
    fit: (Int) -> Int? = { it },
): Boolean = synchronized(SAVE_WRITE_LOCK) {
    val marker = File(private.parentFile, "${private.name}.imported")
    if (marker.exists()) return true
    val privateExists = private.isFile && private.length() > 0
    if (!legacy.exists) {
        // Nothing to bring in. Marked only once a private save exists to
        // protect - `exists()` is also false on an I/O error, and marking
        // then would skip a real save forever.
        return !privateExists || markImported(marker)
    }
    val size = legacy.size
    if (size in 1..MAX_SAVE_BYTES) {
        val privateIsNewer = privateExists && private.lastModified() >= legacy.lastModified
        if (!privateIsNewer) {
            val read = runCatching { legacy.read() }.getOrNull() ?: return false
            val keep = runCatching { fit(read.size) }.getOrElse { return false }
            if (keep == null) {
                Log.w("GBee/Save", "legacy save of ${read.size} bytes does not fit ${private.name}, skipped")
                return markImported(marker)
            }
            val bytes = read.copyOf(keep)
            if (privateExists) {
                val kept = File(private.parentFile, "${private.name}.before-import")
                if (!atomicWrite(kept, private.readBytes())) return false
            }
            if (!atomicWrite(private, bytes)) return false
            if (!private.readBytes().contentEquals(bytes)) return false
        }
    }
    markImported(marker)
}

/** Backups of a replaced save a manual import keeps: `.bak1` newest to `.bak3` oldest. */
internal const val SAVE_BACKUPS = 3

/**
 * Replace [private] with the first [keep] bytes of [bytes], whatever is there
 * and whatever the marker says: the player asked for exactly this file. The
 * save being replaced is rotated into `.bak1` first (`.bak3` falls off), and
 * the import marker is written so the auto import never later swaps the old
 * file beside the ROM back in.
 *
 * @return false if the backup or the write failed; then the save is untouched.
 */
internal fun forceImportSave(private: File, bytes: ByteArray, keep: Int): Boolean = synchronized(SAVE_WRITE_LOCK) {
    fun bak(n: Int) = File(private.parentFile, "${private.name}.bak$n")
    // A file that shrank since it was checked would be zero-padded, not imported.
    if (bytes.size < keep) return false
    val kept = bytes.copyOf(keep)
    if (private.isFile && private.length() > 0) {
        val old = runCatching { private.readBytes() }.getOrNull() ?: return false
        // Written aside first, so a failed backup costs no older generation.
        val next = File(private.parentFile, "${private.name}.bak.new")
        if (!atomicWrite(next, old)) return false
        for (n in SAVE_BACKUPS - 1 downTo 1) if (bak(n).exists() && !bak(n).renameTo(bak(n + 1))) return false
        if (!next.renameTo(bak(1))) return false
    }
    if (!atomicWrite(private, kept)) return false
    if (!private.readBytes().contentEquals(kept)) return false
    markImported(File(private.parentFile, "${private.name}.imported"))
}

private fun markImported(marker: File): Boolean =
    runCatching { marker.parentFile?.mkdirs(); marker.createNewFile() || marker.exists() }
        .getOrDefault(false)

/**
 * Write [bytes] to [target] so a crash or power loss leaves the old file or
 * the new one, never half of either: a tmp file beside it, fsync, rename.
 * The fsync matters - without it the rename can land while the bytes are
 * still only in the page cache, and a power loss leaves an empty file.
 */
internal fun atomicWrite(target: File, bytes: ByteArray): Boolean {
    val tmp = File(target.parentFile, "${target.name}.tmp")
    val written = runCatching {
        target.parentFile?.mkdirs()
        FileOutputStream(tmp).use { out ->
            out.write(bytes)
            out.flush()
            out.fd.sync()
        }
        tmp.renameTo(target)
    }.getOrDefault(false)
    // On a full disk the write fails part way; do not leave the empty or
    // half-written tmp behind to hold on to space the player is short of.
    if (!written) tmp.delete()
    return written
}
