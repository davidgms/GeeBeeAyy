package com.geebeeayy.app.ui

import com.geebeeayy.app.data.ScaleMode
import org.junit.Assert.assertEquals
import org.junit.Test

class PicturePlacementTest {

    @Test
    fun `integer scale on a portrait phone is 4x, centred across`() {
        // 1080 wide fits 4 x 240 = 960, leaving 60 each side.
        val p = picturePlacement(1080f, 1500f, 240, 160, ScaleMode.INTEGER, 0.5f)
        assertEquals(PicturePlacement(60f, 430f, 960f, 640f), p)
    }

    @Test
    fun `fit fills the limiting side and keeps 3 to 2`() {
        val p = picturePlacement(1080f, 1500f, 240, 160, ScaleMode.FIT, 0f)
        assertEquals(PicturePlacement(0f, 0f, 1080f, 720f), p)
    }

    @Test
    fun `stretch takes the whole area`() {
        val p = picturePlacement(1080f, 1500f, 240, 160, ScaleMode.STRETCH, 0.5f)
        assertEquals(PicturePlacement(0f, 0f, 1080f, 1500f), p)
    }

    @Test
    fun `vertical bias 1 sits the picture on the bottom edge`() {
        val p = picturePlacement(1080f, 1500f, 240, 160, ScaleMode.INTEGER, 1f)
        assertEquals(860f, p.top)
    }

    @Test
    fun `integer scale smaller than native falls back to fit`() {
        val p = picturePlacement(120f, 80f, 240, 160, ScaleMode.INTEGER, 0.5f)
        assertEquals(PicturePlacement(0f, 0f, 120f, 80f), p)
    }

    @Test
    fun `a 2xSaI source still lands at the same size on screen`() {
        // The doubled bitmap must not double the picture: 480 wide fits 2x in 1080.
        val p = picturePlacement(1080f, 1500f, 480, 320, ScaleMode.INTEGER, 0.5f)
        assertEquals(960f, p.width)
    }
}
