package com.geebeeayy.app.ui

import android.content.pm.ActivityInfo
import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * Force Portrait off has to mean "this app rotates freely".
 *
 * It used to return `SCREEN_ORIENTATION_UNSPECIFIED`, which hands the choice
 * to the system and so obeys the phone's auto-rotate lock. With auto-rotate
 * off, turning the setting off did nothing at all.
 */
class OrientationLockTest {

    @Test
    fun `on locks this activity to portrait`() {
        assertEquals(ActivityInfo.SCREEN_ORIENTATION_PORTRAIT, orientationFor(true))
    }

    @Test
    fun `off follows the sensor rather than the system rotation lock`() {
        assertEquals(ActivityInfo.SCREEN_ORIENTATION_FULL_SENSOR, orientationFor(false))
    }
}
