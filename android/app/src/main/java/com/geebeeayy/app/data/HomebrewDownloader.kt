package com.geebeeayy.app.data

import android.content.Context
import android.net.Uri
import android.provider.DocumentsContract
import android.util.Log
import java.io.File
import java.io.FileNotFoundException
import java.io.IOException
import java.net.HttpURLConnection
import java.net.URL
import java.security.DigestOutputStream
import java.security.MessageDigest

/**
 * Fetches a [HomebrewEntry] into the ROM folder.
 *
 * Plain `HttpURLConnection` - the app has no HTTP dependency and one download
 * button does not justify adding one.
 */
object HomebrewDownloader {

    private const val TAG = "GeeBeeAyy/Download"

    /**
     * What a player reads when a download fails. The exception's own text
     * ("Software caused connection abort") is for the log, not the screen.
     * A disk error is told apart by its errno, because the platform raises
     * a plain IOException for both a full disk and a stream cut short.
     */
    internal fun failureMessage(e: Exception): String {
        val text = e.message.orEmpty()
        return when {
            e is TooLargeException -> MISMATCH
            e is java.net.SocketTimeoutException -> "The server took too long to answer. Try again later."
            DISK_ERRORS.any { it in text } -> "Could not save the file. Check that the folder has free space."
            e is java.io.IOException -> "Connection lost. Check your internet and try again."
            else -> "Download failed."
        }
    }

    private const val MISMATCH = "The download is not the file this app checked. Not saved."

    private val DISK_ERRORS = listOf("ENOSPC", "EACCES", "EROFS", "EDQUOT")
    private const val TIMEOUT_MS = 30_000

    sealed interface Result {
        /** [location] is the file's path, or its document Uri string in a tree. */
        data class Done(val location: String) : Result
        data class Failed(val reason: String) : Result
    }

    /** The file names in [folder] (a path or a tree Uri string); empty if it cannot be read. Blocking. */
    fun existing(context: Context, folder: String): Set<String> =
        if (RomBytes.isPath(folder)) {
            File(folder).list()?.toSet().orEmpty()
        } else {
            val tree = Uri.parse(folder)
            RomFolderManager.listChildren(context, tree, DocumentsContract.getTreeDocumentId(tree))
                ?.filterNot { it.isDir }?.mapTo(HashSet()) { it.name }.orEmpty()
        }

    /**
     * Download [entry] into [folder], reporting progress as a 0..1 fraction.
     *
     * A path gets a `.part` file beside the target, renamed on success. A
     * tree is downloaded, sized and hashed in the app's own cache first and
     * only then written into the folder, so no half file ever shows up in the
     * player's library. Blocking - call it off the main thread.
     */
    fun download(
        context: Context,
        entry: HomebrewEntry,
        folder: String,
        onProgress: (Float) -> Unit = {},
    ): Result =
        if (RomBytes.isPath(folder)) {
            download(entry, File(folder), onProgress)
        } else {
            val partial = File(context.cacheDir, "homebrew-${entry.fileName}.part")
            transfer(entry, partial, onProgress, { null }) {
                copyIntoTree(context, entry, Uri.parse(folder), partial)
            }
        }

    /**
     * Download [entry] into [destDir], reporting progress as a 0..1 fraction.
     *
     * Writes to a `.part` file and renames on success, so an interrupted
     * download cannot leave something that looks like a playable ROM in the
     * library. Blocking - call it off the main thread.
     */
    fun download(
        entry: HomebrewEntry,
        destDir: File,
        onProgress: (Float) -> Unit = {},
    ): Result {
        val target = File(destDir, entry.fileName)
        val partial = File(destDir, "${entry.fileName}.part")
        val prepare = {
            if (!destDir.isDirectory && !destDir.mkdirs()) Result.Failed("Cannot write to ${destDir.name}") else null
        }
        return transfer(entry, partial, onProgress, prepare) {
            if (target.exists() && !target.delete()) {
                Result.Failed("Could not replace the existing file")
            } else if (!partial.renameTo(target)) {
                Result.Failed("Could not move the download into place")
            } else {
                Result.Done(target.absolutePath)
            }
        }
    }

