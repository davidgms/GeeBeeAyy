package com.geebeeayy.app.data

/** How a custom button drives the real keys it was built from. */
enum class CustomButtonMode {
    /** Every key pressed together, held while the finger is down and
     *  released together on release - one button standing in for several
     *  pressed at once (an L+R+Start+Select soft reset, say). */
    COMBO,

    /** Each key tapped in turn, once each, a beat apart - fired once per tap,
     *  not held. A key repeated in the list taps it that many times, which is
     *  what makes "three A presses" expressible as one button. */
    SEQUENCE,

    /** Tap to start holding every key down continuously; tap again to let
     *  go - stands in for a finger that would otherwise have to stay planted
     *  on L for a whole section. */
    TOGGLE_HOLD,
}

/**
 * A player-defined button built from the GBA's real keys
 * ([com.geebeeayy.app.engine.GbaEngine] `KEY_*` constants).
 *
 * Lives inside one [ControlLayout] - different layouts can carry entirely
 * different custom buttons, the same as they can position the real ones
 * differently.
 */
data class CustomButton(
    val id: String,
    val name: String,
    val mode: CustomButtonMode,
    val keys: List<Int>,
)
