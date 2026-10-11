package com.geebeeayy.app.data

import java.io.ByteArrayOutputStream
import java.io.IOException
import java.io.InputStream
import java.io.OutputStream

/** A read went past its byte ceiling. */
class TooLargeException(val limit: Long) :
    IOException(if (limit >= 1 shl 20) "File is over ${limit shr 20} MB" else "File is over $limit bytes")

/**
 * Copy at most [limit] bytes, and throw [TooLargeException] past it.
 *
 * The ceiling is counted on what actually arrives, not on a Content-Length or
 * a zip entry's declared size: both come from whoever made the file, and a
 * zip bomb or an endless response lies in exactly those fields.
 */
fun InputStream.copyCapped(out: OutputStream, limit: Long, onProgress: (Long) -> Unit = {}): Long {
    val buffer = ByteArray(64 * 1024)
    var total = 0L
    while (true) {
        val n = read(buffer)
        if (n < 0) return total
        total += n
        if (total > limit) throw TooLargeException(limit)
        out.write(buffer, 0, n)
        onProgress(total)
    }
}

fun InputStream.readCapped(limit: Long): ByteArray =
    ByteArrayOutputStream().also { copyCapped(it, limit) }.toByteArray()
