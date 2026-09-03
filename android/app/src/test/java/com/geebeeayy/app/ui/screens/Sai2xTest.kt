package com.geebeeayy.app.ui.screens

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * 2xSaI is easy to get subtly wrong in ways that only show as a slightly soft
 * image, so these check the properties that distinguish it from a plain blur:
 * flat areas stay flat, hard runs survive, and it never writes out of bounds.
 */
class Sai2xTest {

    private fun argb(r: Int, g: Int, b: Int) = (0xFF shl 24) or (r shl 16) or (g shl 8) or b

    @Test
    fun `a flat image doubles to a flat image`() {
        val w = 8
        val h = 8
        val colour = argb(0x12, 0x34, 0x56)
        val src = IntArray(w * h) { colour }
        val dst = IntArray(4 * w * h)

        Sai2x.scale(src, dst, w, h)

        assertArrayEquals(IntArray(4 * w * h) { colour }, dst)
    }

    @Test
    fun `a hard vertical edge stays hard away from the seam`() {
        val w = 8
        val h = 8
        val left = argb(0xFF, 0, 0)
        val right = argb(0, 0, 0xFF)
        // Left half red, right half blue.
        val src = IntArray(w * h) { i -> if (i % w < w / 2) left else right }
        val dst = IntArray(4 * w * h)

        Sai2x.scale(src, dst, w, h)

        // Well inside each flat region the colour must be untouched - a blur
        // would have bled the other colour in.
        val dw = w * 2
        assertEquals(left, dst[2 * dw + 2])
        assertEquals(right, dst[2 * dw + (dw - 3)])
    }

    @Test
    fun `every output pixel is written`() {
        val w = 4
        val h = 4
        val src = IntArray(w * h) { i -> argb(i * 7 and 0xFF, i * 13 and 0xFF, i * 29 and 0xFF) }
        val sentinel = 0x00DEAD00
        val dst = IntArray(4 * w * h) { sentinel }

        Sai2x.scale(src, dst, w, h)

        assertEquals(0, dst.count { it == sentinel })
    }

    @Test
    fun `a destination that is too small is left untouched`() {
        val w = 4
        val h = 4
        val src = IntArray(w * h) { argb(1, 2, 3) }
        val dst = IntArray(4 * w * h - 1)

        Sai2x.scale(src, dst, w, h)

        assertEquals(dst.size, dst.count { it == 0 })
    }

    @Test
    fun `a zero sized image is a no-op rather than a crash`() {
        val dst = IntArray(4)
        Sai2x.scale(IntArray(0), dst, 0, 0)
        assertArrayEquals(IntArray(4), dst)
    }

    @Test
    fun `output alpha is always opaque`() {
        val w = 6
        val h = 6
        val src = IntArray(w * h) { i -> argb(i * 3 and 0xFF, 0, 0xFF - (i and 0xFF)) }
        val dst = IntArray(4 * w * h)

        Sai2x.scale(src, dst, w, h)

        assertEquals(0, dst.count { (it ushr 24) != 0xFF })
    }
}
