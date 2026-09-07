package com.geebeeayy.app.data

import android.content.Context

/** When each ROM was last launched, keyed by its absolute file path. */
class LastPlayed(context: Context) {
    private val prefs = context.getSharedPreferences("last_played", Context.MODE_PRIVATE)

    fun record(filePath: String) {
        prefs.edit().putLong(filePath, System.currentTimeMillis()).apply()
    }

    fun get(filePath: String): Long? {
        val value = prefs.getLong(filePath, -1L)
        return if (value < 0) null else value
    }
}
