package com.geebeeayy.app.data

/**
 * The blocks of the touch overlay a player can move independently.
 *
 * Deliberately coarse. Moving individual buttons would let a layout end up
 * with A and B overlapping or a D-pad key off-screen, and the thing people
 * actually want is the D-pad further left or the face buttons further down.
 */
enum class ControlGroup {
    DPAD,
    ACTIONS,
    SHOULDERS,
    START_SELECT,
    ;

    val label: String
        get() = when (this) {
            DPAD -> "D-Pad"
            ACTIONS -> "A / B"
            SHOULDERS -> "L / R"
            START_SELECT -> "Start / Select"
        }
}
