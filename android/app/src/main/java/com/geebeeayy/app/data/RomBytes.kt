package com.geebeeayy.app.data

import android.content.Context
import android.net.Uri
import android.provider.OpenableColumns
import java.io.File
import java.io.FileNotFoundException
import java.io.InputStream
import java.net.URLDecoder
import java.net.URLEncoder
import java.util.zip.ZipFile
import java.util.zip.ZipInputStream

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

    /** True for a legacy absolute path; anything else is a `content://` document Uri. */
    fun isPath(id: String): Boolean = id.startsWith("/")

    /**
     * The file name behind [id]. A path has it in its last segment; a document
     * Uri is opaque (its last segment is a provider-defined document id), so
     * the name is asked of the provider, never parsed out of the Uri.
     */
    fun fileName(context: Context, id: String): String {
        if (isPath(id)) return File(id).name
        context.contentResolver.query(
            Uri.parse(id), arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null,
        )?.use { if (it.moveToFirst()) it.getString(0)?.let { name -> return name } }
        return ""
    }

    /** True if [name] is a `.zip`, whatever the case. */
    fun isZip(name: String): Boolean =
        File(name).extension.equals(ZIP_EXTENSION, ignoreCase = true)

    /**
     * The largest cart there is. The core rejects anything bigger
     * (`core/src/cart/mod.rs`), so reading past it only spends memory on a
     * file that will not load - or, for a zip bomb, crashes the app.
     */
    const val MAX_ROM_BYTES = 32L * 1024 * 1024

    /**
     * An id as a nav-route segment, and back. A document Uri carries `%` and
     * `/`, so the route must see it encoded exactly once on the way in.
     */
    fun encodeId(id: String): String = URLEncoder.encode(id, "UTF-8")

    fun decodeId(encoded: String): String = URLDecoder.decode(encoded, "UTF-8")

    /** The one place an id is opened: a path as a file, anything else as a document Uri. */
    fun open(context: Context, id: String): InputStream =
        if (isPath(id)) {
            File(id).inputStream()
        } else {
            context.contentResolver.openInputStream(Uri.parse(id))
                ?: throw FileNotFoundException(id)
        }

    /**
     * Every byte of the cart at [id].
     *
     * Throws whatever the read threw, so a caller can report it, and
     * [TooLargeException] past [MAX_ROM_BYTES]. Returns null only when a zip
     * has been opened and holds no cart at all - a separate case from "could
     * not read", and worth a different message.
     */
    fun read(context: Context, id: String): ByteArray? {
        if (isPath(id) && !isZip(id)) return readPlainFile(File(id))
        val zipped = isZip(fileName(context, id))
        if (zipped) withZipFile(context, id) { it.readCapped(MAX_ROM_BYTES) }?.let { return it.value }
        return readCart(zipped) { open(context, id) }
    }

    /**
     * The first [count] bytes of the cart, or fewer if it is shorter. For a
     * header: a zipped cart is inflated only as far as [count], not whole.
     */
    fun head(context: Context, id: String, count: Int): ByteArray? {
        val zipped = isZip(fileName(context, id))
        if (zipped) withZipFile(context, id) { readUpTo(it, count) }?.let { return it.value }
        return headOfCart(zipped, count) { open(context, id) }
    }

    /**
     * A zip through its central directory, as [ZipFile] reads it: the entry
     * list without inflating anything, which the stream fallback cannot do.
     * A document from the external-storage provider is a real file, so its
     * descriptor can be opened as a path. Null if the provider gives a pipe
     * or the descriptor cannot be opened that way - then [withCart] streams.
     * The outer null means "not handled"; [Handled.value] may itself be null
     * for a zip that holds no cart.
     */
    private fun <T> withZipFile(context: Context, id: String, block: (InputStream) -> T): Handled<T?>? {
        if (isPath(id)) return ZipFile(File(id)).use { Handled(useCart(it, block)) }
        val descriptor = runCatching {
            context.contentResolver.openFileDescriptor(Uri.parse(id), "r")
        }.getOrNull() ?: return null
        return descriptor.use { pfd ->
            val zip = runCatching { ZipFile(File("/proc/self/fd/${pfd.fd}")) }.getOrNull() ?: return null
            zip.use { Handled(useCart(it, block)) }
        }
    }

    private class Handled<T>(val value: T)

    private fun <T> useCart(zip: ZipFile, block: (InputStream) -> T): T? {
        val entry = zip.entries().asSequence()
            .filterNot { it.isDirectory }
            .map { it.name }
            .let(::chooseCart)
            ?.let(zip::getEntry) ?: return null
        return zip.getInputStream(entry).use(block)
    }

    /**
     * The cart a zip opens as. Sorted, so a zip holding several carts picks
     * the same one every launch: an arbitrary order would key save states on
     * a different game between runs.
     */
    private fun chooseCart(names: Sequence<String>): String? =
        names.sortedBy { it.lowercase() }.firstOrNull { name ->
            ROM_EXTENSIONS.any { name.endsWith(".$it", ignoreCase = true) }
        }

    private fun readUpTo(stream: InputStream, count: Int): ByteArray {
        val head = ByteArray(count)
        var read = 0
        while (read < count) {
            val n = stream.read(head, read, count - read)
            if (n <= 0) break
            read += n
        }
        return head.copyOf(read)
    }

    /**
     * Checked up front, then read in one sized allocation: a capped stream
     * would peak at twice the cart while it copies out.
     */
    fun readPlainFile(file: File): ByteArray {
        if (file.length() > MAX_ROM_BYTES) throw TooLargeException(MAX_ROM_BYTES)
        return file.readBytes()
    }

    /** [read] over any source of streams, so it runs without a Context. */
    fun readCart(zipped: Boolean, open: () -> InputStream): ByteArray? =
        withCart(zipped, open) { it.readCapped(MAX_ROM_BYTES) }

    /** [head] over any source of streams. */
    fun headOfCart(zipped: Boolean, count: Int, open: () -> InputStream): ByteArray? =
        withCart(zipped, open) { readUpTo(it, count) }

    /**
     * The fallback for a provider that only gives a stream. Two passes,
     * because a stream has no central directory: the first inflates through
     * the archive for the entry names, the second stops at the chosen one.
     * Same [chooseCart] rule, since save-state keys follow it.
     */
    private fun <T> withCart(zipped: Boolean, open: () -> InputStream, block: (InputStream) -> T): T? {
        if (!zipped) return open().use(block)
        val names = ZipInputStream(open()).use { zip ->
            generateSequence { zip.nextEntry }.filterNot { it.isDirectory }.map { it.name }.toList()
        }
        val chosen = chooseCart(names.asSequence()) ?: return null
        ZipInputStream(open()).use { zip ->
            while (true) {
                // Gone between the passes: the file changed under us.
                val entry = zip.nextEntry ?: return null
                if (!entry.isDirectory && entry.name == chosen) return block(zip)
            }
        }
    }

    /**
     * The name to show and to key saves on.
     *
     * For a zip this is the **archive's** name, not the entry's: the file on
     * disk is what the player renames, moves and recognises, and keying saves
     * on the entry would orphan them the moment somebody rezipped.
     */
    fun displayBaseName(name: String): String = File(name).nameWithoutExtension
}
