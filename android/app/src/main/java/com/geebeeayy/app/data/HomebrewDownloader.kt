package com.geebeeayy.app.data

import android.util.Log
import java.io.File
import java.net.HttpURLConnection
import java.net.URL

/**
 * Fetches a [HomebrewEntry] into the ROM folder.
 *
 * Plain `HttpURLConnection` - the app has no HTTP dependency and one download
 * button does not justify adding one.
 */
object HomebrewDownloader {

    private const val TAG = "GeeBeeAyy/Download"
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

            // Content-Length is a hint, not a guarantee; fall back to the
            // catalogue's figure so the bar still moves when it is absent.
            val expected = connection.contentLengthLong.takeIf { it > 0 } ?: entry.approxBytes

            var written = 0L
            connection.inputStream.use { input ->
                partial.outputStream().use { output ->
                    val buffer = ByteArray(64 * 1024)
                    while (true) {
                        val read = input.read(buffer)
                        if (read < 0) break
                        output.write(buffer, 0, read)
                        written += read
                        onProgress((written.toFloat() / expected).coerceIn(0f, 1f))
                    }
                }
            }

            if (written == 0L) {
                partial.delete()
                return Result.Failed("Downloaded nothing")
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
            return Result.Failed(e.message ?: e.javaClass.simpleName)
        } finally {
            connection?.disconnect()
        }
    }
}
