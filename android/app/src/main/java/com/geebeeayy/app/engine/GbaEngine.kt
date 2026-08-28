package com.geebeeayy.app.engine

import android.util.Log

/**
 * JNI bridge to the GeeBeeAyy Rust core.
 *
 * Requires libgeebeeayy_core.so to be loaded via System.loadLibrary("geebeeayy_core").
 */
class GbaEngine {

    companion object {
        private const val TAG = "GeeBeeAyy/Engine"

        init {
            try {
                System.loadLibrary("geebeeayy_core")
            } catch (e: UnsatisfiedLinkError) {
                Log.e(TAG, "Failed to load native library", e)
                throw e
            }
        }

        /** Width of the GBA screen in pixels. */
        const val SCREEN_WIDTH = 240
        /** Height of the GBA screen in pixels. */
        const val SCREEN_HEIGHT = 160
        /** Bytes per pixel (RGB888). */
        const val BYTES_PER_PIXEL = 3
        /** Total frame buffer size in bytes. */
        const val FRAME_BUFFER_SIZE = SCREEN_WIDTH * SCREEN_HEIGHT * BYTES_PER_PIXEL

        // Key bitmask, GBATEK order. A set bit means pressed; see nativeSetKeys.
        const val KEY_A = 1 shl 0
        const val KEY_B = 1 shl 1
        const val KEY_SELECT = 1 shl 2
        const val KEY_START = 1 shl 3
        const val KEY_RIGHT = 1 shl 4
        const val KEY_LEFT = 1 shl 5
        const val KEY_UP = 1 shl 6
        const val KEY_DOWN = 1 shl 7
        const val KEY_R = 1 shl 8
        const val KEY_L = 1 shl 9
    }

    private var handle: Long = 0L
    private val frameBuffer = ByteArray(FRAME_BUFFER_SIZE)

    val isLoaded: Boolean get() = handle != 0L

    /** Create a new emulator instance. */
    fun create() {
        handle = nativeCreate()
    }

    /** Destroy the emulator instance. */
    fun destroy() {
        if (handle != 0L) {
            nativeDestroy(handle)
            handle = 0L
        }
    }

    /**
     * Load a ROM from a byte array.
     * @return true on success.
     */
    fun loadRom(data: ByteArray): Boolean {
        ensureHandle()
        val result = nativeLoadRom(handle, data, data.size)
        return result == 0
    }

    /** Run a single frame of emulation. */
    fun runFrame() {
        ensureHandle()
        nativeRunFrame(handle)
        nativeFrameBufferCopy(handle, frameBuffer)
    }

    /**
     * Run multiple frames (fast-forward).
     * @param count Number of frames to run.
     */
    fun runFrames(count: Int) {
        ensureHandle()
        nativeRunFrames(handle, count)
        nativeFrameBufferCopy(handle, frameBuffer)
    }

    /**
     * Get the current frame buffer as a ByteArray (240x160 RGB888).
     *
     * Call this after [runFrame] or [runFrames] to get the latest frame.
     */
    fun getFrameBuffer(): ByteArray = frameBuffer

    /**
     * Push the current button state to the core.
     *
     * @param keys Bitmask in GBATEK order (bit 0 = A, 1 = B, 2 = Select,
     * 3 = Start, 4 = Right, 5 = Left, 6 = Up, 7 = Down, 8 = R, 9 = L).
     * A set bit means pressed; the core handles the active-low inversion.
     */
    fun setKeys(keys: Int) {
        ensureHandle()
        nativeSetKeys(handle, keys)
    }

    /**
     * Drain the core's audio buffer into [out] (f32 mono).
     *
     * The caller owns the array so the emulation loop does not allocate once
     * per frame.
     *
     * @return the number of samples written.
     */
    fun readAudio(out: FloatArray): Int {
        ensureHandle()
        return nativeAudioCopy(handle, out, out.size)
    }

    /**
     * Create a save state.
     * @return Opaque handle to the save state, or 0L on failure.
     */
    fun saveStateCreate(): Long {
        ensureHandle()
        return nativeSaveStateCreate(handle)
    }

    /**
     * Restore from a save state.
     * @param stateHandle Handle returned by [saveStateCreate].
     * @return true on success.
     */
    fun loadState(stateHandle: Long): Boolean {
        ensureHandle()
        return nativeLoadState(handle, stateHandle) == 0
    }

    /**
     * Destroy a save state created by [saveStateCreate].
     * @param stateHandle Handle to destroy.
     */
    fun saveStateDestroy(stateHandle: Long) {
        if (stateHandle != 0L) {
            nativeSaveStateDestroy(stateHandle)
        }
    }

    private fun ensureHandle() {
        check(handle != 0L) { "GbaEngine not created. Call create() first." }
    }

    // JNI native methods
    private external fun nativeCreate(): Long
    private external fun nativeDestroy(handle: Long)
    private external fun nativeLoadRom(handle: Long, data: ByteArray, len: Int): Int
    private external fun nativeRunFrame(handle: Long)
    private external fun nativeRunFrames(handle: Long, count: Int)
    private external fun nativeFrameBufferCopy(handle: Long, out: ByteArray)
    private external fun nativeAudioCopy(handle: Long, out: FloatArray, maxSamples: Int): Int
    private external fun nativeSetKeys(handle: Long, keys: Int)
    private external fun nativeSaveStateCreate(handle: Long): Long
    private external fun nativeLoadState(handle: Long, stateHandle: Long): Int
    private external fun nativeSaveStateDestroy(stateHandle: Long)
}
