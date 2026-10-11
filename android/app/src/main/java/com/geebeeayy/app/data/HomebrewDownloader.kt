package com.geebeeayy.app.data

import android.util.Log
import java.io.File
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
        data class Done(val file: File) : Result
        data class Failed(val reason: String) : Result
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
        var connection: HttpURLConnection? = null
        try {
            if (!destDir.isDirectory && !destDir.mkdirs()) {
                return Result.Failed("Cannot write to ${destDir.name}")
            }
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
                partial.delete()
                return Result.Failed("Downloaded nothing")
            }
            val hash = digest.digest().joinToString("") { "%02x".format(it) }
            if (written != entry.bytes || hash != entry.sha256) {
                Log.w(TAG, "${entry.name}: got $written bytes, sha256 $hash")
                partial.delete()
                return Result.Failed(MISMATCH)
            }
            if (target.exists() && !target.delete()) {
                partial.delete()
                return Result.Failed("Could not replace the existing file")
            }
            if (!partial.renameTo(target)) {
                partial.delete()
                return Result.Failed("Could not move the download into place")
            }
            onProgress(1f)
            return Result.Done(target)
        } catch (e: Exception) {
            Log.e(TAG, "Downloading ${entry.name} failed", e)
            partial.delete()
            return Result.Failed(failureMessage(e))
        } finally {
            connection?.disconnect()
        }
    }
}
