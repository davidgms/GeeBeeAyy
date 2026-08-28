package com.geebeeayy.app.data

import android.content.Context

/** How the 240x160 frame buffer is mapped onto the on-screen area. */
enum class ScaleMode {
    /** Largest size preserving the 3:2 aspect ratio; may land on a fractional pixel scale. */
    FIT,

    /** Largest whole-number multiple that fits, preserving aspect ratio. Falls back to [FIT]
     *  when the available area is smaller than the native 240x160 resolution. */
    INTEGER,

    /** Fills the whole available area, ignoring aspect ratio. */
    STRETCH,
}

/**
 * Display preferences, persisted across launches.
 *
 * Plain SharedPreferences, matching [RomFolderManager]'s pattern - this app
 * has no DataStore dependency and these are a handful of small values, not a
 * case for adding one.
 */
class DisplaySettings(context: Context) {
    private val prefs = context.getSharedPreferences("display_settings", Context.MODE_PRIVATE)

    fun getScaleMode(): ScaleMode =
        prefs.getString(KEY_SCALE_MODE, null)
            ?.let { saved -> runCatching { ScaleMode.valueOf(saved) }.getOrNull() }
            ?: ScaleMode.INTEGER

    fun setScaleMode(mode: ScaleMode) {
        prefs.edit().putString(KEY_SCALE_MODE, mode.name).apply()
    }

    /**
     * Whether the app locks itself to portrait via [android.app.Activity.setRequestedOrientation].
     * Defaults to true: the manifest used to hard-lock portrait unconditionally, so this keeps
     * that behaviour as the default after the lock moves into user-togglable settings, rather
     * than surprising an existing install with landscape it never asked for.
     */
    fun getForcePortrait(): Boolean = prefs.getBoolean(KEY_FORCE_PORTRAIT, true)

    fun setForcePortrait(forced: Boolean) {
        prefs.edit().putBoolean(KEY_FORCE_PORTRAIT, forced).apply()
    }

    private companion object {
        const val KEY_SCALE_MODE = "scale_mode"
        const val KEY_FORCE_PORTRAIT = "force_portrait"
    }
}
