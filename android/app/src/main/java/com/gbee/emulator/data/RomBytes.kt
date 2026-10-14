package com.gbee.emulator.data

import android.content.Context
import android.net.Uri
import android.provider.OpenableColumns
import android.util.Log
import java.io.File
import java.io.FileNotFoundException
import java.io.InputStream
import java.util.Base64
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
     * An id as a nav-route segment, and back. Base64url, not percent encoding:
     * Navigation decodes a path argument itself, so a percent-encoded document
     * Uri arrived decoded twice (`%3A` became `:`) and the provider refused it.
     * Base64url has no `%`, `/` or `+` for that extra decode to touch.
     */
    fun encodeId(id: String): String =
        Base64.getUrlEncoder().withoutPadding().encodeToString(id.toByteArray(Charsets.UTF_8))

    fun decodeId(encoded: String): String = String(Base64.getUrlDecoder().decode(encoded), Charsets.UTF_8)

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
    fun read(context: Context, id: String): ByteArray? = when {
        isPath(id) && !isZip(id) -> readPlainFile(File(id))
        isZipId(context, id) -> withZip(context, id) { cartIn(it) { s -> s.readCapped(MAX_ROM_BYTES) } }
        else -> open(context, id).use { it.readCapped(MAX_ROM_BYTES) }
    }

    /**
     * The first [count] bytes of the cart, or fewer if it is shorter. For a
     * header: a zipped cart is inflated only as far as [count], not whole.
     */
    fun head(context: Context, id: String, count: Int): ByteArray? =
        if (isZipId(context, id)) {
            withZip(context, id) { cartIn(it) { s -> readUpTo(s, count) } }
        } else {
            open(context, id).use { readUpTo(it, count) }
        }

    /**
     * A zip by its name, or - when a provider gives no name - by its first
     * bytes. Defaulting to "not a zip" would read the archive itself as a
     * cart and key its saves on garbage.
     */
    private fun isZipId(context: Context, id: String): Boolean {
        if (isPath(id)) return isZip(id)
        val name = runCatching { fileName(context, id) }.getOrDefault("")
        if (name.isNotEmpty()) return isZip(name)
        return open(context, id).use { readUpTo(it, 4) }.contentEquals(ZIP_MAGIC)
    }

    private val ZIP_MAGIC = byteArrayOf(0x50, 0x4B, 0x03, 0x04)

    /**
     * Open a zip as [ZipFile], through its central directory: the entry list
     * without inflating anything. Every id ends up here as a real file, so a
     * path and a document pick the same cart the same way.
     *
     * - A path is opened directly.
     * - A document from the external-storage provider is a real file, so its
     *   descriptor opens as `/proc/self/fd/N`.
     * - Anything else (a provider that hands out a pipe) is copied into the
     *   cache first, capped, and deleted after.
     */
    private fun <T> withZip(context: Context, id: String, block: (ZipFile) -> T): T {
        if (isPath(id)) return ZipFile(File(id)).use(block)
        context.contentResolver.openFileDescriptor(Uri.parse(id), "r")?.use { pfd ->
            val direct = runCatching { ZipFile(File("/proc/self/fd/${pfd.fd}")) }
                .onFailure { Log.w(TAG, "zip descriptor not openable as a file, copying: $it") }
                .getOrNull()
            if (direct != null) return direct.use(block)
        }
        val copy = File.createTempFile("cart", ".zip", context.cacheDir)
        try {
            open(context, id).use { input -> copy.outputStream().use { input.copyCapped(it, MAX_ZIP_BYTES) } }
            return ZipFile(copy).use(block)
        } finally {
            copy.delete()
        }
    }

    /** A zip of a 32 MB cart is smaller than the cart; this leaves room for extras. */
    private const val MAX_ZIP_BYTES = 64L * 1024 * 1024

    private const val TAG = "GBee/Rom"

    /**
     * The cart a zip opens as, handed to [block]; null if it holds none.
     * Sorted, so a zip holding several carts picks the same one every launch:
     * an arbitrary order would key save states on a different game between
     * runs.
     */
    fun <T> cartIn(zip: ZipFile, block: (InputStream) -> T): T? {
        val entry = zip.entries().asSequence()
            .filterNot { it.isDirectory }
            .sortedBy { it.name.lowercase() }
            .firstOrNull { e -> ROM_EXTENSIONS.any { e.name.endsWith(".$it", ignoreCase = true) } }
            ?: return null
        return zip.getInputStream(entry).use(block)
    }

    fun readUpTo(stream: InputStream, count: Int): ByteArray {
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

    /**
     * The name to show and to key saves on.
     *
     * For a zip this is the **archive's** name, not the entry's: the file on
     * disk is what the player renames, moves and recognises, and keying saves
     * on the entry would orphan them the moment somebody rezipped.
     */
    fun displayBaseName(name: String): String = File(name).nameWithoutExtension
}