    /**
     * The shared part: fetch into [partial], check size and SHA-256, hand it
     * to [place]. [partial] is gone afterwards whatever happened.
     */
    private fun transfer(
        entry: HomebrewEntry,
        partial: File,
        onProgress: (Float) -> Unit,
        prepare: () -> Result.Failed?,
        place: () -> Result,
    ): Result {
        var connection: HttpURLConnection? = null
        try {
            prepare()?.let { return it }
            connection = (URL(entry.url).openConnection() as HttpURLConnection).apply {
                connectTimeout = TIMEOUT_MS
                readTimeout = TIMEOUT_MS
                instanceFollowRedirects = true
            }
            val code = connection.responseCode
            if (code != HttpURLConnection.HTTP_OK) {
                return Result.Failed("Server returned $code")
            }

            val digest = MessageDigest.getInstance("SHA-256")
            val written = connection.inputStream.use { input ->
                DigestOutputStream(partial.outputStream(), digest).use { output ->
                    input.copyCapped(output, entry.bytes) { done ->
                        onProgress((done.toFloat() / entry.bytes).coerceIn(0f, 1f))
                    }
                }
            }

            if (written == 0L) {
                return Result.Failed("Downloaded nothing")
            }
            val hash = digest.digest().joinToString("") { "%02x".format(it) }
            if (written != entry.bytes || hash != entry.sha256) {
                Log.w(TAG, "${entry.name}: got $written bytes, sha256 $hash")
                return Result.Failed(MISMATCH)
            }
            val result = place()
            if (result is Result.Done) onProgress(1f)
            return result
        } catch (e: Exception) {
            Log.e(TAG, "Downloading ${entry.name} failed", e)
            return Result.Failed(failureMessage(e))
        } finally {
            partial.delete()
            connection?.disconnect()
        }
    }

    /**
     * Copy the verified [partial] into the tree, over a file of the same name
     * if there is one (written in place: a delete-then-create would lose the
     * old copy if the create failed). A new file is made with a generic mime
     * type, and renamed if the provider added an extension to it.
     */
    private fun copyIntoTree(context: Context, entry: HomebrewEntry, tree: Uri, partial: File): Result {
        val resolver = context.contentResolver
        val rootId = DocumentsContract.getTreeDocumentId(tree)
        val children = RomFolderManager.listChildren(context, tree, rootId)
            ?: return Result.Failed("Cannot write to that folder. Pick it again in Settings.")
        val old = children.firstOrNull { !it.isDir && it.name == entry.fileName }
        val target = old?.let { DocumentsContract.buildDocumentUriUsingTree(tree, it.docId) }
            ?: DocumentsContract.createDocument(
                resolver,
                DocumentsContract.buildDocumentUriUsingTree(tree, rootId),
                "application/octet-stream",
                entry.fileName,
            )
            ?: return Result.Failed("Could not create the file in that folder")
        try {
            val out = resolver.openOutputStream(target, "wt") ?: throw FileNotFoundException(target.toString())
            out.use { partial.inputStream().use { input -> input.copyTo(it) } }
            val name = resolver
                .query(target, arrayOf(DocumentsContract.Document.COLUMN_DISPLAY_NAME), null, null, null)
                ?.use { if (it.moveToFirst()) it.getString(0) else null }
            if (name != null && name != entry.fileName) {
                DocumentsContract.renameDocument(resolver, target, entry.fileName)
                    ?: throw IOException("the provider named the file $name")
            }
        } catch (e: Exception) {
            if (old == null) runCatching { DocumentsContract.deleteDocument(resolver, target) }
            throw e
        }
        return Result.Done(target.toString())
    }
}
