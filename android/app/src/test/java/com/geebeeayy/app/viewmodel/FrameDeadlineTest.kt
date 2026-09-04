package com.geebeeayy.app.viewmodel

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * The fast-forward multiplier is only as honest as this arithmetic. The loop
 * used to sleep a whole frame period *after* emulating, so the emulation time
 * was added to the wait instead of hidden inside it and "8x" delivered under
 * 2x on real hardware.
 */
class FrameDeadlineTest {

    private val period = EmulationViewModel.GBA_FRAME_NANOS

    @Test
    fun `a batch that finished early sleeps only the remainder`() {
        val deadline = 1_000_000_000L
        // 6 ms of work out of a 16.743 ms slice.
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
