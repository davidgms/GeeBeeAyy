package com.geebeeayy.app.data

import java.io.File

/**
 * The two identifying fields of a GBA cartridge header.
 *
 * GBATEK, *GBA Cartridge Header*: the game title is 12 bytes at 0xA0 and the
 * game code is 4 bytes at 0xAC, both ASCII, both zero-padded.
 */
data class RomHeader(val title: String, val gameCode: String) {

    /**
     * The filename fragment save states and battery saves are keyed on.
     *
     * Two carts with the same file name must not collide on the same slot
     * file, and one cart must key the same way on every launch - so this is
     * derived from the header rather than the path. **Changing it orphans
     * every save state already on disk**, which is why
     * `RomHeaderTest` pins the exact output.
     */
    fun stateKey(): String {
        val raw = when {
            gameCode.isNotBlank() && title.isNotBlank() -> "${title}_$gameCode"
            gameCode.isNotBlank() -> gameCode
            title.isNotBlank() -> title
            else -> "rom"
        }
        return raw.uppercase().replace(Regex("[^A-Z0-9_]"), "_").ifBlank { "rom" }
    }

    companion object {
        /** Bytes needed before the header's fields are all present. */
        const val MIN_BYTES = 0xB0

        fun from(romData: ByteArray): RomHeader? {
            if (romData.size < MIN_BYTES) return null
            fun ascii(offset: Int, len: Int): String =
                String(romData, offset, len, Charsets.US_ASCII)
                    .substringBefore('\u0000')
                    .trim()
            return RomHeader(ascii(0xA0, 12), ascii(0xAC, 4))
        }

        /**
         * Read a header without reading the whole cart. A ROM is up to 32 MB
         * and the header is the first 176 bytes of it.
         */
        fun read(file: File): RomHeader? = runCatching {
            file.inputStream().use { stream ->
                val head = ByteArray(MIN_BYTES)
                var read = 0
                while (read < MIN_BYTES) {
                    val n = stream.read(head, read, MIN_BYTES - read)
                    if (n <= 0) break
                    read += n
                }
                if (read < MIN_BYTES) null else from(head)
            }
        }.getOrNull()
    }
}
