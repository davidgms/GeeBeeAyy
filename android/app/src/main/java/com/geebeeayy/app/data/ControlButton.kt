package com.geebeeayy.app.data

/**
 * Every button of the touch overlay a player can drag independently.
 *
 * Used to be four coarse groups (D-Pad, A/B, L/R, Start/Select) rather than
 * one entry per button, on the theory that moving individual buttons would
 * let a layout end up with A and B overlapping or a D-pad key off-screen.
 * Players wanted finer control than that trade-off allowed, so this is now
 * one entry per button - nothing stops two buttons overlapping if that is
 * where a player drags them.
 *
 * The D-pad is the exception, and deliberately so: it is one entry, because
 * on screen it is one cross with one touch area rather than four buttons.
 * Splitting it back into four would take diagonals away again.
 */
enum class ControlButton {
    DPAD,
    BUTTON_A,
    BUTTON_B,
    SHOULDER_L,
    SHOULDER_R,
    START,
    SELECT,
    ;

    val label: String
        get() = when (this) {
            DPAD -> "D-Pad"
            BUTTON_A -> "A"
            BUTTON_B -> "B"
            SHOULDER_L -> "L"
            SHOULDER_R -> "R"
            START -> "Start"
            SELECT -> "Select"
        }
}
