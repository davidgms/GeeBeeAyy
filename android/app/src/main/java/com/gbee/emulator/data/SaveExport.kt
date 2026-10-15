package com.gbee.emulator.data

import android.content.Context
import android.net.Uri
import android.provider.DocumentsContract
import android.util.Log
import java.io.File
import java.io.FileNotFoundException
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

/**
 * Manual export of our battery saves into another emulator's folder: name
 * each save after its library game, show what would be replaced, and write the
 * ones the player confirms. [exportTargetName] and [dedupeTargets] are the pure
 * parts, pinned by JVM tests; the rest only glues them to the document provider.
 */
private const val TAG = "GBee/SaveExport"

/**
 * The file name [romFileName]'s save gets in a folder that already holds
 * [existing] names. When exactly one save-looking file there names the same
 * game, its exact name is reused (so `.srm` stays `.srm`); otherwise the other
 * emulators' common form, the ROM name with `.sav`.
 */
internal fun exportTargetName(romFileName: String, existing: List<String>): String {
    val wanted = normaliseSaveName(romFileName)
    val same = if (wanted.isEmpty()) emptyList() else existing.filter { isSaveName(it) && normaliseSaveName(it) == wanted }
    return same.singleOrNull() ?: (RomBytes.displayBaseName(romFileName) + ".sav")
}

/** [items] with a repeated key dropped after its first occurrence. */
internal fun <T> dedupeBy(items: List<T>, key: (T) -> String): List<T> {
    val seen = HashSet<String>()
    return items.filter { seen.add(key(it)) }
}

/**
 * One of our saves offered for export. [targetDocId] and [targetSize] describe
 * the file already in the folder under [targetName]; null id means a new one.
 */
data class SaveExportRow(
    val rom: RomEntry,
    val key: String,
    val targetName: String,
    val saveSize: Long,
    val targetDocId: String? = null,
    val targetSize: Long = 0L,
    val targetModified: Long = 0L,
    val selected: Boolean = false,
    /** Null for the battery save; otherwise the save-state slot this row carries. */
    val slot: Int? = null,
    /** The row is a state's `.ra` sidecar; it is only written along with [sidecar]'s owner. */
    val ra: Boolean = false,
    val sidecar: SaveExportRow? = null,
)

class SaveExportResult(val exported: Int, val skipped: Int, val failed: Int, val refused: List<String>)

object SaveExport {

    /**
     * Our saves, named for the picked [tree]'s own files (no recursion), or
     * null if the folder cannot be read. Reads every library game once to get
     * its save key, so call it off the main thread.
     */
    fun scan(context: Context, tree: Uri, roms: List<RomEntry>): List<SaveExportRow>? {
        val rootId = DocumentsContract.getTreeDocumentId(tree)
        val kids = RomFolderManager.listChildren(context, tree, rootId)?.filter { !it.isDir } ?: return null
        val names = kids.map { it.name }
        fun row(rom: RomEntry, key: String, file: File, name: String, slot: Int?, ra: Boolean, side: SaveExportRow? = null): SaveExportRow {
            val there = kids.firstOrNull { it.name == name }
            return SaveExportRow(
                rom = rom,
                key = key,
                targetName = name,
                saveSize = file.length(),
                targetDocId = there?.docId,
                targetSize = there?.size ?: 0L,
                targetModified = there?.modified ?: 0L,
                // A file already there is "leave it" until asked otherwise.
                selected = there == null,
                slot = slot,
                ra = ra,
                sidecar = side,
            )
        }
        val statesDir = File(context.filesDir, "states")
        val rows = dedupeBy(roms.filter { it.exists }.mapNotNull { rom ->
            RomFiles.stateKey(context, rom.filePath)?.let { rom to it }
        }) { it.second }.flatMap { (rom, key) ->
            val save = RomFiles.privateSave(context, key).takeIf { it.isFile && it.length() > 0 }
            listOfNotNull(save?.let { row(rom, key, it, exportTargetName(rom.fileName, names), null, false) }) +
                stateSlotFiles(statesDir, key).map { (slot, files) ->
                    val side = files.second?.let { row(rom, key, it, stateExportName(rom.fileName, slot, true, names), slot, true) }
                    row(rom, key, files.first, stateExportName(rom.fileName, slot, false, names), slot, false, side)
                }
        }
        return dedupeBy(rows) { it.targetName }
    }

