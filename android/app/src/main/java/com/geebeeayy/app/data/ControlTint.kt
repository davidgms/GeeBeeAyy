package com.geebeeayy.app.data

import androidx.compose.ui.graphics.Color
import com.geebeeayy.app.ui.theme.AmberResin
import com.geebeeayy.app.ui.theme.BurntRoot
import com.geebeeayy.app.ui.theme.GoldenSaplight
import com.geebeeayy.app.ui.theme.HoneyLight
import com.geebeeayy.app.ui.theme.LensCyan
import com.geebeeayy.app.ui.theme.NeonMagenta
import com.geebeeayy.app.ui.theme.NeonViolet
import com.geebeeayy.app.ui.theme.NightPanel
import com.geebeeayy.app.ui.theme.NightRaised
import com.geebeeayy.app.ui.theme.NightVoid
import com.geebeeayy.app.ui.theme.PineGlowMist

/**
 * The five colours every on-screen control is built from.
 *
 * One object rather than five settings: a button body, the same body while
 * held, and a label that has to stay readable on either of them are one
 * decision, not three. Picking them apart is how a theme ends up with white
 * text on a yellow button.
 */
data class ControlPalette(
    /** The button body at rest. */
    val fill: Color,
    /** The button body while held. */
    val pressed: Color,
    /** Text, and the D-pad's arrows, on [fill]. */
    val label: Color,
    /** Text, and the D-pad's arrows, on [pressed]. */
    val labelPressed: Color,
    /** The D-pad's thumb rest, which is also its dead zone. */
    val dish: Color,
)

/** A named control colour scheme, as offered in Settings. */
enum class ControlTint(val label: String) {
    NIGHT("Night"),
    HONEY("Honey"),
    VIOLET("Violet"),
    CLASSIC("Classic grey");

    val palette: ControlPalette
        get() = when (this) {
            NIGHT -> ControlPalette(
                fill = NightPanel,
                pressed = AmberResin,
                label = PineGlowMist,
                labelPressed = BurntRoot,
                dish = NightRaised,
            )
            HONEY -> ControlPalette(
                fill = AmberResin,
                pressed = GoldenSaplight,
                label = PineGlowMist,
                labelPressed = BurntRoot,
                dish = HoneyLight,
            )
            VIOLET -> ControlPalette(
                fill = NeonViolet,
                pressed = LensCyan,
                label = PineGlowMist,
                labelPressed = NightVoid,
                dish = NeonMagenta,
            )
            // The AGB-001's own buttons: grey body, lighter grey where the
            // moulding catches the light. The one scheme here that is not
            // from the app's palette, because it is quoting the hardware.
            CLASSIC -> ControlPalette(
                fill = Color(0xFF4A4A55),
                pressed = Color(0xFFB6B6C2),
                label = Color(0xFFE6E6EE),
                labelPressed = Color(0xFF1E1E24),
                dish = Color(0xFF32323C),
            )
        }
}
