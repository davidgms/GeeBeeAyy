package com.geebeeayy.app.data

data class RomEntry(
    val name: String,
    val fileName: String,
    val size: String,
    val filePath: String,
    val lastPlayed: String? = null,
    val isFavorite: Boolean = false,
)
