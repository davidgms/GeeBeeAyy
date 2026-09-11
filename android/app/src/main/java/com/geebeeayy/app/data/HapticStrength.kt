package com.geebeeayy.app.data

/**
 * How hard the phone answers a press of an on-screen button.
 *
 * A touch button gives nothing back on its own, which is the largest single
 * difference between playing on a screen and playing on a pad: a thumb has no
 * way to know a press landed except by watching the game for it.
 *
 * Amplitudes rather than one canned effect, because a control that fires many
 * times a second has to be quieter than a notification. `VibrationEffect`
 * takes 1-255; the top step here is deliberately well under full power.
 */
enum class HapticStrength(val label: String, val amplitude: Int, val millis: Long) {
    OFF("Off", 0, 0),
    LIGHT("Light", 40, 8),
    MEDIUM("Medium", 90, 12),
    STRONG("Strong", 160, 16),
}
