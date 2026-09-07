package com.geebeeayy.app.data

import android.graphics.Bitmap
import android.graphics.BitmapFactory
import java.io.File

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

    private val EXTENSIONS = listOf("png", "jpg", "jpeg", "webp")

    /** Largest edge we decode to. List thumbnails are ~56.dp. */
    private const val MAX_EDGE = 256

    /**
     * The artwork file for [romPath], or null if the player has not supplied
     * one. Checks `<name>.<ext>` beside the ROM first, then `covers/<name>.<ext>`.
     */
    fun findFile(romPath: String): File? {
        val rom = File(romPath)
        val dir = rom.parentFile ?: return null
        val base = rom.nameWithoutExtension
        for (ext in EXTENSIONS) {
            val beside = File(dir, "$base.$ext")
            if (beside.isFile) return beside
            val inCovers = File(File(dir, "covers"), "$base.$ext")
            if (inCovers.isFile) return inCovers
        }
        return null
    }

    /**
     * Decode the artwork for [romPath], downsampled to roughly [MAX_EDGE].
     *
     * Returns null when there is no artwork or the file is not a decodable
     * image - a corrupt or misnamed file falls back to the placeholder icon
     * rather than taking the list down.
     */
    fun load(romPath: String): Bitmap? {
        val file = findFile(romPath) ?: return null
        return try {
            val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
            BitmapFactory.decodeFile(file.absolutePath, bounds)
            if (bounds.outWidth <= 0 || bounds.outHeight <= 0) return null

            var sample = 1
            while (
                bounds.outWidth / sample > MAX_EDGE ||
                bounds.outHeight / sample > MAX_EDGE
            ) {
                sample *= 2
            }
            BitmapFactory.decodeFile(
                file.absolutePath,
                BitmapFactory.Options().apply { inSampleSize = sample },
            )
        } catch (e: Exception) {
            null
        }
    }
}