    /**
     * Write the selected rows into [tree]. [openGame] is the library id of the
     * game now running, if any, whose pending save the caller has already flushed.
     * Pass null for it when so; pass the id when the flush failed and its file
     * may be behind the emulator's memory, so it is refused. Blocking.
     */
    fun apply(context: Context, tree: Uri, rows: List<SaveExportRow>, openGame: String?): SaveExportResult {
        val openKey = openGame?.let { RomFiles.stateKey(context, it) }
        var exported = 0
        var skipped = 0
        var failed = 0
        val refused = mutableListOf<String>()
        for (r in rows) {
            if (!r.selected) {
                skipped++
            } else if (r.rom.filePath == openGame || r.key == openKey || openGame != null && openKey == null) {
                refused += r.rom.name
                skipped++
            } else {
                val ok = put(context, tree, r)
                if (ok != false) r.sidecar?.let { put(context, tree, it) }
                when (ok) {
                    true -> exported++
                    null -> skipped++
                    false -> failed++
                }
            }
        }
        return SaveExportResult(exported, skipped, failed, refused)
    }

    private fun source(context: Context, r: SaveExportRow): File = when {
        r.slot == null -> RomFiles.privateSave(context, r.key)
        else -> File(File(context.filesDir, "states"), "${r.key}_slot${r.slot}" + if (r.ra) ".ra" else ".state")
    }

    /** [write] of [r]'s own file; failures (unreadable, empty) come back as false. */
    private fun put(context: Context, tree: Uri, r: SaveExportRow): Boolean? {
        val bytes = runCatching { source(context, r).readBytes() }.getOrNull()
        if (bytes == null || bytes.isEmpty()) return false
        return runCatching { write(context, tree, r, bytes) }
            .onFailure { Log.w(TAG, "export of ${r.targetName} failed: $it") }
            .getOrElse { false }
    }

    /**
     * True written and read back equal, null identical so left alone, false
     * failed. A failure leaves their folder as it was: a file we created is
     * deleted, a file we truncated gets its old bytes back.
     */
    private fun write(context: Context, tree: Uri, r: SaveExportRow, bytes: ByteArray): Boolean? {
        val resolver = context.contentResolver
        fun readBack(uri: Uri) = RomBytes.open(context, uri.toString()).use { it.readCapped(STATE_MAX_BYTES) }
        fun put(uri: Uri, data: ByteArray) {
            // "wt": a plain "w" need not truncate, and a shorter save would keep a stale tail.
            (resolver.openOutputStream(uri, "wt") ?: throw FileNotFoundException(uri.toString())).use { it.write(data) }
        }
        val existing = r.targetDocId?.let { DocumentsContract.buildDocumentUriUsingTree(tree, it) }
        if (existing != null) {
            val old = readBack(existing)
            if (old.contentEquals(bytes)) return null
            // Every replaced file is kept, app-private, before anything in their folder changes.
            val stamp = SimpleDateFormat("yyyyMMdd-HHmmss", Locale.US).format(Date())
            if (!atomicWrite(File(File(context.filesDir, "export-backups"), "${r.targetName}.$stamp"), old)) return false
            return runCatching { put(existing, bytes); readBack(existing).contentEquals(bytes) }.getOrDefault(false) ||
                run { runCatching { put(existing, old) }; false }
        }
        val parent = DocumentsContract.buildDocumentUriUsingTree(tree, DocumentsContract.getTreeDocumentId(tree))
        var doc = DocumentsContract.createDocument(resolver, parent, "application/octet-stream", r.targetName) ?: return false
        val ok = runCatching {
            put(doc, bytes)
            val name = resolver.query(doc, arrayOf(DocumentsContract.Document.COLUMN_DISPLAY_NAME), null, null, null)
                ?.use { c -> if (c.moveToFirst()) c.getString(0) else null }
            // Some providers append an extension to a new file; the other emulator needs the exact name.
            if (name != null && name != r.targetName) {
                doc = DocumentsContract.renameDocument(resolver, doc, r.targetName) ?: return@runCatching false
            }
            readBack(doc).contentEquals(bytes)
        }.getOrDefault(false)
        if (!ok) runCatching { DocumentsContract.deleteDocument(resolver, doc) }
        return ok
    }
}
