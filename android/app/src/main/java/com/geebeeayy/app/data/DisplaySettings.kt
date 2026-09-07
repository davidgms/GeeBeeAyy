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

    /**
     * Whether the core averages each frame with the one before it.
     *
     * Defaults to on, because it is what the hardware did: the GBA's LCD was
     * slow enough to smear two frames together, and games counted on it to
     * fake transparency. Off shows exactly what the PPU drew, flicker and all.
     */
    fun getInterframeBlend(): Boolean = prefs.getBoolean(KEY_INTERFRAME_BLEND, true)

    fun setInterframeBlend(on: Boolean) {
        prefs.edit().putBoolean(KEY_INTERFRAME_BLEND, on).apply()
    }

    fun getScreenFilter(): ScreenFilter =
        prefs.getString(KEY_SCREEN_FILTER, null)
            ?.let { saved -> runCatching { ScreenFilter.valueOf(saved) }.getOrNull() }
            ?: ScreenFilter.NONE

    fun setScreenFilter(filter: ScreenFilter) {
        prefs.edit().putString(KEY_SCREEN_FILTER, filter.name).apply()
    }

    /**
     * Multiplier on the on-screen control sizes, [MIN_CONTROL_SCALE] to 1.0.
     *
     * Shrink only, because there is nowhere to grow: at 1.0 the button row
     * already spans nearly the full width, so anything above it pushes L, R
     * and the outer D-pad keys off the screen. Below 1.0 the buttons drop
     * under the 48.dp minimum touch target Material and WCAG 2.1 SC 2.5.8 ask
     * for, which is why the default stays at 1.0 and the floor is 0.7 rather
     * than something smaller.
     */
    fun getControlScale(): Float =
        prefs.getFloat(KEY_CONTROL_SCALE, 1.0f).coerceIn(MIN_CONTROL_SCALE, 1.0f)

    fun setControlScale(scale: Float) {
        prefs.edit().putFloat(KEY_CONTROL_SCALE, scale.coerceIn(MIN_CONTROL_SCALE, 1.0f)).apply()
    }

    /** Opacity of the on-screen controls, 0.3 to 1.0. */
    fun getControlOpacity(): Float =
        prefs.getFloat(KEY_CONTROL_OPACITY, 1.0f).coerceIn(MIN_CONTROL_OPACITY, 1.0f)

    fun setControlOpacity(opacity: Float) {
        prefs.edit()
            .putFloat(KEY_CONTROL_OPACITY, opacity.coerceIn(MIN_CONTROL_OPACITY, 1.0f))
            .apply()
    }

    companion object {
        /** 0.7 puts a 48.dp button at 34.dp, which is about as small as it
         *  can get and still be hit reliably. */
        const val MIN_CONTROL_SCALE = 0.7f

        /**
         * Below this the controls stop being findable by touch, which matters
         * more than the extra visibility of the game behind them.
         */
        const val MIN_CONTROL_OPACITY = 0.3f

        private const val KEY_SCALE_MODE = "scale_mode"
        private const val KEY_FORCE_PORTRAIT = "force_portrait"
        private const val KEY_CONTROL_SCALE = "control_scale"
        private const val KEY_CONTROL_OPACITY = "control_opacity"
        private const val KEY_SCREEN_FILTER = "screen_filter"
        private const val KEY_INTERFRAME_BLEND = "interframe_blend"
    }
}
