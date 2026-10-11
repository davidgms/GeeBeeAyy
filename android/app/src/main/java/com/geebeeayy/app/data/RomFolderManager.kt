package com.geebeeayy.app.data

import android.content.Context
import android.content.Intent
import android.net.Uri
import android.provider.DocumentsContract
import android.util.Log
import java.io.File
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow

/**
 * The configured ROM folders. Each is either a document-tree Uri string
 * (what the picker returns, and what persists across reboots) or, from before
 * that, an absolute path. Both scan; a path under shared storage is shown in
 * Settings as needing a re-pick, and [addTree] swaps it for the tree.
 */
class RomFolderManager(private val context: Context) {
    companion object {
        private const val TAG = "GeeBeeAyy/RomFolders"

        private val PROJECTION = arrayOf(
            DocumentsContract.Document.COLUMN_DOCUMENT_ID,
            DocumentsContract.Document.COLUMN_DISPLAY_NAME,
            DocumentsContract.Document.COLUMN_MIME_TYPE,
            DocumentsContract.Document.COLUMN_SIZE,
            DocumentsContract.Document.COLUMN_LAST_MODIFIED,
        )

        private const val GRANT = Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_WRITE_URI_PERMISSION

        private val unavailableFlow = MutableStateFlow<Set<String>>(emptySet())

        /** Tree folders whose last scan could not read the root: revoked, or the volume is gone. */
        val unavailable: StateFlow<Set<String>> get() = unavailableFlow

        /** A folder as a player reads it: the path, or the volume and path inside a tree. */
        fun folderLabel(folder: String): String =
            if (RomBytes.isPath(folder)) {
                folder
            } else {
                docIdLabel(DocumentsContract.getTreeDocumentId(Uri.parse(folder)))
            }

        /**
         * Where the picker should start to re-pick [folder]: the folder itself
         * when it is a path on a volume the provider serves, a tree's own Uri
         * when it went unavailable, else null.
         */
        fun repickStart(folder: String): Uri? = when {
            !RomBytes.isPath(folder) -> Uri.parse(folder)
            else -> legacyDocId(folder)?.let {
                DocumentsContract.buildDocumentUri(EXTERNAL_STORAGE_AUTHORITY, it)
            }
        }

        /** One listing of a directory in a tree; null when it cannot be read. */
        internal fun listChildren(context: Context, tree: Uri, docId: String): List<DocChild>? = runCatching {
            val uri = DocumentsContract.buildChildDocumentsUriUsingTree(tree, docId)
            context.contentResolver.query(uri, PROJECTION, null, null, null)?.use { c ->
                buildList {
                    while (c.moveToNext()) {
                        add(
                            DocChild(
                                docId = c.getString(0),
                                name = c.getString(1).orEmpty(),
                                mime = c.getString(2).orEmpty(),
                                size = if (c.isNull(3)) null else c.getLong(3),
                                modified = if (c.isNull(4)) 0L else c.getLong(4),
                            ),
                        )
                    }
                }
            }
        }.onFailure { Log.w(TAG, "cannot list $docId: $it") }.getOrNull()
    }

    private val prefs = context.getSharedPreferences("rom_folders", Context.MODE_PRIVATE)
    private val lastPlayed = LastPlayed(context)
    private val favorites = Favorites(context)

    /**
     * The configured folders (tree Uris and legacy paths), sorted.
     *
     * The backing store is a `Set`, whose iteration order is arbitrary, so
     * anything that means "the first folder" - the homebrew downloader's
     * default destination, for one - got a different answer between runs.
     */
    fun getFolderPaths(): List<String> {
        return prefs.getStringSet("folders", emptySet())?.sorted() ?: emptyList()
    }

    fun addFolder(path: String) {
        val current = getFolderPaths().toMutableSet()
        current.add(path)
        prefs.edit().putStringSet("folders", current).apply()
    }

    /** Removes [folder], releasing the grant when it is a tree. */
    fun removeFolder(folder: String) {
        val current = getFolderPaths().toMutableSet()
        current.remove(folder)
        prefs.edit().putStringSet("folders", current).apply()
        if (!RomBytes.isPath(folder)) {
            runCatching { context.contentResolver.releasePersistableUriPermission(Uri.parse(folder), GRANT) }
        }
    }

    /**
     * Add a picked tree. Keeps the grant across reboots, and when the tree is
     * exactly a legacy path folder re-picked, replaces that folder and moves
     * its games' last-played times and stars onto the new ids.
     *
     * @return false if the grant could not be kept, in which case nothing changed.
     */
    fun addTree(tree: Uri): Boolean {
        try {
            context.contentResolver.takePersistableUriPermission(tree, GRANT)
        } catch (e: SecurityException) {
            Log.w(TAG, "grant not kept for $tree", e)
            return false
        }
        val rootId = DocumentsContract.getTreeDocumentId(tree)
        val current = getFolderPaths().toMutableSet()
        for (path in current.filter { RomBytes.isPath(it) && legacyDocId(it) == rootId }) {
            val moved = rekeyPaths(lastPlayed.paths() + favorites.paths(), path) { docId ->
                DocumentsContract.buildDocumentUriUsingTree(tree, docId).toString()
            }
            lastPlayed.rekey(moved)
            favorites.rekey(moved)
            current.remove(path)
        }
        current.add(tree.toString())
        prefs.edit().putStringSet("folders", current).apply()
        return true
    }

