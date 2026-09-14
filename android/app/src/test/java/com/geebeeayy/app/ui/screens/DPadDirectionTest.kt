package com.geebeeayy.app.ui.screens

import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.unit.IntSize
import com.geebeeayy.app.engine.GbaEngine
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The D-pad went from four buttons to one cross so a single finger could
 * produce a diagonal. All of that lives in [dpadKeysAt]: the drawing is just
 * the drawing, this is the part that can be wrong without looking wrong.
 */
class DPadDirectionTest {

    private val size = IntSize(144, 144)
    private val centre = Offset(72f, 72f)

    /** [dx]/[dy] in screen terms: y grows downward. */
    private fun at(dx: Float, dy: Float) = dpadKeysAt(Offset(centre.x + dx, centre.y + dy), size)

    @Test
    fun `cardinals report exactly one direction`() {
        assertEquals(setOf(GbaEngine.KEY_UP), at(0f, -60f))
        assertEquals(setOf(GbaEngine.KEY_DOWN), at(0f, 60f))
        assertEquals(setOf(GbaEngine.KEY_LEFT), at(-60f, 0f))
        assertEquals(setOf(GbaEngine.KEY_RIGHT), at(60f, 0f))
    }

    @Test
    fun `corners report both directions of their quadrant`() {
        assertEquals(setOf(GbaEngine.KEY_UP, GbaEngine.KEY_RIGHT), at(50f, -50f))
        assertEquals(setOf(GbaEngine.KEY_UP, GbaEngine.KEY_LEFT), at(-50f, -50f))
        assertEquals(setOf(GbaEngine.KEY_DOWN, GbaEngine.KEY_LEFT), at(-50f, 50f))
        assertEquals(setOf(GbaEngine.KEY_DOWN, GbaEngine.KEY_RIGHT), at(50f, 50f))
    }

    @Test
    fun `the centre dish reports nothing`() {
        assertTrue(dpadKeysAt(centre, size).isEmpty())
        // Just inside the dead zone's edge: 22% of a 72px half-width is ~15.8px.
        assertTrue(at(14f, 0f).isEmpty())
        // And just outside it, the pad wakes up again.
        assertEquals(setOf(GbaEngine.KEY_RIGHT), at(18f, 0f))
    }

    @Test
    fun `a cardinal owns 60 degrees and a diagonal owns 30`() {
        // 29 degrees off straight up is still Up; 31 degrees is a diagonal.
        val r = 60f
        val just = Math.toRadians(29.0)
        val past = Math.toRadians(31.0)
        assertEquals(
            setOf(GbaEngine.KEY_UP),
            at((r * Math.sin(just)).toFloat(), (-r * Math.cos(just)).toFloat()),
        )
        assertEquals(
            setOf(GbaEngine.KEY_UP, GbaEngine.KEY_RIGHT),
            at((r * Math.sin(past)).toFloat(), (-r * Math.cos(past)).toFloat()),
        )
    }

    @Test
    fun `every point outside the dead zone reports something`() {
        // A gap between sectors would be a spot where the pad silently does
        // nothing, which is the failure players describe as "it dropped".
        for (degrees in 0 until 360) {
            val a = Math.toRadians(degrees.toDouble())
            val keys = at((60 * Math.cos(a)).toFloat(), (-60 * Math.sin(a)).toFloat())
            assertTrue("no direction at $degrees degrees", keys.isNotEmpty())
            assertTrue("too many directions at $degrees degrees", keys.size <= 2)
        }
    }

    @Test
    fun `opposite directions are never reported together`() {
        for (degrees in 0 until 360) {
            val a = Math.toRadians(degrees.toDouble())
            val keys = at((60 * Math.cos(a)).toFloat(), (-60 * Math.sin(a)).toFloat())
            assertTrue(
                "up and down together at $degrees degrees",
                !(GbaEngine.KEY_UP in keys && GbaEngine.KEY_DOWN in keys),
            )
            assertTrue(
                "left and right together at $degrees degrees",
                !(GbaEngine.KEY_LEFT in keys && GbaEngine.KEY_RIGHT in keys),
            )
        }
    }

    /**
     * The width of a diagonal is a setting, because there is no right answer:
     * it depends on the thumb, on where the pad sits, and on whether the game
     * is a platformer or a menu. These pin the two ends of that range.
     */
    @Test
    fun `at the widest straight setting there are no diagonals`() {
        val size = IntSize(144, 144)
        // 45 degrees of half-width means the four sectors meet edge to edge.
        for (degrees in 0 until 360 step 3) {
            val radians = Math.toRadians(degrees.toDouble())
            val keys = dpadKeysAt(
                Offset(
                    72f + (60f * kotlin.math.cos(radians)).toFloat(),
                    72f - (60f * kotlin.math.sin(radians)).toFloat(),
                ),
                size,
                cardinalHalfDegrees = 45f,
            )
            assertEquals("$degrees deg reported $keys", 1, keys.size)
        }
    }

    @Test
    fun `a narrow straight setting makes the diagonals wider than the straights`() {
        val size = IntSize(144, 144)
        // 15 degrees of half-width: a straight spans 30 degrees, a diagonal 60.
        var straight = 0
        var diagonal = 0
        for (degrees in 0 until 360 step 3) {
            val radians = Math.toRadians(degrees.toDouble())
            val keys = dpadKeysAt(
                Offset(
                    72f + (60f * kotlin.math.cos(radians)).toFloat(),
                    72f - (60f * kotlin.math.sin(radians)).toFloat(),
                ),
                size,
                cardinalHalfDegrees = 15f,
            )
            if (keys.size == 1) straight++ else diagonal++
        }
        assertTrue(
            "straights $straight, diagonals $diagonal",
            diagonal > straight,
        )
    }
}
