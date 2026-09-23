package com.geebeeayy.app.data

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The tile a game gets when nobody supplied cover art. It has to be the same
 * tile every time - a thumbnail that changes between launches is what makes a
 * list harder to scan, which is the whole argument in `RomArtwork`.
 */
class RomPlaceholderTest {

    @Test
    fun `two words give two initials`() {
        assertEquals("MT", RomPlaceholder.initials("Mario Tennis"))
    }

    @Test
    fun `one word gives one initial`() {
        assertEquals("C", RomPlaceholder.initials("celeste"))
    }

    /** ROM dumps are full of these and none of them identify the game. */
    @Test
    fun `a region tag is not an initial`() {
        assertEquals("MT", RomPlaceholder.initials("Mario Tennis Advance (U) [!]"))
        assertEquals("YU", RomPlaceholder.initials("Yggdra Union - We'll Never Fight Alone (USA)"))
    }

    @Test
    fun `separators in a file name are word breaks`() {
        assertEquals("FK", RomPlaceholder.initials("fantasy_knight"))
        assertEquals("PP", RomPlaceholder.initials("power-pig"))
    }

    @Test
    fun `a name with no letters still gives something to draw`() {
        assertEquals("?", RomPlaceholder.initials(""))
        assertEquals("?", RomPlaceholder.initials("   "))
        assertEquals("24", RomPlaceholder.initials("240p 4k"))
    }

    @Test
    fun `the hue is stable and inside the colour wheel`() {
        val first = RomPlaceholder.hue("Mario Tennis")
        assertEquals(first, RomPlaceholder.hue("Mario Tennis"))
        assertTrue("hue $first out of range", first in 0..359)
    }

    @Test
    fun `case does not change the colour`() {
        assertEquals(RomPlaceholder.hue("Celeste"), RomPlaceholder.hue("celeste"))
    }

    @Test
    fun `different games get different colours`() {
        // Not a guarantee for every possible pair - 360 hues collide
        // eventually - but the games actually on the test phone must not all
        // come out the same shade.
        val hues = listOf("Mario Tennis", "Yggdra Union", "celeste", "fantasyknight", "240p")
            .map { RomPlaceholder.hue(it) }
        assertEquals(hues.toSet().size, hues.size)
        assertNotEquals(hues[0], hues[1])
    }
}
