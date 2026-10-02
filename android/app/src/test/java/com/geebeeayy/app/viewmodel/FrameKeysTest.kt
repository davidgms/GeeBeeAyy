package com.geebeeayy.app.viewmodel

import org.junit.Assert.assertEquals
import org.junit.Test

class FrameKeysTest {

    private val a = 1
    private val b = 2

    @Test
    fun `a held key reaches every slice`() {
        val keys = FrameKeys()
        repeat(4) { assertEquals(a, keys.forSlice(held = a, pressedSinceLast = 0)) }
    }

    @Test
    fun `a quick tap is kept until the end of the frame`() {
        // Pressed and released between two polls: with one poll per frame it
        // stayed down for the whole frame, so a game that reads KEYINPUT once
        // in vblank saw it. Slicing must not shrink that to a quarter frame.
        val keys = FrameKeys()
        assertEquals(0, keys.forSlice(held = 0, pressedSinceLast = 0))
        assertEquals(b, keys.forSlice(held = 0, pressedSinceLast = b))
        assertEquals(b, keys.forSlice(held = 0, pressedSinceLast = 0))
        assertEquals(b, keys.forSlice(held = 0, pressedSinceLast = 0))
        keys.endFrame()
        assertEquals(0, keys.forSlice(held = 0, pressedSinceLast = 0))
    }

    @Test
    fun `held and tapped keys combine`() {
        val keys = FrameKeys()
        assertEquals(a or b, keys.forSlice(held = a, pressedSinceLast = b))
    }
}
