package com.gbee.emulator.data

/**
 * The pure half of folder handling: path to document-id mapping and the
 * recursive tree scan, with no Android types so JVM tests pin both. Anything
 * that needs a `Uri` or a `ContentResolver` lives in [RomFolderManager].
 */

internal const val EXTERNAL_STORAGE_AUTHORITY = "com.android.externalstorage.documents"

/** `DocumentsContract.Document.MIME_TYPE_DIR`, restated so the scan stays JVM-testable. */
internal const val DIR_MIME = "vnd.android.document/directory"

/** Deepest folder the tree scan descends into; a provider loop cannot run away. */
internal const val MAX_TREE_DEPTH = 16

internal val ART_EXTENSIONS = listOf("png", "jpg", "jpeg", "webp")

/**
 * Zips too: a downloaded ROM arrives in one, and the cart is pulled out when
 * the game is opened rather than on the scan, which would read every archive
 * in the folder to list it.
 */
internal fun isRomName(name: String): Boolean =
    (RomBytes.ROM_EXTENSIONS + RomBytes.ZIP_EXTENSION).any { name.endsWith(".$it", ignoreCase = true) }

/**
 * The external-storage document id of a legacy path folder, or null when it
 * is not on a volume the provider serves. Exact, never guessed from names:
 * `/storage/emulated/0/<rel>` and `/sdcard/<rel>` are `primary:<rel>`, and
 * `/storage/<volume>/<rel>` is `<volume>:<rel>`.
 */
internal fun legacyDocId(path: String): String? {
    val p = path.trimEnd('/')
    return when {
        p == "/sdcard" || p.startsWith("/sdcard/") ->
            "primary:" + p.removePrefix("/sdcard").removePrefix("/")
        p == "/storage/emulated/0" || p.startsWith("/storage/emulated/0/") ->
            "primary:" + p.removePrefix("/storage/emulated/0").removePrefix("/")
        p.startsWith("/storage/") -> {
            val after = p.removePrefix("/storage/")
            val volume = after.substringBefore('/')
            if (volume.isEmpty() || volume == "emulated" || volume == "self") {
                null
            } else {
                "$volume:" + after.substringAfter('/', "")
            }
        }
        else -> null
    }
}

/** True if [id] is [root] itself or lies under it (the "/" boundary keeps `Roms2` out of `Roms`). */
internal fun docIdContains(root: String, id: String): Boolean {
    if (id == root) return true
    val prefix = if (root.endsWith(":")) root else "$root/"
    return id.startsWith(prefix)
}

/**
 * Old key to new key for every legacy path in [paths] strictly under the
 * picked tree's [rootDocId]. The new id is the path's own `<volume>:<rel>`,
 * so old and new are related by this mapping alone, never by file name -
 * and it covers every path key there is, whichever folders are configured.
 */
internal fun rekeyUnder(
    rootDocId: String,
    paths: Collection<String>,
    toId: (docId: String) -> String,
): Map<String, String> =
    paths.mapNotNull { old ->
        if (!RomBytes.isPath(old)) return@mapNotNull null
        val id = legacyDocId(old)?.takeIf { it != rootDocId && docIdContains(rootDocId, it) }
        id?.let { old to toId(it) }
    }.toMap()

/**
 * [times] after moving keys per [map]. Two old keys (the `/sdcard` and
 * `/storage/emulated/0` spellings) can land on one new key: the later time wins.
 */
internal fun rekeyedTimes(times: Map<String, Long>, map: Map<String, String>): Map<String, Long> {
    val out = HashMap<String, Long>()
    for ((key, time) in times) out.merge(map[key] ?: key, time, ::maxOf)
    return out
}

/** [keys] after moving per [map]; keys that land together are one. */
internal fun rekeyedSet(keys: Set<String>, map: Map<String, String>): Set<String> =
    keys.mapTo(HashSet()) { map[it] ?: it }

