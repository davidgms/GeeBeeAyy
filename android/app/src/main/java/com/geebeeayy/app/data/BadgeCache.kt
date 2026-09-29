package com.geebeeayy.app.data

import android.content.Context
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import java.io.File
import java.net.HttpURLConnection
import java.net.URL

/**
 * Achievement badges, fetched once and kept.
 *
 * The same posture as [CoverArt]: the app is a client fetching an image to a
 * player's device at their request, and nothing is ever bundled in the APK.
 * The difference is that a badge needs no setting of its own - somebody who
 * signed in to RetroAchievements has already asked for this.
 *
 * Badges are tiny (a few KB each) and there are at most a few hundred per
 * game, so the cache is left to grow rather than given an eviction policy it
 * would never reach.
 */
object BadgeCache {

    private fun dir(context: Context): File =
        File(context.filesDir, "badges").apply { mkdirs() }

    /**
     * The badge for [url], downloading it if this is the first time.
     *
     * Blocking: call it off the main thread. Returns null when there is no
     * URL, no network, or the bytes are not a decodable image - all of which
     * fall back to the icon the list already draws.
     */
    fun load(context: Context, url: String): Bitmap? {
        if (url.isBlank()) return null
        val file = File(dir(context), fileName(url))
        if (!file.isFile && !download(url, file)) return null
        return runCatching { BitmapFactory.decodeFile(file.absolutePath) }.getOrNull()
    }

    /**
     * The last path segment, which for RetroAchievements badges is the badge
     * id plus its state suffix - unique, short, and already a safe file name.
     * Falling back to the hash keeps a malformed URL from escaping the folder.
     */
    internal fun fileName(url: String): String {
        val last = url.substringAfterLast('/')
        return if (last.isNotBlank() && last.all { it.isLetterOrDigit() || it == '.' || it == '_' }) {
            last
        } else {
            "${url.hashCode().toUInt()}.png"
        }
    }

    private fun download(url: String, into: File): Boolean = runCatching {
        val connection = (URL(url).openConnection() as HttpURLConnection).apply {
            connectTimeout = 10_000
            readTimeout = 20_000
            instanceFollowRedirects = true
        }
        try {
            if (connection.responseCode != HttpURLConnection.HTTP_OK) return@runCatching false
            // Written beside the target and renamed, so a dropped connection
            // cannot leave a half-decoded image that looks cached.
            val temp = File(into.parentFile, "${into.name}.part")
            connection.inputStream.use { input ->
                temp.outputStream().use { output -> input.copyTo(output) }
            }
            temp.renameTo(into) || run { temp.delete(); false }
        } finally {
            connection.disconnect()
        }
    }.getOrDefault(false)

    fun clear(context: Context) {
        dir(context).listFiles()?.forEach { it.delete() }
    }

    fun bytes(context: Context): Long =
        dir(context).listFiles()?.sumOf { it.length() } ?: 0L
}
