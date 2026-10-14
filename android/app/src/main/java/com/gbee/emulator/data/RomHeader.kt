package com.gbee.emulator.data

import android.content.Context
import java.util.zip.CRC32

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

        /**
         * The state key for a whole cart.
         *
         * A cart with neither a title nor a game code - most homebrew - would
         * key as `ROM`, and every such cart would share one set of save
         * states: Celeste loaded the 240p suite's quick save. Those carts are
         * told apart by a CRC32 of the whole image instead. Carts with either
         * field key exactly as before.
         */
        fun stateKey(romData: ByteArray): String =
            from(romData)?.stateKey()?.takeIf { it != "ROM" }
                ?: "ROM_%08X".format(CRC32().apply { update(romData) }.value)

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
         * and the header is the first 176 bytes of it; a zipped cart is
         * inflated only that far.
         */
        fun read(context: Context, id: String): RomHeader? =
            runCatching { RomBytes.head(context, id, MIN_BYTES)?.let { from(it) } }.getOrNull()
    }
}
