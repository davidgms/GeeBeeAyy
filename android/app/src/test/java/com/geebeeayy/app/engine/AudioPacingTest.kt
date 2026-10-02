package com.geebeeayy.app.engine

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class AudioPacingTest {

    // --- FrameSkipGovernor ---

    @Test
    fun `a healthy buffer never skips`() {
        val g = FrameSkipGovernor()
        repeat(10) { assertFalse(g.shouldSkip(underruns = 0, bufferedSamples = 1_200)) }
    }

    @Test
    fun `an unknown buffer fill never skips`() {
        val g = FrameSkipGovernor()
        repeat(10) { assertFalse(g.shouldSkip(underruns = 0, bufferedSamples = -1)) }
    }

    @Test
    fun `a nearly empty buffer skips`() {
        assertTrue(FrameSkipGovernor().shouldSkip(0, 100))
    }

    @Test
    fun `underrun growth skips even with a full buffer`() {
        val g = FrameSkipGovernor()
        assertFalse(g.shouldSkip(2, 1_600))
        assertTrue(g.shouldSkip(3, 1_600))
        assertFalse(g.shouldSkip(3, 1_600))
    }

    @Test
    fun `skips are capped and the next frame is drawn`() {
        val g = FrameSkipGovernor(maxConsecutive = 3)
        val decisions = List(5) { g.shouldSkip(0, 0) }
        assertEquals(listOf(true, true, true, false, true), decisions)
    }

    // --- AudioWatchdog ---

    @Test
    fun `a moving playback head never trips the watchdog`() {
        val w = AudioWatchdog(limit = 100)
        repeat(1_000) { assertFalse(w.observe(head = it * 800L, wrote = true)) }
    }

    @Test
    fun `a frozen head trips after the limit and then re-arms`() {
        val w = AudioWatchdog(limit = 100)
        assertFalse(w.observe(5L, true)) // first sample only sets the baseline
        repeat(99) { assertFalse(w.observe(5L, true)) }
        assertTrue(w.observe(5L, true))
        assertFalse(w.observe(5L, true))
    }

    @Test
    fun `failed writes trip it even if the head reads as moving`() {
        val w = AudioWatchdog(limit = 3)
        assertFalse(w.observe(1L, false))
        assertFalse(w.observe(2L, false))
        assertTrue(w.observe(3L, false))
    }

    @Test
    fun `one good frame resets the stall count`() {
        val w = AudioWatchdog(limit = 3)
        w.observe(1L, true)
        w.observe(1L, true)
        w.observe(2L, true)
        assertFalse(w.observe(2L, true))
        assertFalse(w.observe(2L, true))
        assertTrue(w.observe(2L, true))
    }

    // --- alignToBurst ---

    @Test
    fun `buffer rounds up to a whole number of bursts`() {
        assertEquals(1_728, alignToBurst(1_600, 192))
        assertEquals(1_600, alignToBurst(1_600, 160))
    }

    @Test
    fun `buffer never shrinks and tolerates a missing burst size`() {
        assertEquals(1_600, alignToBurst(1_600, 0))
        assertEquals(1_600, alignToBurst(1_600, -1))
        assertTrue(alignToBurst(1_600, 3_000) >= 1_600)
    }
}