/** What a player recognises a folder by: the volume and the path inside it. */
internal fun docIdLabel(docId: String): String {
    val volume = docId.substringBefore(':')
    val rel = docId.substringAfter(':', "").trim('/')
    val root = if (volume == "primary") "Internal storage" else volume
    return if (rel.isEmpty()) root else "$root/$rel"
}

/** The last segment of a document id, for a row whose provider is not asked. */
internal fun docIdName(docId: String): String = docId.substringAfterLast('/').substringAfterLast(':')

/** One row of a directory listing. [size] is null when the provider does not say. */
internal class DocChild(
    val docId: String,
    val name: String,
    val mime: String,
    val size: Long?,
    val modified: Long,
) {
    val isDir get() = mime == DIR_MIME
}

/** A ROM found by [scanTree], with the siblings seen in the same directory listing. */
internal class TreeRom(
    val rom: DocChild,
    val folderLabel: String,
    /** The directory listing the ROM came from. */
    val parentDocId: String,
    val save: DocChild?,
    val cover: DocChild?,
)

/**
 * Walk a document tree from [rootDocId], one [children] call per directory.
 *
 * The `.sav` and the cover beside each ROM come out of the listing already
 * in hand (cover rules are [RomArtwork]'s: `<base>.<ext>` beside the ROM,
 * else `covers/<base>.<ext>`), so nothing later needs a query per file.
 * [children] returns null when a directory cannot be read: for the root that
 * is the whole answer (null, so the caller can say the folder is
 * unavailable); for a subdirectory it is skipped.
 */
internal fun scanTree(
    rootDocId: String,
    maxDepth: Int = MAX_TREE_DEPTH,
    children: (docId: String) -> List<DocChild>?,
): List<TreeRom>? {
    // The one listing read ahead of its turn: `covers/` is needed while its
    // parent is scanned, and again when the walk reaches it.
    val ahead = HashMap<String, List<DocChild>>()
    val out = ArrayList<TreeRom>()
    fun walk(docId: String, depth: Int, root: Boolean): Boolean {
        val kids = ahead.remove(docId) ?: children(docId) ?: return !root
        val files = kids.filterNot { it.isDir }.associateBy { it.name.lowercase() }
        val roms = kids.filter { !it.isDir && isRomName(it.name) }
        if (roms.isNotEmpty()) {
            val covers = kids.firstOrNull { it.isDir && it.name.equals("covers", ignoreCase = true) }
                ?.let { dir -> children(dir.docId)?.also { ahead[dir.docId] = it } }
                ?.filterNot { it.isDir }?.associateBy { it.name.lowercase() }
                ?: emptyMap()
            val label = docIdLabel(docId)
            for (rom in roms) {
                val base = rom.name.substringBeforeLast('.')
                val cover = ART_EXTENSIONS.firstNotNullOfOrNull { ext ->
                    files["$base.$ext".lowercase()] ?: covers["$base.$ext".lowercase()]
                }
                out += TreeRom(rom, label, docId, files["$base.sav".lowercase()], cover)
            }
        }
        if (depth < maxDepth) kids.filter { it.isDir }.forEach { walk(it.docId, depth + 1, false) }
        return true
    }
    return if (walk(rootDocId, 0, true)) out else null
}

/**
 * What the last scan saw beside each document ROM, keyed by its id string.
 * In memory only: a scan rebuilds it, and nothing here is worth persisting.
 * A missing key means "not scanned", which is not the same as "no `.sav`".
 */
internal class Siblings(
    val name: String,
    /** The tree and the directory the ROM sits in, to look its `.sav` up when the scan saw none. */
    val tree: String,
    val parentDocId: String,
    val saveUri: String?,
    val saveSize: Long?,
    val saveModified: Long,
    val coverUri: String?,
)

internal object RomSiblings {
    @Volatile
    private var byId: Map<String, Siblings> = emptyMap()

    fun replace(map: Map<String, Siblings>) {
        byId = map
    }

    fun get(id: String): Siblings? = byId[id]
}
