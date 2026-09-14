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

    /**
     * How many times real time fast forward runs, or 0 for unlimited.
     *
     * The ratio is a promise the audio clock enforces: the loop runs this
     * many emulated frames per real frame period, so 2 really is 2x. A
     * device that cannot keep up degrades to whatever it can do, with the
     * audio underrunning, rather than quietly ignoring the number.
     *
     * Unlimited keeps the old behaviour - no audio, no throttle, whatever the
     * CPU gives - which is what you want for seeking through a long cutscene
     * and is why mGBA, RetroArch and BizHawk all keep an unbounded mode
     * beside the ratio.
     *
     * 2x is the default, matching NanoBoyAdvance and Azahar, and sits inside
     * the headroom a phone actually has.
     */
    fun getFastForwardRatio(): Int =
        prefs.getInt(KEY_FAST_FORWARD_RATIO, 2).let { saved ->
            if (saved == 0 || saved in 2..MAX_FAST_FORWARD_RATIO) saved else 2
        }

    /**
     * Whether fast forward plays silence.
     *
     * The samples are still written, because the write is the frame clock.
     * Only their contents are zeroed. Worth having above 4x: the device
     * cannot produce samples fast enough to keep the track fed, and an
     * underrunning track buzzes.
     */
    fun getMuteOnFastForward(): Boolean = prefs.getBoolean(KEY_MUTE_FAST_FORWARD, false)

    fun setMuteOnFastForward(mute: Boolean) {
        prefs.edit().putBoolean(KEY_MUTE_FAST_FORWARD, mute).apply()
    }

    fun setFastForwardRatio(ratio: Int) {
        prefs.edit().putInt(KEY_FAST_FORWARD_RATIO, ratio).apply()
    }

    /**
     * Whether the emulator's sound plays.
     *
     * Muting does not stop the audio being written - that write is the frame
     * clock, and skipping it would let the loop free-run. The samples are
     * zeroed instead.
     */
    fun getSoundEnabled(): Boolean = prefs.getBoolean(KEY_SOUND_ENABLED, true)

    fun setSoundEnabled(enabled: Boolean) {
        prefs.edit().putBoolean(KEY_SOUND_ENABLED, enabled).apply()
    }

    /** How large the letters on the on-screen buttons are drawn. */
    fun getControlFontSize(): ControlFontSize =
        prefs.getString(KEY_CONTROL_FONT, null)
            ?.let { saved -> runCatching { ControlFontSize.valueOf(saved) }.getOrNull() }
            ?: ControlFontSize.MEDIUM

    fun setControlFontSize(size: ControlFontSize) {
        prefs.edit().putString(KEY_CONTROL_FONT, size.name).apply()
    }

    /** How hard the phone answers a press of an on-screen button. */
    fun getHapticStrength(): HapticStrength =
        prefs.getString(KEY_HAPTICS, null)
            ?.let { saved -> runCatching { HapticStrength.valueOf(saved) }.getOrNull() }
            ?: HapticStrength.LIGHT

    fun setHapticStrength(strength: HapticStrength) {
        prefs.edit().putString(KEY_HAPTICS, strength.name).apply()
    }

    /**
     * Half the width, in degrees, of each of the D-pad's four straight
     * directions. What is left between them is a diagonal.
     *
     * 45 leaves no diagonals at all - the four sectors meet. 30, the default,
     * gives a 60-degree straight and a 30-degree diagonal. Down at 15 the
     * diagonals are twice as wide as the straights.
     *
     * There is no right answer here, which is why it is a setting and not a
     * constant: it depends on the size of the thumb, where the pad sits, and
     * whether the game is a platformer or a menu.
     */
    fun getDpadCardinalHalfDegrees(): Float =
        prefs.getFloat(KEY_DPAD_CARDINAL, 30f)
            .coerceIn(MIN_DPAD_CARDINAL_HALF, MAX_DPAD_CARDINAL_HALF)

    fun setDpadCardinalHalfDegrees(degrees: Float) {
        prefs.edit()
            .putFloat(
                KEY_DPAD_CARDINAL,
                degrees.coerceIn(MIN_DPAD_CARDINAL_HALF, MAX_DPAD_CARDINAL_HALF),
            )
            .apply()
    }

    /** Which colour scheme the on-screen controls draw themselves in. */
    fun getControlTint(): ControlTint =
        prefs.getString(KEY_CONTROL_TINT, null)
            ?.let { saved -> runCatching { ControlTint.valueOf(saved) }.getOrNull() }
            ?: ControlTint.NIGHT

    fun setControlTint(tint: ControlTint) {
        prefs.edit().putString(KEY_CONTROL_TINT, tint.name).apply()
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
    /**
     * Opacity of the on-screen controls.
     *
     * Defaults to 0.65 rather than opaque: the controls now sit *over* the
     * picture instead of under it, so at 1.0 the bottom third of the game
     * would be behind solid buttons. An existing install keeps whatever it
     * had.
     */
    fun getControlOpacity(): Float =
        prefs.getFloat(KEY_CONTROL_OPACITY, 0.65f).coerceIn(MIN_CONTROL_OPACITY, 1.0f)

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
        private const val KEY_FAST_FORWARD_RATIO = "fast_forward_ratio"
        private const val KEY_MUTE_FAST_FORWARD = "mute_fast_forward"
        private const val KEY_CONTROL_TINT = "control_tint"
        private const val KEY_CONTROL_FONT = "control_font_size"
        private const val KEY_SOUND_ENABLED = "sound_enabled"
        private const val KEY_HAPTICS = "haptic_strength"
        private const val KEY_DPAD_CARDINAL = "dpad_cardinal_half_degrees"

        /** At 45 the four straight sectors meet and there is no diagonal left. */
        const val MAX_DPAD_CARDINAL_HALF = 45f

        /** At 15 a diagonal is twice as wide as a straight. */
        const val MIN_DPAD_CARDINAL_HALF = 15f

        /**
         * Highest ratio offered.
         *
         * Measured on a Mi 10T Pro: a game with headroom hits 2x, 3x and 4x
         * exactly and peaks around 5.5x, after which a higher ratio is
         * *slower* as well as choppier - the decimation pass and the audio
         * read both grow with it, and the picture is published once per
         * batch. 8 is past the peak already and is there for the games that
         * are CPU-bound anyway, where the only thing left to trade is
         * smoothness.
         */
        const val MAX_FAST_FORWARD_RATIO = 8
    }
}
