package com.geebeeayy.app.data

import android.content.Context
import java.io.File

class RomFolderManager(private val context: Context) {
    companion object {
        private const val TAG = "GeeBeeAyy/RomFolders"
    }

    private val prefs = context.getSharedPreferences("rom_folders", Context.MODE_PRIVATE)
    private val lastPlayed = LastPlayed(context)
    private val favorites = Favorites(context)

    /**
     * The configured folders, sorted.
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

    fun removeFolder(path: String) {
        val current = getFolderPaths().toMutableSet()
        current.remove(path)
        prefs.edit().putStringSet("folders", current).apply()
    }

    fun hasFolders(): Boolean = getFolderPaths().isNotEmpty()

    fun scanAllFolders(): List<RomEntry> {
        val folders = getFolderPaths()
        val roms = mutableListOf<RomEntry>()
        for (folderPath in folders) {
            val folder = File(folderPath)
            if (folder.isDirectory) {
                scanDirectory(folder, roms)
            }
        }
        roms += missingEntries(roms.map { it.filePath }.toSet())
        return roms.sortedBy { it.name.lowercase() }
    }

    /**
     * Rows for games the player has played whose file the scan did not find.
     *
     * Only played ones: a path is remembered because a game was launched from
     * it, so this cannot invent a row for a file that was merely glanced at.
     * The alternative - dropping them - makes a game disappear silently, which
     * reads as the app having lost it rather than as a card being out.
     */
    private fun missingEntries(found: Set<String>): List<RomEntry> =
        lastPlayed.paths()
            .filterNot { it in found }
            .filterNot { File(it).isFile }
            .map { path ->
                val file = File(path)
                RomEntry(
                    name = displayName(file.name),
                    fileName = file.name,
                    size = "-",
                    filePath = path,
                    lastPlayedMillis = lastPlayed.get(path),
                    isFavorite = favorites.isFavorite(path),
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
                if (name.endsWith(".gba", ignoreCase = true) ||
                    name.endsWith(".agb", ignoreCase = true) ||
                    name.endsWith(".bin", ignoreCase = true)
                ) {
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
