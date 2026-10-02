package com.geebeeayy.app.ui

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class RateTipTest {

    @Test
    fun `the MIUI 50 Hz trap shows the tip once it has settled`() {
        assertTrue(RateTip.shouldShow(hz = 50f, enabled = true, alreadyShown = false, lowForMs = 5_000))
    }

    @Test
    fun `a short dip while MIUI steps down is not the trap`() {
        // Coming back from Home MIUI walks 144, 120, 60; a moment below 60
        // must not nag.
        assertFalse(RateTip.shouldShow(hz = 50f, enabled = true, alreadyShown = false, lowForMs = 2_000))
    }

    @Test
    fun `60 Hz and above never shows it`() {
        assertFalse(RateTip.shouldShow(hz = 60f, enabled = true, alreadyShown = false, lowForMs = 60_000))
        // 144 is uneven for GBA, but Home and back would not change it.
        assertFalse(RateTip.shouldShow(hz = 144f, enabled = true, alreadyShown = false, lowForMs = 60_000))
    }

    @Test
    fun `it is shown once and only when enabled`() {
        assertFalse(RateTip.shouldShow(hz = 50f, enabled = true, alreadyShown = true, lowForMs = 60_000))
        assertFalse(RateTip.shouldShow(hz = 50f, enabled = false, alreadyShown = false, lowForMs = 60_000))
    }

    @Test
    fun `on by default only on Xiaomi phones`() {
        // Redmi and POCO phones report Xiaomi as the manufacturer.
        assertTrue(RateTip.defaultEnabled("Xiaomi"))
        assertTrue(RateTip.defaultEnabled("xiaomi"))
        assertFalse(RateTip.defaultEnabled("samsung"))
        assertFalse(RateTip.defaultEnabled("Google"))
    }
}
