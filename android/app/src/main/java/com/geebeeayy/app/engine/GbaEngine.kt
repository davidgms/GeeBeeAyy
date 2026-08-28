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
     * Has the cartridge's battery save changed since the last call?
     *
     * Clears the dirty flag as a side effect. Callers must call this
     * *before* [readSave] - a write landing between the read and the clear
     * would otherwise be lost.
     */
    fun saveTakeDirty(): Boolean {
        ensureHandle()
        return nativeSaveTakeDirty(handle) != 0
    }

    /**
     * Copy the cartridge's battery save bytes out.
     * @return the save bytes, empty if the cart has no save chip.
     */
    fun readSave(): ByteArray {
        ensureHandle()
        return nativeSaveRead(handle)
    }

    /**
     * Restore the cartridge's battery save from [data].
     * @return true on success.
     */
    fun writeSave(data: ByteArray): Boolean {
        ensureHandle()
        return nativeSaveWrite(handle, data) == 0
    }

    /**
     * Snapshot the whole machine as a versioned, self-describing byte blob.
     * @return the save state bytes, empty on failure.
     */
    fun readState(): ByteArray {
        ensureHandle()
        return nativeStateRead(handle)
    }

    /**
     * Restore the machine from a byte blob produced by [readState].
     *
     * A `false` result means the load was rejected (bad version, truncated
     * or corrupt data) - and because the core's restore writes into the live
     * machine as it parses, a rejected load can leave a hybrid of the old
     * and new states rather than the original untouched. Callers must treat
     * the session as unreliable rather than let emulation continue on it.
     *
     * @return true on success.
     */
    fun writeState(data: ByteArray): Boolean {
        ensureHandle()
        return nativeStateWrite(handle, data) == 0
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
    private external fun nativeSaveTakeDirty(handle: Long): Int
    private external fun nativeSaveRead(handle: Long): ByteArray
    private external fun nativeSaveWrite(handle: Long, data: ByteArray): Int
    private external fun nativeStateRead(handle: Long): ByteArray
    private external fun nativeStateWrite(handle: Long, data: ByteArray): Int
}
