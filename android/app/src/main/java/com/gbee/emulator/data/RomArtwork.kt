package com.gbee.emulator.data

import android.content.Context
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.net.Uri
import java.io.File
import java.io.InputStream

/**
 * Cover art for a ROM, loaded from an image file the player supplies.
 *
 * Deliberately local-only. The art is whatever sits next to the ROM under one
 * of [EXTENSIONS] with the same base name, or in a `covers/` folder beside it -
 * so `Sonic Advance.gba` is illustrated by `Sonic Advance.png`. Nothing is
 * fetched, so there is no metadata service to choose, no network permission to
 * justify, and no question about who owns the image.
 *
 * It is also *not* a screenshot of the last save. A thumbnail that changes
 * every time you put the game down makes the list harder to scan, not easier;
 * a fixed picture is what lets you find a game by shape.
 */
object RomArtwork {

    /** Largest edge we decode to. List thumbnails are ~56.dp. */
    private const val MAX_EDGE = 256

    /**
     * The artwork file beside the ROM at path [romPath], or null if the player has not supplied
     * one. Checks `<name>.<ext>` beside the ROM first, then `covers/<name>.<ext>`.
     */
    fun findFile(romPath: String): File? {
        val rom = File(romPath)
        val dir = rom.parentFile ?: return null
        val base = rom.nameWithoutExtension
        for (ext in ART_EXTENSIONS) {
            val beside = File(dir, "$base.$ext")
            if (beside.isFile) return beside
            val inCovers = File(File(dir, "covers"), "$base.$ext")
            if (inCovers.isFile) return inCovers
        }
        return null
    }

    /**
     * The same search, plus [CoverArt]'s download cache as a last resort.
     * A document ROM has no file beside it to find: its own cover is a
     * document the scan recorded, read by [load].
     *
     * The player's own file wins over a downloaded one on purpose: they put it
     * there, and a fetched cover should never quietly replace it.
     */
    fun findFile(context: Context, romPath: String): File? =
        (if (RomBytes.isPath(romPath)) findFile(romPath) else null)
            ?: CoverArt.cachedFile(context, romPath).takeIf { it.isFile }

    /**
     * Decode the artwork for [romPath], downsampled to roughly [MAX_EDGE].
     *
     * Returns null when there is no artwork or the file is not a decodable
     * image - a corrupt or misnamed file falls back to the placeholder icon
     * rather than taking the list down.
     */
    fun load(context: Context, romPath: String): Bitmap? {
        if (!RomBytes.isPath(romPath)) {
            RomSiblings.get(romPath)?.coverUri?.let { uri ->
                decode { context.contentResolver.openInputStream(Uri.parse(uri)) }?.let { return it }
            }
        }
        return findFile(context, romPath)?.let { file -> decode { file.inputStream() } }
    }

    private fun decode(open: () -> InputStream?): Bitmap? {
        return try {
            val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
            open()?.use { BitmapFactory.decodeStream(it, null, bounds) }
            if (bounds.outWidth <= 0 || bounds.outHeight <= 0) return null

            var sample = 1
            while (
                bounds.outWidth / sample > MAX_EDGE ||
                bounds.outHeight / sample > MAX_EDGE
            ) {
                sample *= 2
            }
            open()?.use {
                BitmapFactory.decodeStream(it, null, BitmapFactory.Options().apply { inSampleSize = sample })
            }
        } catch (e: Exception) {
            null
        }
    }
}
