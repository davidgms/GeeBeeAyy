package com.geebeeayy.app.data

/**
 * Which way the phone has to be held for a layout to be offered.
 *
 * A portrait arrangement does not survive the turn: the picture moves, the
 * controls flank it instead of floating under it, and a custom button's
 * absolute position lands somewhere else entirely. Rather than guess a
 * translation, a layout says where it belongs and the other orientation gets
 * a layout of its own.
 */
enum class LayoutOrientation(val label: String) {
    PORTRAIT("Portrait"),
    LANDSCAPE("Landscape"),
    BOTH("Both");

    fun appliesTo(landscape: Boolean): Boolean = when (this) {
        PORTRAIT -> !landscape
        LANDSCAPE -> landscape
        BOTH -> true
    }

    companion object {
        /** What a layout saved before this setting existed is taken to be. */
        val DEFAULT = BOTH

        fun fromName(name: String?): LayoutOrientation =
            entries.firstOrNull { it.name == name } ?: DEFAULT
    }
}
