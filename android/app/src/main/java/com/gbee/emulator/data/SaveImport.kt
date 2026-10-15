package com.gbee.emulator.data

import android.content.Context
import android.net.Uri
import android.provider.DocumentsContract
import android.util.Log
import java.io.File

/**
 * Manual import of battery saves from another emulator's folder: find the
 * files, pair each with a library game, check it fits that game's save chip,
 * and write the ones the player confirms. The pure parts are pinned by JVM
 * tests; the rest only glues them to the document provider.
 */
private const val TAG = "GBee/SaveImport"

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
    return seen.filter {
        !it.isDir && (isSaveName(it.name) && (it.size ?: 1L) in 1..MAX_SAVE_BYTES ||
            parseStateName(it.name) != null && (it.size ?: 1L) in 1..STATE_MAX_BYTES)
    }
}

/** What No$GBA's compressed save format starts with. */
private val NOCASH_MAGIC = "NocashGbaBackupMediaSavDataFile".toByteArray(Charsets.US_ASCII)

/** True for a No$GBA compressed save, which is not decoded: the player re-saves it as Raw. */
internal fun isNocashSave(bytes: ByteArray): Boolean =
    bytes.size >= NOCASH_MAGIC.size && NOCASH_MAGIC.indices.all { bytes[it] == NOCASH_MAGIC[it] }

/** The chip a game uses, as the player reads it; null for a size that is none of them (0 is "no chip", handled apart). */
internal fun saveChipLabel(size: Int): String? = when (size) {
    32768 -> "32 KB SRAM"
    65536 -> "64 KB Flash"
    131072 -> "128 KB Flash"
    8192 -> "EEPROM (512 B or 8 KB)"
    else -> null
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
    /** The save chip size this game has, when the file did not fit it (see [saveChipLabel]); null unknown. */
    val chipSize: Int? = null,
    /** The file is a No$GBA compressed save. */
    val nocash: Boolean = false,
    val existingSize: Long = 0L,
    val existingModified: Long = 0L,
    val selected: Boolean = false,
    /** Null for a battery save; otherwise the save-state slot this file is for. */
    val slot: Int? = null,
    /** Document uri of the state's `.gbra` sidecar, when the folder has one. */
    val raUri: String? = null,
) {
    val importable get() = keep != null
}

class SaveImportResult(val imported: Int, val skipped: Int, val failed: Int, val refused: List<String>)

object SaveImport {

    /** The saves under a picked [tree], or null if it cannot be read. Blocking. */
    fun scan(
        context: Context,
        tree: Uri,
        roms: List<RomEntry>,
        saveFit: (ByteArray, Int) -> Int?,
        saveSize: (ByteArray) -> Int?,
    ): List<SaveImportRow>? {
        val found = findSaves(DocumentsContract.getTreeDocumentId(tree)) {
            RomFolderManager.listChildren(context, tree, it)
        } ?: return null
        fun uriOf(f: DocChild) = DocumentsContract.buildDocumentUriUsingTree(tree, f.docId).toString()
        val sidecars = found.filter { parseStateName(it.name)?.ra == true }.associateBy { it.name.lowercase() }
        return found.filter { parseStateName(it.name)?.ra != true }.mapNotNull { f ->
            val st = parseStateName(f.name)
            // Only our own format: a state without the core's magic is not listed at all.
            if (st != null && !runCatching { RomBytes.open(context, uriOf(f)).use { it.readNBytes(STATE_HEADER_BYTES) } }.map(::isGbeeState).getOrDefault(false)) {
                return@mapNotNull null
            }
            val ra = st?.let { sidecars[f.name.lowercase().removeSuffix("gbstate") + "gbra"] }?.let(::uriOf)
            val rom = matchRom(st?.let { it.base + ".gba" } ?: f.name, roms)
            row(context, f.name, uriOf(f), f.size ?: -1L, f.modified, rom, saveFit, saveSize, st?.slot, ra)
        }
    }

