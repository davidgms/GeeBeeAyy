package com.geebeeayy.app.ui

import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Rect
import org.junit.Assert.assertEquals
import org.junit.Test

class ControlDragTest {

    private val screen = Rect(0f, 0f, 1080f, 2400f)

    @Test
    fun `a drag inside the screen passes through`() {
        val dpad = Rect(66f, 1806f, 462f, 2202f)
        assertEquals(Offset(100f, -200f), clampedDrag(dpad, Offset(100f, -200f), screen))
    }

    @Test
    fun `a control stops at the bottom right corner`() {
        // The device run dragged the d-pad to (1075, 2395) and two thirds of
        // it left the screen. It must stop with its edges on the screen's.
        val dpad = Rect(66f, 1806f, 462f, 2202f)
        assertEquals(Offset(618f, 198f), clampedDrag(dpad, Offset(5000f, 5000f), screen))
    }

    @Test
    fun `a control stops at the top left corner`() {
        val dpad = Rect(66f, 1806f, 462f, 2202f)
        assertEquals(Offset(-66f, -1806f), clampedDrag(dpad, Offset(-5000f, -5000f), screen))
    }

    @Test
    fun `a control already off screen can still be dragged back`() {
        // Layouts saved before the clamp may hold a control half outside;
        // a drag toward the screen must not be refused.
        val outside = Rect(900f, 2200f, 1296f, 2596f)
        assertEquals(Offset(-300f, -300f), clampedDrag(outside, Offset(-300f, -300f), screen))
    }

    @Test
    fun `the area need not start at the window's corner`() {
        // The controls' box sits below the top bar and above the status strip.
        val box = Rect(0f, 266f, 1080f, 2266f)
        val dpad = Rect(66f, 1806f, 462f, 2202f)
        assertEquals(Offset(618f, 64f), clampedDrag(dpad, Offset(5000f, 5000f), box))
        assertEquals(Offset(-66f, -1540f), clampedDrag(dpad, Offset(-5000f, -5000f), box))
    }

    @Test
    fun `a drag further out from off screen is held where it is`() {
        val outside = Rect(900f, 2200f, 1296f, 2596f)
        assertEquals(Offset.Zero, clampedDrag(outside, Offset(50f, 50f), screen))
    }
}
