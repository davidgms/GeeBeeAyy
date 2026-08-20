package com.geebeeayyayy.app.engine

/**
 * JNI bridge to the GeeBeeAyy Rust core.
 *
 * Requires libgeebeeayy_core.so to be loaded via System.loadLibrary("geebeeayy_core").
 */
class GbaEngine {

    companion object {
        init {
            System.loadLibrary("geebeeayy_core")
        }

        /** Width of the GBA screen in pixels. */
        const val SCREEN_WIDTH = 240
        /** Height of the GBA screen in pixels. */
        const val SCREEN_HEIGHT = 160
        /** Bytes per pixel (RGB888). */
        const val BYTES_PER_PIXEL = 3
        /** Total frame buffer size in bytes. */
        const val FRAME_BUFFER_SIZE = SCREEN_WIDTH * SCREEN_HEIGHT * BYTES_PER_PIXEL
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
        return nativeLoadRom(handle, data, data.size) == 0
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
     * Get audio samples (f32 mono).
     * @param maxSamples Maximum number of samples to retrieve.
     * @return Array of f32 samples, or empty if none available.
     */
    fun getAudioSamples(maxSamples: Int = 4096): FloatArray {
        ensureHandle()
        val samples = FloatArray(maxSamples)
        val count = nativeAudioCopy(handle, samples, maxSamples)
        return samples.copyOf(count)
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
    private external fun nativeSaveStateCreate(handle: Long): Long
    private external fun nativeLoadState(handle: Long, stateHandle: Long): Int
    private external fun nativeSaveStateDestroy(stateHandle: Long)
}
