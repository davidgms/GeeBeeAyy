package com.geebeeayy.app.data

import com.geebeeayy.app.data.ControlLayoutStore.Companion.resolveLayoutForGame
import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * The fallback chain behind [ControlLayoutStore.getLayoutForGame], which
 * needs an Android `Context` for its `SharedPreferences` and so cannot be
 * driven end to end here. [resolveLayoutForGame] is the pure decision it
 * makes once the store has fetched its inputs, extracted so the fallback
 * order itself - the part a game losing its controls actually depends on -
 * is checked without one.
 *
 * A wrong answer here strands a game with no controls at all in one
 * orientation, which reads as the layout feature being broken.
 */
class ControlLayoutStoreTest {

    private val orientations = mapOf(
        "default" to LayoutOrientation.BOTH,
        "wide" to LayoutOrientation.LANDSCAPE,
        "tall" to LayoutOrientation.PORTRAIT,
    )
    private val ids = orientations.keys.toList()
    private fun orientationOf(id: String) = orientations.getValue(id)

    private fun resolve(chosen: String?, landscape: Boolean, defaultId: String = "default") =
        resolveLayoutForGame(chosen, ids, defaultId, landscape, ::orientationOf)

    @Test
    fun `a chosen layout that still applies is kept`() {
        assertEquals("wide", resolve(chosen = "wide", landscape = true))
        assertEquals("tall", resolve(chosen = "tall", landscape = false))
    }

    @Test
    fun `a chosen layout that no longer exists falls back to default`() {
        assertEquals("default", resolve(chosen = "deleted", landscape = true))
    }

    @Test
    fun `a chosen layout that no longer applies this way round falls back to default`() {
        // "tall" was picked in portrait; the phone turned landscape.
        assertEquals("default", resolve(chosen = "tall", landscape = true))
    }

    @Test
    fun `no chosen layout falls back to default when default applies`() {
        assertEquals("default", resolve(chosen = null, landscape = true))
        assertEquals("default", resolve(chosen = null, landscape = false))
    }

    @Test
    fun `a default that does not apply falls back to the first layout that does`() {
        // Default itself is landscape-only here, and the phone is upright.
        val landscapeOnlyDefault = mapOf(
            "default" to LayoutOrientation.LANDSCAPE,
            "tall" to LayoutOrientation.PORTRAIT,
        )
        val result = resolveLayoutForGame(
            chosen = null,
            layoutIds = landscapeOnlyDefault.keys.toList(),
            defaultLayoutId = "default",
            landscape = false,
            orientationOf = { landscapeOnlyDefault.getValue(it) },
        )
        assertEquals("tall", result)
    }

    @Test
    fun `nothing applying at all still returns the built-in id rather than nothing`() {
        val allWrong = mapOf("default" to LayoutOrientation.LANDSCAPE)
        val result = resolveLayoutForGame(
            chosen = null,
            layoutIds = allWrong.keys.toList(),
            defaultLayoutId = "default",
            landscape = false,
            orientationOf = { allWrong.getValue(it) },
        )
        assertEquals(ControlLayoutStore.DEFAULT_LAYOUT_ID, result)
    }
}
