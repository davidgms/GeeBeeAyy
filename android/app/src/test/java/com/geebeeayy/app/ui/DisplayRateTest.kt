package com.geebeeayy.app.ui

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class DisplayRateTest {

    private fun modes(vararg hz: Float) =
        hz.mapIndexed { i, rate -> DisplayModeSpec(id = i + 1, width = 1080, height = 2400, refreshRate = rate) }

    @Test
    fun `a phone that idles at 50 Hz is asked for 60`() {
        // The Mi 10T Pro's modes, as `dumpsys SurfaceFlinger` lists them. It sat
        // at 50 Hz during play, so ten of the GBA's sixty frames a second were
        // never shown.
        val mi10tPro = modes(30f, 48f, 50f, 60f, 90f, 120f, 144f)
        assertEquals(60f, gameDisplayMode(mi10tPro, 1080, 2400)?.refreshRate)
    }

    @Test
    fun `the lowest even multiple wins, to spare the battery`() {
        assertEquals(60f, gameDisplayMode(modes(120f, 60f), 1080, 2400)?.refreshRate)
    }

    @Test
    fun `120 Hz is used when there is no 60`() {
        assertEquals(120f, gameDisplayMode(modes(90f, 120f, 144f), 1080, 2400)?.refreshRate)
    }

    @Test
    fun `no even multiple means no request`() {
        // 90 and 144 do not hold a GBA frame for a whole number of refreshes.
        assertNull(gameDisplayMode(modes(90f, 144f), 1080, 2400))
    }

    @Test
    fun `a mode at another resolution is never picked`() {
        // Switching resolution would flash the screen and rescale the UI.
        val list = listOf(DisplayModeSpec(1, 720, 1600, 60f), DisplayModeSpec(2, 1080, 2400, 50f))
        assertNull(gameDisplayMode(list, 1080, 2400))
    }
}
