package com.geebeeayy.app.data

/**
 * How large the letters on the on-screen buttons are drawn.
 *
 * Three fixed steps rather than a slider. The labels are one to six
 * characters inside a fixed circle or pill, so there is a narrow band where
 * they are legible and still fit - a free multiplier mostly offers ways to
 * get it wrong.
 *
 * A multiplier and not a size in sp, because the labels do not start equal:
 * A and B are 20sp inside a big circle, START and SELECT are 13sp inside a
 * narrow pill, and a single sp value for both would break one of them.
 */
enum class ControlFontSize(val label: String, val scale: Float) {
    SMALL("Small", 0.85f),
    MEDIUM("Medium", 1.0f),
    LARGE("Large", 1.25f),
}
