package com.geebeeayy.app.ui

import android.content.pm.ActivityInfo
import com.geebeeayy.app.data.ScreenOrientation
import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * Automatic has to mean "this app rotates freely".
 *
 * It used to return `SCREEN_ORIENTATION_UNSPECIFIED`, which hands the choice
 * to the system and so obeys the phone's auto-rotate lock. With auto-rotate
 * off, turning the setting off did nothing at all.
 */
class OrientationLockTest {

    @Test
    fun `automatic follows the sensor rather than the system rotation lock`() {
        assertEquals(
            ActivityInfo.SCREEN_ORIENTATION_FULL_SENSOR,
            orientationFor(ScreenOrientation.AUTO),
        )
    }

    @Test
    fun `portrait locks upright`() {
        assertEquals(
            ActivityInfo.SCREEN_ORIENTATION_PORTRAIT,
            orientationFor(ScreenOrientation.PORTRAIT),
        )
    }

    @Test
    fun `landscape still lets the phone be turned either way round`() {
        // Sensor landscape, not plain landscape: a phone held the other way up
        // should end up the right way up, not upside down.
        assertEquals(
            ActivityInfo.SCREEN_ORIENTATION_SENSOR_LANDSCAPE,
            orientationFor(ScreenOrientation.LANDSCAPE),
        )
    }
}
