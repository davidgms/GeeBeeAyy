package com.geebeeayy.app.data

import android.content.Context
import java.io.File

/**
 * Everything on disk that belongs to one ROM, and how to remove it.
 *
 * The layout is `EmulationViewModel`'s, restated here rather than reached
 * into: the battery save is `<rom base name>.sav` beside the cart, with an
 * app-private `saves/<state key>.sav` standing in when the ROM's folder is not
 * writable, and the save states are `states/<state key>_slotN.state`.
 * **Changing either shape here without changing it there deletes nothing and
 * reports success**, so `RomFilesTest` pins the names.
 */
object RomFiles {

    /** The battery save and every save state for [romPath] that exists. */
    fun saveData(context: Context, romPath: String): List<File> {
        val rom = File(romPath)
        val files = mutableListOf<File>()
        rom.parentFile?.let { dir -> files += File(dir, "${rom.nameWithoutExtension}.sav") }
        val key = stateKey(rom)
        if (key != null) {
            files += File(File(context.filesDir, "saves"), "$key.sav")
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
    private fun stateKey(rom: File): String? =
        RomHeader.read(rom)?.stateKey()

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
