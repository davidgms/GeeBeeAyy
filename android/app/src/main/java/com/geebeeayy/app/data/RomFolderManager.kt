package com.geebeeayy.app.data

import android.content.Context
import java.io.File

class RomFolderManager(private val context: Context) {
    companion object {
        private const val TAG = "GeeBeeAyy/RomFolders"
    }

    private val prefs = context.getSharedPreferences("rom_folders", Context.MODE_PRIVATE)

    fun getFolderPaths(): List<String> {
        return prefs.getStringSet("folders", emptySet())?.toList() ?: emptyList()
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
        return roms.sortedBy { it.name.lowercase() }
    }

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
                    val sizeStr = formatSize(file.length())
                    val displayName = name.substringBeforeLast(".")
                        .replace("_", " ")
                        .replace("-", " ")
                    roms.add(
                        RomEntry(
                            name = displayName,
                            fileName = name,
                            size = sizeStr,
                            filePath = file.absolutePath,
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
