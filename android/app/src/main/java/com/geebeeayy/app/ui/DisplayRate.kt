package com.geebeeayy.app.ui

import android.view.Window
import kotlin.math.abs
import kotlin.math.roundToInt

/** A display mode reduced to what [gameDisplayMode] needs, so it can be tested off-device. */
data class DisplayModeSpec(val id: Int, val width: Int, val height: Int, val refreshRate: Float)

/** The GBA's frame rate: 16.78 MHz / 280896 cycles per frame. */
internal const val GBA_FPS = 59.7275f

/**
 * The display mode to ask for while a game runs, or null to leave the system's choice.
 *
 * Phones with adaptive refresh drop to whatever rate looks "enough" for the
 * content: the Mi 10T Pro sat at 50 Hz during play, so ten of every sixty GBA
 * frames were never shown and scrolling judders. A rate that is a whole
 * multiple of the GBA's (60, 120) shows every frame for an equal time. The
 * lowest such rate wins, because a higher one only costs battery here.
 * Only modes at the current resolution are considered: a resolution switch
 * flashes the screen and rescales the UI.
 */
fun gameDisplayMode(modes: List<DisplayModeSpec>, width: Int, height: Int): DisplayModeSpec? =
    modes
        .filter { it.width == width && it.height == height }
        .filter { mode ->
            val multiple = mode.refreshRate / GBA_FPS
            multiple.roundToInt() >= 1 && abs(multiple - multiple.roundToInt()) < 0.02f
        }
        .minByOrNull { it.refreshRate }

/**
 * Ask [window] for the [gameDisplayMode], returning what to restore on the way out.
 * The request is a hint: the system may still override it (battery saver, heat).
 * MIUI's smart refresh ignores both hints - measured on a Mi 10T Pro, it stays
 * at 50 Hz - and also the per-surface vote in [GameSurfaceRenderer], although
 * the compositor lists both votes (60 Exact, 59.73 ExactOrMultiple).
 */
fun requestGameDisplayMode(window: Window): () -> Unit {
    @Suppress("DEPRECATION") // Window.getContext().display needs API 30; minSdk is 26.
    val display = window.windowManager.defaultDisplay
    val current = display.mode
    val pick = gameDisplayMode(
        display.supportedModes.map { DisplayModeSpec(it.modeId, it.physicalWidth, it.physicalHeight, it.refreshRate) },
        current.physicalWidth,
        current.physicalHeight,
    ) ?: return {}
    val previousId = window.attributes.preferredDisplayModeId
    val previousRate = window.attributes.preferredRefreshRate
    // Both hints: some vendor policies (MIUI's smart refresh) read one and not the other.
    window.attributes = window.attributes.apply {
        preferredDisplayModeId = pick.id
        preferredRefreshRate = pick.refreshRate
    }
    return {
        window.attributes = window.attributes.apply {
            preferredDisplayModeId = previousId
            preferredRefreshRate = previousRate
        }
    }
}

