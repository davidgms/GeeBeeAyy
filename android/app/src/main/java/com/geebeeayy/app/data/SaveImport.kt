package com.geebeeayy.app.data

import android.content.Context
import android.net.Uri
import android.provider.DocumentsContract
import android.util.Log

/**
 * Manual import of battery saves from another emulator's folder: find the
 * files, pair each with a library game, check it fits that game's save chip,
 * and write the ones the player confirms. The pure parts are pinned by JVM
 * tests; the rest only glues them to the document provider.
 */
private const val TAG = "GeeBeeAyy/SaveImport"

internal val SAVE_EXTENSIONS = listOf("sav", "srm", "sa1")

internal fun isSaveName(name: String): Boolean =
    SAVE_EXTENSIONS.any { name.endsWith(".$it", ignoreCase = true) }

private val TAG_GROUPS = Regex("""\([^)]*\)|\[[^\]]*]""")

/**
 * The key two names are compared by: lowercase, extension gone, `(USA)` and
 * `[!]` groups gone, then only letters and digits. Empty means "nothing to
 * match on", which never matches.
 */
internal fun normaliseSaveName(name: String): String =
    name.substringBeforeLast('.').lowercase().replace(TAG_GROUPS, "").filter { it.isLetterOrDigit() }

/** The one library game [saveName] names, or null if none or several do. */
internal fun matchRom(saveName: String, roms: List<RomEntry>): RomEntry? {
    val wanted = normaliseSaveName(saveName).ifEmpty { return null }
    return roms.filter { it.exists && normaliseSaveName(it.fileName) == wanted }.singleOrNull()
}

/**
 * Every save-looking file under [rootDocId], 1 byte to [MAX_SAVE_BYTES]; null
 * if the root cannot be read. [scanTree] is the one tree walker and lists each
 * directory once, so the listings it asks for are the whole tree.
 */
internal fun findSaves(rootDocId: String, children: (String) -> List<DocChild>?): List<DocChild>? {
    val seen = ArrayList<DocChild>()
    scanTree(rootDocId) { id -> children(id)?.also { seen += it } } ?: return null
    return seen.filter { !it.isDir && isSaveName(it.name) && (it.size ?: 1L) in 1..MAX_SAVE_BYTES }
}

/**
 * One file offered for import. [rom] null means unmatched (the player picks);
 * a [rom] with no [keep] means it does not fit that game, or [key] null that
 * the game could not be read. [existingSize] is 0 when the game has no save.
 */
data class SaveImportRow(
    val fileName: String,
    val uri: String,
    val size: Long,
    val modified: Long,
    val rom: RomEntry? = null,
    val key: String? = null,
    val keep: Int? = null,
    val existingSize: Long = 0L,
    val existingModified: Long = 0L,
    val selected: Boolean = false,
) {
    val importable get() = keep != null
}

class SaveImportResult(val imported: Int, val skipped: Int, val failed: Int, val refused: List<String>)

object SaveImport {

    /** The saves under a picked [tree], or null if it cannot be read. Blocking. */
    fun scan(context: Context, tree: Uri, roms: List<RomEntry>, saveFit: (ByteArray, Int) -> Int?): List<SaveImportRow>? {
        val found = findSaves(DocumentsContract.getTreeDocumentId(tree)) {
            RomFolderManager.listChildren(context, tree, it)
        } ?: return null
        return found.map { f ->
            val uri = DocumentsContract.buildDocumentUriUsingTree(tree, f.docId).toString()
            row(context, f.name, uri, f.size ?: -1L, f.modified, matchRom(f.name, roms), saveFit)
        }
    }

    /** [row] re-evaluated against [rom], after the player picked it. Blocking. */
    fun pick(context: Context, row: SaveImportRow, rom: RomEntry, saveFit: (ByteArray, Int) -> Int?) =
        row(context, row.fileName, row.uri, row.size, row.modified, rom, saveFit)

    private fun row(
        context: Context,
        name: String,
        uri: String,
        size: Long,
        modified: Long,
        rom: RomEntry?,
        saveFit: (ByteArray, Int) -> Int?,
    ): SaveImportRow {
        val bare = SaveImportRow(name, uri, size, modified, rom)
        if (rom == null) return bare
        // One read of the game per row; the key and the fit check both need it.
        val cart = runCatching { RomBytes.read(context, rom.filePath) }.getOrNull() ?: return bare
        val len = if (size >= 0) size else runCatching { read(context, uri).size.toLong() }.getOrNull() ?: return bare
        val key = RomHeader.stateKey(cart)
        val keep = saveFit(cart, len.toInt())
        val existing = RomFiles.privateSave(context, key).takeIf { it.isFile && it.length() > 0 }
        return bare.copy(
            size = len,
            key = key,
            keep = keep,
            existingSize = existing?.length() ?: 0L,
            existingModified = existing?.lastModified() ?: 0L,
            // A game that already has a save is "keep mine" until asked otherwise.
            selected = keep != null && existing == null,
        )
    }

    private fun read(context: Context, uri: String): ByteArray =
        RomBytes.open(context, uri).use { it.readCapped(MAX_SAVE_BYTES) }

    /**
     * Write the selected rows. [openGame] is the library id of the game now
     * running, if any: writing its save would be flushed over by the emulator
     * (or lost on its next load), so it is refused. Blocking.
     */
    fun apply(context: Context, rows: List<SaveImportRow>, openGame: String?): SaveImportResult {
        val openKey = openGame?.let { runCatching { RomBytes.read(context, it) }.getOrNull()?.let(RomHeader::stateKey) }
        var imported = 0
        var skipped = 0
        var failed = 0
        val refused = mutableListOf<String>()
        val done = HashSet<String>()
        for (r in rows) {
            val key = r.key
            val keep = r.keep
            if (!r.selected || key == null || keep == null) {
                skipped++
            } else if (r.rom?.filePath == openGame || key == openKey || openGame != null && openKey == null) {
                // An open game whose key cannot be read could be any row's.
                refused += r.rom?.name ?: key
                skipped++
            } else if (!done.add(key)) {
                // Two files for one game: the first selected one wins.
                skipped++
            } else {
                val ok = runCatching { forceImportSave(RomFiles.privateSave(context, key), read(context, r.uri), keep) }
                    .onFailure { Log.w(TAG, "import of ${r.fileName} failed: $it") }
                    .getOrDefault(false)
                if (ok) imported++ else failed++
            }
        }
        return SaveImportResult(imported, skipped, failed, refused)
    }
}
