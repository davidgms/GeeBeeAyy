package com.geebeeayy.app.data

import android.content.Context

/**
 * Which ROMs the player starred, keyed by absolute file path.
 *
 * The same key [LastPlayed] uses, and for the same reason: two carts with the
 * same display name must not share a star. A path that no longer exists keeps
 * its star, so a game on a card that is out of the phone comes back starred
 * rather than forgotten.
 */
class Favorites(context: Context) {
    private val prefs = context.getSharedPreferences("favorites", Context.MODE_PRIVATE)

    fun paths(): Set<String> = prefs.getStringSet(KEY, emptySet()) ?: emptySet()

    fun isFavorite(filePath: String): Boolean = filePath in paths()

    /** Returns the state after the toggle, so a caller can act on it. */
    fun toggle(filePath: String): Boolean {
        val current = paths().toMutableSet()
        val nowFavorite = current.add(filePath)
        if (!nowFavorite) current.remove(filePath)
        // A new Set instance, not the one handed out: SharedPreferences keeps
        // the reference it was given, and mutating that one in place makes the
        // next read return the change without ever writing it to disk.
        prefs.edit().putStringSet(KEY, current.toSet()).apply()
        return nowFavorite
    }

    fun forget(filePath: String) {
        val current = paths().toMutableSet()
        if (current.remove(filePath)) {
            prefs.edit().putStringSet(KEY, current.toSet()).apply()
        }
    }

    private companion object {
        const val KEY = "favorite_paths"
    }
}
