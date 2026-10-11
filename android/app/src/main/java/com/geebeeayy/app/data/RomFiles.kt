package com.geebeeayy.app.data

import android.content.Context
import android.net.Uri
import android.provider.DocumentsContract
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

    /**
     * One thing [deleteSaveData] would remove: a file, or the `.sav` document
     * beside a document ROM. [legacy] marks the one the player put there,
     * which goes last.
     */
    class SaveFile(val name: String, val file: File?, val document: String?, val legacy: Boolean)

    /** The battery save and every save state for [romPath] that exists. */
    fun saveData(context: Context, romPath: String): List<SaveFile> {
        val files = mutableListOf<SaveFile>()
        if (RomBytes.isPath(romPath)) {
            files += fileItem(legacySave(File(romPath)), true)
        } else {
            RomSiblings.get(romPath)?.let { sib ->
                sib.saveUri?.let { files += SaveFile("${RomBytes.displayBaseName(sib.name)}.sav", null, it, true) }
            }
        }
        val key = stateKey(context, romPath)
        if (key != null) {
            val save = privateSave(context, key)
            files += fileItem(save, false)
            files += fileItem(File(save.parentFile, "${save.name}.before-import"), false)
            val states = File(context.filesDir, "states")
            states.listFiles()?.filter { it.name.startsWith("${key}_slot") }
                ?.forEach { files += fileItem(it, false) }
        }
        return files.filter { it.document != null || it.file?.isFile == true }
    }

    private fun fileItem(file: File, legacy: Boolean) = SaveFile(file.name, file, null, legacy)

    fun delete(context: Context, save: SaveFile): Boolean =
        save.file?.delete() ?: deleteDocument(context, save.document!!)

    private fun deleteDocument(context: Context, uri: String): Boolean =
        runCatching { DocumentsContract.deleteDocument(context.contentResolver, Uri.parse(uri)) }
            .getOrDefault(false)

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
     * The `.sav` beside [id] as an import sees it, or null when that is not
     * known yet: a document ROM the scan has not listed, or one whose `.sav`
     * could not be sized. Null means skip and try again - never "absent", or
     * [importLegacySave] would mark the cart done and a real save would never
     * come in.
     */
    internal fun legacySaveFor(context: Context, id: String): LegacySave? {
        if (RomBytes.isPath(id)) return legacySave(File(id)).asLegacySave()
        val sib = RomSiblings.get(id) ?: return null
        val uri = sib.saveUri ?: return LegacySave(false, 0L, 0L) { ByteArray(0) }
        if (sib.saveSize != null) {
            return LegacySave(true, sib.saveSize, sib.saveModified) {
                RomBytes.open(context, uri).use { it.readCapped(MAX_SAVE_BYTES) }
            }
        }
        // A provider that does not report SIZE: measure by reading, capped one
        // past the limit so "too big" is told apart from "unreadable".
        val bytes = runCatching { RomBytes.open(context, uri).use { RomBytes.readUpTo(it, MAX_SAVE_BYTES.toInt() + 1) } }
            .getOrNull() ?: return null
        return LegacySave(true, bytes.size.toLong(), sib.saveModified) { bytes }
    }

    private fun hasLegacySave(id: String): Boolean =
        if (RomBytes.isPath(id)) legacySave(File(id)).isFile else RomSiblings.get(id)?.saveUri != null

    private fun legacyModified(id: String): Long =
        if (RomBytes.isPath(id)) legacySave(File(id)).lastModified() else RomSiblings.get(id)?.saveModified ?: 0L

    /**
     * Import every `.sav` still beside a cart into app storage, once each.
     * Runs on every scan, so saves for carts the player has not opened since
     * the update are moved too, while the app can still read the folder - and
     * for a cart in a document tree, the first scan after the folder is
     * re-picked brings in a `.sav` the old permission path missed.
     * A cart with no `.sav` beside it is skipped before its header is read.
     * ponytail: a cart that has one is re-keyed on every scan (a full read
     * for a header-less cart) because the legacy file is never deleted;
     * cache the key per path if a library with many such carts gets slow.
     */
    fun importLegacySaves(context: Context, roms: List<RomEntry>) {
        roms.filter { hasLegacySave(it.filePath) }
            .mapNotNull { entry -> stateKey(context, entry.filePath)?.let { it to entry } }
            .groupBy({ it.first }, { it.second })
            .forEach { (key, carts) ->
                // Two copies of one cart (a .gba and a .zip of it) had a .sav
                // each and now share one key. The copy played last is the
                // save the player last saw; the other stays where it is.
                val chosen = carts.maxWith(
                    compareBy({ it.lastPlayedMillis ?: 0L }, { legacyModified(it.filePath) }),
                )
                legacySaveFor(context, chosen.filePath)?.let { importLegacySave(it, privateSave(context, key)) }
            }
    }

    private fun stateKey(context: Context, rom: String): String? =
        RomHeader.read(context, rom)?.stateKey()?.takeIf { it != "ROM" }
            ?: runCatching { RomBytes.read(context, rom) }.getOrNull()?.let { RomHeader.stateKey(it) }

    /**
     * True if every file was removed. Every file is tried, the app's own
     * first: a `.sav` beside the ROM that will not delete must not leave
     * the private save behind.
     */
    fun deleteSaveData(context: Context, romPath: String): Boolean =
        synchronized(SAVE_WRITE_LOCK) {
            saveData(context, romPath).sortedBy { it.legacy }.map { delete(context, it) }.all { it }
        }

    /** Removes the cart itself and everything keyed to it. */
    fun deleteRom(context: Context, romPath: String): Boolean {
        // Saves first: their names are read out of the ROM's own header, so
        // deleting the cart first would leave the app-private copies
        // unfindable and orphaned.
        deleteSaveData(context, romPath)
        if (RomBytes.isPath(romPath)) {
            RomArtwork.findFile(romPath)?.delete()
            return File(romPath).delete()
        }
        RomSiblings.get(romPath)?.coverUri?.let { deleteDocument(context, it) }
        return deleteDocument(context, romPath)
    }
}
