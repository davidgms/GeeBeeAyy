package com.geebeeayy.app.data

data class RomEntry(
    val name: String,
    val fileName: String,
    val size: String,
    val sizeBytes: Long = 0,
    val dateModifiedMillis: Long = 0,
    val filePath: String,
    val lastPlayedMillis: Long? = null,
    val isFavorite: Boolean = false,
    /**
     * False for a game the player has played before whose file is no longer
     * where it was. Such a row is kept and marked rather than dropped: a game
     * vanishing from the list on its own looks like the app lost it, when the
     * usual cause is a card out of the phone or a folder renamed.
     */
    val exists: Boolean = true,
)