    fun hasFolders(): Boolean = getFolderPaths().isNotEmpty()

    /** Blocking: call it off the main thread. */
    fun scanAllFolders(): List<RomEntry> {
        val folders = getFolderPaths()
        val roms = mutableListOf<RomEntry>()
        val siblings = HashMap<String, Siblings>()
        val down = mutableSetOf<String>()
        for (folderPath in folders) {
            if (RomBytes.isPath(folderPath)) {
                val folder = File(folderPath)
                if (folder.isDirectory) {
                    scanDirectory(folder, roms)
                }
            } else if (!scanTreeFolder(Uri.parse(folderPath), roms, siblings)) {
                down += folderPath
            }
        }
        unavailableFlow.value = down
        RomSiblings.replace(siblings)
        roms += missingEntries(roms.map { it.filePath }.toSet())
        return roms.sortedBy { it.name.lowercase() }
    }

    /** @return false if the tree's root could not be read. */
    private fun scanTreeFolder(tree: Uri, roms: MutableList<RomEntry>, siblings: MutableMap<String, Siblings>): Boolean {
        val found = scanTree(DocumentsContract.getTreeDocumentId(tree)) { listChildren(context, tree, it) }
            ?: return false
        fun uriOf(docId: String) = DocumentsContract.buildDocumentUriUsingTree(tree, docId).toString()
        for (r in found) {
            val id = uriOf(r.rom.docId)
            val size = r.rom.size ?: 0L
            roms.add(
                RomEntry(
                    name = displayName(r.rom.name),
                    fileName = r.rom.name,
                    size = formatSize(size),
                    sizeBytes = size,
                    dateModifiedMillis = r.rom.modified,
                    filePath = id,
                    lastPlayedMillis = lastPlayed.get(id),
                    isFavorite = favorites.isFavorite(id),
                    folderLabel = r.folderLabel,
                ),
            )
            siblings[id] = Siblings(
                name = r.rom.name,
                saveUri = r.save?.let { uriOf(it.docId) },
                saveSize = r.save?.size,
                saveModified = r.save?.modified ?: 0L,
                coverUri = r.cover?.let { uriOf(it.docId) },
            )
        }
        return true
    }

    /**
     * Rows for games the player has played whose file the scan did not find.
     *
     * Only played ones: a path is remembered because a game was launched from
     * it, so this cannot invent a row for a file that was merely glanced at.
     * The alternative - dropping them - makes a game disappear silently, which
     * reads as the app having lost it rather than as a card being out.
     *
     * A document id is missing when the scan did not list it: asking its
     * provider about each one would be a query per played game on every scan.
     */
    private fun missingEntries(found: Set<String>): List<RomEntry> =
        lastPlayed.paths()
            .filterNot { it in found }
            .filterNot { RomBytes.isPath(it) && File(it).isFile }
            .map { id ->
                val name = if (RomBytes.isPath(id)) {
                    File(id).name
                } else {
                    runCatching { docIdName(DocumentsContract.getDocumentId(Uri.parse(id))) }.getOrDefault("")
                }
                RomEntry(
                    name = displayName(name),
                    fileName = name,
                    size = "-",
                    filePath = id,
                    lastPlayedMillis = lastPlayed.get(id),
                    isFavorite = favorites.isFavorite(id),
                    exists = false,
                )
            }

    private fun displayName(fileName: String): String =
        fileName.substringBeforeLast(".").replace("_", " ").replace("-", " ")

    private fun scanDirectory(dir: File, roms: MutableList<RomEntry>) {
        val files = dir.listFiles() ?: return
        for (file in files) {
            if (file.isDirectory) {
                scanDirectory(file, roms)
            } else if (file.isFile) {
                val name = file.name
                if (isRomName(name)) {
                    roms.add(
                        RomEntry(
                            name = displayName(name),
                            fileName = name,
                            size = formatSize(file.length()),
                            sizeBytes = file.length(),
                            dateModifiedMillis = file.lastModified(),
                            filePath = file.absolutePath,
                            lastPlayedMillis = lastPlayed.get(file.absolutePath),
                            isFavorite = favorites.isFavorite(file.absolutePath),
                        )
                    )
                }
            }
        }
    }

    private fun formatSize(bytes: Long): String {
        return when {
            bytes < 1024 -> "$bytes B"
            bytes < 1024 * 1024 -> "${bytes / 1024} KB"
            bytes < 1024 * 1024 * 1024 -> "${bytes / (1024 * 1024)} MB"
            else -> "${bytes / (1024 * 1024 * 1024)} GB"
        }
    }
}
