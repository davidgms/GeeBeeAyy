package com.geebeeayy.app.data

data class RomEntry(
    val name: String,
    val fileName: String,
    val size: String,
    val sizeBytes: Long = 0,
    val dateModifiedMillis: Long = 0,
    val filePath: String,
    val lastPlayed: String? = null,
    val isFavorite: Boolean = false,
)
