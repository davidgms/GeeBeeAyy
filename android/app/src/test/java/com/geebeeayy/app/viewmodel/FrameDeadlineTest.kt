package com.geebeeayy.app.viewmodel

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * Frame pacing when there is no audio device to block on. The loop used to
 * sleep a whole period *after* emulating, so the emulation time was added to
 * the wait rather than hidden inside it and every frame came late by however
 * long it took to produce.
 */
class FrameDeadlineTest {

    /** One frame at 60 Hz, in nanoseconds. */
    private val period = 16_666_666L

    @Test
    fun `a batch that finished early sleeps only the remainder`() {
        val deadline = 1_000_000_000L
        // 6 ms of work out of a 16.67 ms slice.
        val now = deadline + 6_000_000L
        val next = EmulationViewModel.advanceDeadline(deadline, now, period)

        assertEquals(deadline + period, next)
        assertEquals(period - 6_000_000L, next - now)
    }

    @Test
    fun `a batch that used its whole slice leaves nothing to sleep`() {
        val deadline = 1_000_000_000L
        val now = deadline + period
        val next = EmulationViewModel.advanceDeadline(deadline, now, period)

        assertEquals(now, next)
    }

    @Test
    fun `a device that cannot keep up snaps instead of building a debt`() {
        var deadline = 1_000_000_000L
        var now = deadline
        // Every batch takes four times its slice. Ten of them in a row must
        // not leave the deadline ten slices behind the clock.
        repeat(10) {
            now += period * 4
            deadline = EmulationViewModel.advanceDeadline(deadline, now, period)
        }
        assertEquals(now, deadline)
    }
}
