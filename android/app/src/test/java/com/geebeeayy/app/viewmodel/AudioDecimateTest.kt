package com.geebeeayy.app.viewmodel

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * Squeezing a fast-forward batch's audio down to one frame's worth is what
 * paces fast forward: `AudioOutput.write` blocks until the device has room,
 * so handing it one real frame of samples releases the loop one real frame
 * later, whatever the ratio.
 *
 * Fast forward used to skip the write entirely and run open loop, which
 * published the picture once per batch - 18 to 37 irregular updates a second
 * instead of 60, which reads as slow motion with dropped frames.
 */
class AudioDecimateTest {

    @Test
    fun `ratio one leaves the samples alone`() {
        val samples = floatArrayOf(1f, 2f, 3f, 4f)
        assertEquals(4, EmulationViewModel.decimate(samples, 4, 1))
        assertEquals(1f, samples[0], 0f)
        assertEquals(4f, samples[3], 0f)
    }

    @Test
    fun `each run of ratio samples becomes their average`() {
        val samples = floatArrayOf(0f, 2f, 4f, 8f, 1f, 3f)
        assertEquals(3, EmulationViewModel.decimate(samples, 6, 2))
        assertEquals(1f, samples[0], 0f)
        assertEquals(6f, samples[1], 0f)
        assertEquals(2f, samples[2], 0f)
    }

    @Test
    fun `a batch of ratio frames comes out one frame long`() {
        for (ratio in 2..4) {
            val perFrame = 804
            val samples = FloatArray(perFrame * ratio) { 0.5f }
            val out = EmulationViewModel.decimate(samples, samples.size, ratio)
            assertEquals(perFrame, out)
            // A constant signal has to survive averaging untouched, or fast
            // forward would quietly change the volume.
            assertEquals(0.5f, samples[0], 1e-6f)
            assertEquals(0.5f, samples[out - 1], 1e-6f)
        }
    }

    @Test
    fun `a tail shorter than one run is kept rather than dropped`() {
        // 803.65 samples a frame means a batch is never an exact multiple of
        // the ratio. Dropping the remainder would shorten every write and
        // drift the clock.
        val samples = floatArrayOf(1f, 1f, 1f, 1f, 1f)
        assertEquals(3, EmulationViewModel.decimate(samples, 5, 2))
        assertEquals(1f, samples[2], 0f)
    }

    @Test
    fun `no samples is not an error`() {
        assertEquals(0, EmulationViewModel.decimate(FloatArray(4), 0, 2))
    }
}
