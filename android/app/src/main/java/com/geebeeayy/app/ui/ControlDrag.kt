package com.geebeeayy.app.ui

import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Rect

/**
 * The part of [drag] that keeps a control, currently at [bounds], inside [area] -
 * the box the controls live in, so not under the top bar or the status strip.
 *
 * Without it a control could be dragged almost entirely off the screen: the
 * Phase 4 device run left two thirds of the d-pad outside the bottom-right
 * corner, under the B button. A drag never pushes a control *further* out, but
 * one already outside - from a layout saved before this existed, or grown at
 * an edge - can always be dragged back, and is never yanked in on its own.
 */
fun clampedDrag(bounds: Rect, drag: Offset, area: Rect): Offset = Offset(
    drag.x.coerceIn(minOf(area.left - bounds.left, 0f), maxOf(area.right - bounds.right, 0f)),
    drag.y.coerceIn(minOf(area.top - bounds.top, 0f), maxOf(area.bottom - bounds.bottom, 0f)),
)
