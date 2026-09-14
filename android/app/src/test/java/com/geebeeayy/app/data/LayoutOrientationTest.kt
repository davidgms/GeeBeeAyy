package com.geebeeayy.app.data

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The rule that decides which layouts a player is offered. Getting it wrong
 * either hides every layout or offers a portrait arrangement sideways, and
 * both read as the layout feature being broken.
 */
class LayoutOrientationTest {

    @Test
    fun `a portrait layout applies only upright`() {
        assertTrue(LayoutOrientation.PORTRAIT.appliesTo(landscape = false))
        assertFalse(LayoutOrientation.PORTRAIT.appliesTo(landscape = true))
    }

    @Test
    fun `a landscape layout applies only sideways`() {
        assertFalse(LayoutOrientation.LANDSCAPE.appliesTo(landscape = false))
        assertTrue(LayoutOrientation.LANDSCAPE.appliesTo(landscape = true))
    }

    @Test
    fun `both applies either way`() {
        assertTrue(LayoutOrientation.BOTH.appliesTo(landscape = false))
        assertTrue(LayoutOrientation.BOTH.appliesTo(landscape = true))
    }

    /** A layout saved before this setting existed must not vanish. */
    @Test
    fun `an unknown or missing name reads as both`() {
        assertEquals(LayoutOrientation.BOTH, LayoutOrientation.fromName(null))
        assertEquals(LayoutOrientation.BOTH, LayoutOrientation.fromName(""))
        assertEquals(LayoutOrientation.BOTH, LayoutOrientation.fromName("SIDEWAYS"))
        assertEquals(LayoutOrientation.PORTRAIT, LayoutOrientation.fromName("PORTRAIT"))
    }
}
