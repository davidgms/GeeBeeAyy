package com.gbee.emulator.data

import android.content.Context

/** When each ROM was last launched, keyed by its absolute file path. */
class LastPlayed(context: Context) {
    private val prefs = context.getSharedPreferences("last_played", Context.MODE_PRIVATE)

    fun record(filePath: String) {
        prefs.edit().putLong(filePath, System.currentTimeMillis()).apply()
    }

    /** Every path ever launched, including ones whose file has since gone. */
    fun paths(): Set<String> = prefs.all.keys

    fun forget(filePath: String) {
        prefs.edit().remove(filePath).apply()
    }

    fun get(filePath: String): Long? {
        val value = prefs.getLong(filePath, -1L)
        return if (value < 0) null else value
    }

    /** Move each entry to its new key (a folder re-picked as a tree), keeping the latest time. */
    fun rekey(map: Map<String, String>) {
        val all = prefs.all.mapNotNull { (k, v) -> (v as? Long)?.let { k to it } }.toMap()
        val moved = rekeyedTimes(all.filterKeys { it in map }, map)
        if (moved.isEmpty()) return
        val edit = prefs.edit()
        map.keys.forEach { edit.remove(it) }
        for ((key, time) in moved) edit.putLong(key, maxOf(time, all[key] ?: 0L))
        edit.apply()
    }
}