    /** [row] re-evaluated against [rom], after the player picked it. Blocking. */
    fun pick(
        context: Context,
        row: SaveImportRow,
        rom: RomEntry,
        saveFit: (ByteArray, Int) -> Int?,
        saveSize: (ByteArray) -> Int?,
    ) = row(context, row.fileName, row.uri, row.size, row.modified, rom, saveFit, saveSize, row.slot, row.raUri)

    private fun row(
        context: Context,
        name: String,
        uri: String,
        size: Long,
        modified: Long,
        rom: RomEntry?,
        saveFit: (ByteArray, Int) -> Int?,
        saveSize: (ByteArray) -> Int?,
        slot: Int?,
        raUri: String?,
    ): SaveImportRow {
        val bare = SaveImportRow(name, uri, size, modified, rom, slot = slot, raUri = raUri)
        if (rom == null) return bare
        // One read of the game per row; the key and the fit check both need it.
        val cart = runCatching { RomBytes.read(context, rom.filePath) }.getOrNull() ?: return bare
        val len = if (size >= 0) size else runCatching { read(context, uri, if (slot != null) STATE_MAX_BYTES else MAX_SAVE_BYTES).size.toLong() }.getOrNull() ?: return bare
        val key = RomHeader.stateKey(cart)
        if (slot != null) {
            val existing = stateTarget(context, key, slot).takeIf { it.isFile && it.length() > 0 }
            // keep is only "fits" for a state: it is written whole.
            return bare.copy(
                size = len, key = key, keep = len.toInt(),
                existingSize = existing?.length() ?: 0L, existingModified = existing?.lastModified() ?: 0L,
                selected = existing == null,
            )
        }
        // A No$GBA container is refused whatever its size: one that happens to match a chip would import as garbage.
        val nocash = runCatching { isNocashSave(read(context, uri)) }.getOrDefault(false)
        val keep = if (nocash) null else saveFit(cart, len.toInt())
        val chip = if (keep == null && !nocash) saveSize(cart) else null
        val existing = RomFiles.privateSave(context, key).takeIf { it.isFile && it.length() > 0 }
        return bare.copy(
            size = len,
            key = key,
            keep = keep,
            chipSize = chip,
            nocash = nocash,
            existingSize = existing?.length() ?: 0L,
            existingModified = existing?.lastModified() ?: 0L,
            // A game that already has a save is "keep mine" until asked otherwise.
            selected = keep != null && existing == null,
        )
    }

    private fun read(context: Context, uri: String, cap: Long = MAX_SAVE_BYTES): ByteArray =
        RomBytes.open(context, uri).use { it.readCapped(cap) }

    private fun stateTarget(context: Context, key: String, slot: Int, ra: Boolean = false) =
        File(File(context.filesDir, "states"), "${key}_slot$slot" + if (ra) ".ra" else ".state")

    /** Write a state and its sidecar; an old sidecar with no new one is moved aside so it never pairs with the new state. */
    private fun importState(context: Context, r: SaveImportRow, key: String, slot: Int): Boolean {
        val bytes = read(context, r.uri, STATE_MAX_BYTES)
        if (!isGbeeState(bytes) || !importStateFile(stateTarget(context, key, slot), bytes)) return false
        val ra = stateTarget(context, key, slot, ra = true)
        val new = r.raUri?.let { runCatching { read(context, it, STATE_MAX_BYTES) }.getOrNull() }
        if (new != null) importStateFile(ra, new) else if (ra.isFile) ra.renameTo(File(ra.path + ".bak1"))
        return true
    }

    /**
     * Write the selected rows. [openGame] is the library id of the game now
     * running, if any: writing its save would be flushed over by the emulator
     * (or lost on its next load), so it is refused. The player closes the game first. Blocking.
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
            } else if (!done.add("$key#${r.slot}")) {
                // Two files for one game: the first selected one wins.
                skipped++
            } else {
                val slot = r.slot
                val ok = runCatching {
                    if (slot != null) importState(context, r, key, slot)
                    else forceImportSave(RomFiles.privateSave(context, key), read(context, r.uri), keep)
                }
                    .onFailure { Log.w(TAG, "import of ${r.fileName} failed: $it") }
                    .getOrDefault(false)
                if (ok) imported++ else failed++
            }
        }
        return SaveImportResult(imported, skipped, failed, refused)
    }
}
