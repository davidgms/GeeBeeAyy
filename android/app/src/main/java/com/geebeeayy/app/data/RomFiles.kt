package com.geebeeayy.app.data

import android.content.Context
import java.io.File

/**
 * Everything on disk that belongs to one ROM, and how to remove it.
 *
 * The layout is `EmulationViewModel`'s, restated here rather than reached
 * into: the battery save is app-private `saves/<state key>.sav`, the save
 * states are `states/<state key>_slotN.state`, and `<rom base name>.sav`
 * beside the cart is the old location, imported once by [importLegacySaves].
 * **Changing either shape here without changing it there deletes nothing and
 * reports success**, so `RomFilesTest` pins the names.
 */
object RomFiles {

    /** The battery save and every save state for [romPath] that exists. */
    fun saveData(context: Context, romPath: String): List<File> {
        val rom = File(romPath)
        val files = mutableListOf<File>()
        files += legacySave(rom)
        val key = stateKey(rom)
        if (key != null) {
            files += privateSave(context, key)
            val states = File(context.filesDir, "states")
            states.listFiles()?.filterTo(files) { it.name.startsWith("${key}_slot") }
        }
        return files.filter { it.isFile }
    }

    /**
     * The key the saves are named with, read from the cart header.
     *
     * Null when the ROM is gone or unreadable - in which case the app-private
     * copies cannot be found by name and only the `.sav` beside the cart is
     * offered. Better than deleting something that belongs to another game.
     */
    /** Where the battery save lives. */
    fun privateSave(context: Context, key: String): File =
        File(File(context.filesDir, "saves"), "$key.sav")

    /** Where the battery save used to live, beside the cart. */
    fun legacySave(rom: File): File = File(rom.parentFile, "${rom.nameWithoutExtension}.sav")

    /**
     * Import every `.sav` still beside a cart into app storage, once each.
     * Runs on every scan, so saves for carts the player has not opened since
     * the update are moved too, while the app can still read the folder.
     * Cheap after the first run: a cart with no `.sav` beside it is skipped
     * before its header is read, and an imported one stops at its marker.
     */
    fun importLegacySaves(context: Context, roms: List<RomEntry>) {
        roms.filter { legacySave(File(it.filePath)).isFile }
            .mapNotNull { entry -> stateKey(File(entry.filePath))?.let { it to entry } }
            .groupBy({ it.first }, { it.second })
            .forEach { (key, carts) ->
                // Two copies of one cart (a .gba and a .zip of it) had a .sav
                // each and now share one key. The copy played last is the
                // save the player last saw; the other stays where it is.
                val chosen = carts.maxBy { it.lastPlayedMillis ?: 0L }
                importLegacySave(legacySave(File(chosen.filePath)), privateSave(context, key))
            }
    }

    private fun stateKey(rom: File): String? =
        RomHeader.read(rom)?.stateKey()?.takeIf { it != "ROM" }
            ?: runCatching { RomBytes.read(rom.path) }.getOrNull()?.let { RomHeader.stateKey(it) }

    /** True if every file was removed. */
    fun deleteSaveData(context: Context, romPath: String): Boolean =
        saveData(context, romPath).all { it.delete() }

    /** Removes the cart itself and everything keyed to it. */
    fun deleteRom(context: Context, romPath: String): Boolean {
        // Saves first: their names are read out of the ROM's own header, so
        // deleting the cart first would leave the app-private copies
        // unfindable and orphaned.
        deleteSaveData(context, romPath)
        RomArtwork.findFile(romPath)?.delete()
        return File(romPath).delete()
    }
}
