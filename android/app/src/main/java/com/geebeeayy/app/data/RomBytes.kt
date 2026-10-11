package com.geebeeayy.app.data

import java.io.File
import java.io.InputStream
import java.util.zip.ZipFile

/**
 * Reading a cart's bytes, whether or not somebody zipped it first.
 *
 * Downloaded ROMs arrive zipped, and telling a player to unzip it on a PC is
 * the kind of friction that makes an app feel unfinished. A `.zip` is opened
 * and the first cart inside is used; everything else is read as-is.
 *
 * **Only zip.** 7z and rar are different formats with no decoder in the
 * platform, and adding one is a dependency for a convenience - see
 * [UNSUPPORTED_ARCHIVES], which exists so the UI can say why rather than
 * showing a file that will not open.
 */
object RomBytes {

    /** Extensions that are a cart, inside an archive or out of it. */
    val ROM_EXTENSIONS = listOf("gba", "agb", "bin")

    /** Archives the app can open. */
    const val ZIP_EXTENSION = "zip"

    /** Archives it deliberately cannot, so a message can name them. */
    val UNSUPPORTED_ARCHIVES = listOf("7z", "rar")

    fun isZip(path: String): Boolean =
        File(path).extension.equals(ZIP_EXTENSION, ignoreCase = true)

    /**
     * The largest cart there is. The core rejects anything bigger
     * (`core/src/cart/mod.rs`), so reading past it only spends memory on a
     * file that will not load - or, for a zip bomb, crashes the app.
     */
    const val MAX_ROM_BYTES = 32L * 1024 * 1024

    /**
     * Every byte of the cart at [path].
     *
     * Throws whatever the read threw, so a caller can report it, and
     * [TooLargeException] past [MAX_ROM_BYTES]. Returns null only when a zip
     * has been opened and holds no cart at all - a separate case from "could
     * not read", and worth a different message.
     */
    fun read(path: String): ByteArray? {
        if (!isZip(path)) {
            // Checked up front, then read in one sized allocation: a capped
            // stream would peak at twice the cart while it copies out.
            if (File(path).length() > MAX_ROM_BYTES) throw TooLargeException(MAX_ROM_BYTES)
            return File(path).readBytes()
        }
        return withCart(path) { it.readCapped(MAX_ROM_BYTES) }
    }

    /**
     * The first [count] bytes of the cart, or fewer if it is shorter. For a
     * header: a zipped cart is inflated only as far as [count], not whole.
     */
    fun head(path: String, count: Int): ByteArray? = withCart(path) { stream ->
        val head = ByteArray(count)
        var read = 0
        while (read < count) {
            val n = stream.read(head, read, count - read)
            if (n <= 0) break
            read += n
        }
        head.copyOf(read)
    }

    private fun <T> withCart(path: String, block: (InputStream) -> T): T? {
        if (!isZip(path)) return File(path).inputStream().use(block)
        ZipFile(File(path)).use { zip ->
            val entry = zip.entries().asSequence()
                .filterNot { it.isDirectory }
                // Sorted, so a zip holding several carts picks the same one
                // every launch. An arbitrary order would key save states on a
                // different game between runs.
                .sortedBy { it.name.lowercase() }
                .firstOrNull { candidate ->
                    ROM_EXTENSIONS.any { candidate.name.endsWith(".$it", ignoreCase = true) }
                } ?: return null
            return zip.getInputStream(entry).use(block)
        }
    }

    /**
     * The name to show and to key saves on.
     *
     * For a zip this is the **archive's** name, not the entry's: the file on
     * disk is what the player renames, moves and recognises, and keying saves
     * on the entry would orphan them the moment somebody rezipped.
     */
    fun displayBaseName(path: String): String = File(path).nameWithoutExtension
}
