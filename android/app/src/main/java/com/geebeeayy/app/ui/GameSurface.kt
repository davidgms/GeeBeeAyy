package com.geebeeayy.app.ui

import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.graphics.RectF
import android.os.Build
import android.view.Surface
import android.view.SurfaceHolder
import com.geebeeayy.app.data.ScaleMode
import com.geebeeayy.app.data.ScreenFilter
import com.geebeeayy.app.engine.GbaEngine
import com.geebeeayy.app.ui.screens.Sai2x
import kotlin.math.floor

/** Where the picture goes inside the play area, in pixels. */
data class PicturePlacement(val left: Float, val top: Float, val width: Float, val height: Float)

/**
 * Place a [srcW] x [srcH] picture in an [availW] x [availH] area.
 *
 * INTEGER is the largest whole multiple that fits, falling back to FIT when
 * even 1x overflows. [verticalBias] is 0 for the top edge, 1 for the bottom;
 * it only matters when the area is taller than the picture, which on a
 * portrait phone it always is.
 */
fun picturePlacement(
    availW: Float,
    availH: Float,
    srcW: Int,
    srcH: Int,
    scaleMode: ScaleMode,
    verticalBias: Float,
): PicturePlacement {
    val fit = minOf(availW / srcW, availH / srcH)
    val (w, h) = when (scaleMode) {
        ScaleMode.STRETCH -> availW to availH
        ScaleMode.FIT -> srcW * fit to srcH * fit
        ScaleMode.INTEGER -> {
            val whole = floor(fit)
            if (whole < 1f) srcW * fit to srcH * fit else srcW * whole to srcH * whole
        }
    }
    return PicturePlacement((availW - w) / 2f, (availH - h) * verticalBias.coerceIn(0f, 1f), w, h)
}

/**
 * Draws GBA frames into a SurfaceView, off the UI thread.
 *
 * Replaces a Compose Canvas that converted every frame on the main thread and
 * recomposed the whole emulation screen 60 times a second. Its own surface also
 * lets the game vote for its frame rate: MIUI's smart refresh held the panel at
 * 50 Hz for the Compose path and ignored the window-level hints, so one frame in
 * six never reached the screen.
 *
 * [draw] and [redraw] may be called from any thread; they are serialised here
 * because the surface callbacks arrive on the main thread.
 */
class GameSurfaceRenderer : SurfaceHolder.Callback {

    @Volatile var scaleMode: ScaleMode = ScaleMode.INTEGER
    @Volatile var filter: ScreenFilter = ScreenFilter.NONE
    @Volatile var verticalBias: Float = 0.5f

    private var surface: Surface? = null
    private var surfaceW = 0
    private var surfaceH = 0
    private var last: ByteArray? = null

    private val pixels = IntArray(GbaEngine.SCREEN_WIDTH * GbaEngine.SCREEN_HEIGHT)
    private val native = Bitmap.createBitmap(GbaEngine.SCREEN_WIDTH, GbaEngine.SCREEN_HEIGHT, Bitmap.Config.ARGB_8888)
    private val doubledPixels by lazy { IntArray(pixels.size * 4) }
    private val doubled by lazy {
        Bitmap.createBitmap(GbaEngine.SCREEN_WIDTH * 2, GbaEngine.SCREEN_HEIGHT * 2, Bitmap.Config.ARGB_8888)
    }
    private val picturePaint = Paint()
    private val scanlinePaint = Paint().apply { color = Color.argb(89, 0, 0, 0) } // 35% black
    private val dst = RectF()

    /** Show [frame], or the last frame again when it is null. */
    @Synchronized
    fun draw(frame: ByteArray?) {
        if (frame != null) last = frame
        redraw()
    }

    @Synchronized
    fun redraw() {
        val target = surface ?: return
        val frame = last ?: return
        // The core has not produced a whole frame yet.
        if (frame.size < GbaEngine.FRAME_BUFFER_SIZE || surfaceW == 0 || surfaceH == 0) return
        val bitmap = upload(frame)
        val canvas = try {
            target.lockHardwareCanvas()
        } catch (_: Exception) {
            return // Surface torn down between the check and the lock.
        }
        try {
            paint(canvas, bitmap)
        } finally {
            target.unlockCanvasAndPost(canvas)
        }
    }

    private fun upload(frame: ByteArray): Bitmap {
        for (i in pixels.indices) {
            val o = i * 3
            pixels[i] = (0xFF shl 24) or
                ((frame[o].toInt() and 0xFF) shl 16) or
                ((frame[o + 1].toInt() and 0xFF) shl 8) or
                (frame[o + 2].toInt() and 0xFF)
        }
        val w = GbaEngine.SCREEN_WIDTH
        val h = GbaEngine.SCREEN_HEIGHT
        if (filter == ScreenFilter.SAI_2X) {
            Sai2x.scale(pixels, doubledPixels, w, h)
            doubled.setPixels(doubledPixels, 0, w * 2, 0, 0, w * 2, h * 2)
            return doubled
        }
        native.setPixels(pixels, 0, w, 0, 0, w, h)
        return native
    }

    private fun paint(canvas: Canvas, bitmap: Bitmap) {
        val p = picturePlacement(
            surfaceW.toFloat(), surfaceH.toFloat(), bitmap.width, bitmap.height, scaleMode, verticalBias,
        )
        canvas.drawColor(Color.BLACK)
        dst.set(p.left, p.top, p.left + p.width, p.top + p.height)
        // Nearest-neighbour by default: bilinear turns scaled-up pixel art to mush.
        picturePaint.isFilterBitmap = filter == ScreenFilter.SMOOTH
        canvas.drawBitmap(bitmap, null, dst, picturePaint)
        if (filter == ScreenFilter.SCANLINES) {
            // Darken the lower half of every GBA row; skipped below 2px a row,
            // where it would only dim the picture.
            val row = p.height / GbaEngine.SCREEN_HEIGHT
            if (row >= 2f) {
                var y = p.top + row / 2f
                while (y < p.top + p.height) {
                    canvas.drawRect(p.left, y, p.left + p.width, y + row / 2f, scanlinePaint)
                    y += row
                }
            }
        }
    }

    override fun surfaceCreated(holder: SurfaceHolder) {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            // A per-layer vote the compositor weighs directly, on a layer that
            // presents real frames. FIXED_SOURCE: the content runs at this
            // rate, so the panel should pick a rate that shows it evenly.
            holder.surface.setFrameRate(GBA_FPS, Surface.FRAME_RATE_COMPATIBILITY_FIXED_SOURCE)
        }
    }

    @Synchronized
    override fun surfaceChanged(holder: SurfaceHolder, format: Int, width: Int, height: Int) {
        surface = holder.surface
        surfaceW = width
        surfaceH = height
        redraw() // A paused game must not come back from rotation to a black screen.
    }

    @Synchronized
    override fun surfaceDestroyed(holder: SurfaceHolder) {
        surface = null
    }
}
