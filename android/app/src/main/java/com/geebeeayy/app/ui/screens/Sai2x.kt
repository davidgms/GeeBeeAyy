package com.geebeeayy.app.ui.screens

/**
 * 2xSaI - "2x Scale and Interpolation", Derek Liauw Kie Fa's algorithm.
 *
 * Doubles a 32-bit ARGB image, choosing between duplicating a pixel and
 * blending its neighbours based on how the surrounding 4x4 window matches up.
 * Unlike a plain bilinear upscale it leaves a flat run of identical pixels
 * flat, so text and hard edges survive while diagonals get smoothed.
 *
 * Kept as free functions on plain `IntArray`s rather than anything Compose- or
 * Bitmap-shaped so the arithmetic can be exercised on its own.
 */
object Sai2x {

    /**
     * Scale [src] (`w` x `h`, ARGB_8888) into [dst] (`2w` x `2h`).
     *
     * [dst] must be at least `4 * w * h` long; shorter and this returns
     * without touching it, because a half-written frame is worse than a
     * skipped one.
     */
    fun scale(src: IntArray, dst: IntArray, w: Int, h: Int) {
        if (w <= 0 || h <= 0) return
        if (src.size < w * h || dst.size < 4 * w * h) return

        val dw = w * 2
        for (y in 0 until h) {
            for (x in 0 until w) {
                // The 4x4 neighbourhood 2xSaI works from, clamped at the
                // edges so the border repeats rather than wrapping to the
                // opposite side of the image.
                val colB = at(src, w, h, x, y - 1)
                val colE = at(src, w, h, x, y)
                val colD = at(src, w, h, x - 1, y)
                val colF = at(src, w, h, x + 1, y)
                val colH = at(src, w, h, x, y + 1)
                val colA = at(src, w, h, x - 1, y - 1)
                val colC = at(src, w, h, x + 1, y - 1)
                val colG = at(src, w, h, x - 1, y + 1)
                val colI = at(src, w, h, x + 1, y + 1)
                val colJ = at(src, w, h, x + 2, y)
                val colK = at(src, w, h, x, y + 2)
                val colL = at(src, w, h, x + 2, y + 1)
                val colM = at(src, w, h, x + 1, y + 2)

                val product: Int
                val product1: Int
                val product2: Int

                if (colE == colI && colF != colH) {
                    product = colE
                } else if (colF == colH && colE != colI) {
                    product = colF
                } else if (colE == colI && colF == colH) {
                    var r = 0
                    r += weight(colF, colC)
                    r += weight(colF, colA)
                    r += weight(colH, colG)
                    r += weight(colH, colK)
                    product = when {
                        r > 0 -> colE
                        r < 0 -> colF
                        else -> blend4(colE, colF, colH, colI)
                    }
                } else {
                    product = when {
                        colF == colI && colH == colJ -> blend(colF, colH)
                        colE == colH && colF == colL -> blend(colE, colF)
                        else -> blend4(colE, colF, colH, colI)
                    }
                }

                product1 = when {
                    colE == colH && colF != colH && colD == colE && colE != colM -> blend(colE, colH)
                    colD == colE && colH == colI && colE != colH && colD != colK -> blend(colE, colH)
                    else -> if (colE == colH) colE else blend(colE, colH)
                }

                product2 = when {
                    colE == colF && colH != colF && colB == colE && colE != colL -> blend(colE, colF)
                    colB == colE && colF == colC && colE != colF && colB != colJ -> blend(colE, colF)
                    else -> if (colE == colF) colE else blend(colE, colF)
                }

                val o = (y * 2) * dw + (x * 2)
                dst[o] = colE
                dst[o + 1] = product2
                dst[o + dw] = product1
                dst[o + dw + 1] = product
            }
        }
    }

    /** Sample with edge clamping. */
    private fun at(src: IntArray, w: Int, h: Int, x: Int, y: Int): Int =
        src[y.coerceIn(0, h - 1) * w + x.coerceIn(0, w - 1)]

    /**
     * 2xSaI's "which side looks more like a run" vote: +1 when the pair is
     * equal, -1 when it is not.
     */
    private fun weight(a: Int, b: Int): Int = if (a == b) 1 else -1

    /** Average of two ARGB pixels, per channel, alpha forced opaque. */
    private fun blend(a: Int, b: Int): Int {
        if (a == b) return a
        val r = (((a shr 16) and 0xFF) + ((b shr 16) and 0xFF)) / 2
        val g = (((a shr 8) and 0xFF) + ((b shr 8) and 0xFF)) / 2
        val bl = ((a and 0xFF) + (b and 0xFF)) / 2
        return (0xFF shl 24) or (r shl 16) or (g shl 8) or bl
    }

    /** Average of four ARGB pixels, per channel, alpha forced opaque. */
    private fun blend4(a: Int, b: Int, c: Int, d: Int): Int {
        val r = (((a shr 16) and 0xFF) + ((b shr 16) and 0xFF) +
            ((c shr 16) and 0xFF) + ((d shr 16) and 0xFF)) / 4
        val g = (((a shr 8) and 0xFF) + ((b shr 8) and 0xFF) +
            ((c shr 8) and 0xFF) + ((d shr 8) and 0xFF)) / 4
        val bl = ((a and 0xFF) + (b and 0xFF) + (c and 0xFF) + (d and 0xFF)) / 4
        return (0xFF shl 24) or (r shl 16) or (g shl 8) or bl
    }
}
