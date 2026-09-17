package com.geebeeayy.app.ui

import com.geebeeayy.app.ui.Haptics.Companion.MIN_INTERVAL_MS
import com.geebeeayy.app.ui.Haptics.Companion.shouldBuzz
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * [Haptics.press] itself needs a real [android.os.Vibrator] and
 * [android.os.SystemClock], neither available on the plain JVM this suite
 * runs on. [Haptics.shouldBuzz] is the debounce decision underneath it,
 * extracted so the part that actually decides whether a press buzzes -
 * silent haptics, or a motor that never stops, if this drifts - is checked
 * without either.
 *
 * A COMBO button changes several keys in one frame and the D-pad
 * re-evaluates its key set on every pointer sample, so this is what keeps a
 * thumb resting on a sector boundary from retriggering the motor at
 * touch-sampling rate.
 */
class HapticsTest {

    @Test
    fun `the first buzz fires`() {
        // lastBuzzMs starts at 0; uptime is already well past 0 by the time
        // a player can touch a button.
        assertTrue(shouldBuzz(nowMs = 10_000L, lastBuzzMs = 0L))
    }

    @Test
    fun `a second buzz inside the interval is swallowed`() {
        assertFalse(shouldBuzz(nowMs = MIN_INTERVAL_MS - 1, lastBuzzMs = 0L))
    }

    @Test
    fun `a buzz exactly at the interval fires`() {
        assertTrue(shouldBuzz(nowMs = MIN_INTERVAL_MS, lastBuzzMs = 0L))
    }

    @Test
    fun `a buzz well past the interval fires`() {
        assertTrue(shouldBuzz(nowMs = 1_000L, lastBuzzMs = 0L))
    }
}
