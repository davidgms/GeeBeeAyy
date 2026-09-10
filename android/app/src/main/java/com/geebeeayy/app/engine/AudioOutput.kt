package com.geebeeayy.app.engine

import android.media.AudioAttributes
import android.media.AudioFormat
import android.media.AudioTrack
import android.os.Build
import android.util.Log

/**
 * Mono float PCM sink for the emulation core.
 *
 * Writes are blocking, so once the track's buffer is full the emulation loop
 * advances at exactly the rate the audio device drains it. That makes the audio
 * clock the timing master, which is what keeps playback from crackling or
 * drifting against the video frame rate.
 */
class AudioOutput {

    companion object {
        private const val TAG = "GeeBeeAyy/Audio"

        /**
         * Must match `geebeeayy_core::apu::SAMPLE_RATE`.
         *
         * 48 kHz is the device mixer's native rate. The core used to emit at
         * 16777216/964 = 17403 Hz, which no device supports, so AudioTrack
         * resampled every buffer and refused the fast path outright -
         * `AUDIO_OUTPUT_FLAG_FAST denied by server` in logcat, with
         * PERFORMANCE_MODE_LOW_LATENCY below doing nothing at all.
         */
        const val SAMPLE_RATE = 48_000

        /** Samples the core produces per video frame, at ~59.73 Hz. */
        const val SAMPLES_PER_FRAME = SAMPLE_RATE / 60

        /**
         * Ceiling on the samples one emulated frame can produce.
         *
         * A GBA frame is 1/59.7275 s, not 1/60, so the core emits 803.65
         * samples per frame and [SAMPLES_PER_FRAME]'s 800 is 0.45% short. A
         * buffer sized on the round number silently loses the tail of any
         * multi-frame batch, because `geebeeayy_audio_copy` stops at the
         * array's length.
         */
        const val MAX_SAMPLES_PER_FRAME = 810

        /**
         * Buffer depth, in emulated video frames.
         *
         * Audio is the timing master here - the emulation loop advances at
         * whatever rate the device drains the track - so this buffer *is* the
         * floor on input latency. `getMinBufferSize` reported 3844 frames on
         * a Mi 10T Pro, which is 80 ms; two emulated frames is 1600, or 33 ms.
         * The fast mixer's own period is 4 ms, so there is room below this,
         * but two frames leaves the loop a whole frame of slack against a
         * scheduling hiccup and measured zero underruns.
         */
        private const val BUFFERED_FRAMES = 2
    }

    private var track: AudioTrack? = null

    /** True once a track is playing and [write] can pace the caller. */
    val isPlaying: Boolean get() = track != null

    fun start() {
        if (track != null) return

        val minBytes = AudioTrack.getMinBufferSize(
            SAMPLE_RATE,
            AudioFormat.CHANNEL_OUT_MONO,
            AudioFormat.ENCODING_PCM_FLOAT,
        )
        if (minBytes <= 0) {
            Log.e(TAG, "AudioTrack rejected ${SAMPLE_RATE}Hz mono float (code $minBytes)")
            return
        }
        // Deliberately *not* clamped up to `minBytes`: that figure is the
        // safe size for the normal mixer, and taking it costs 80 ms of
        // latency on a device whose fast mixer runs a 4 ms period. Ask for
        // the smaller buffer and let the framework raise it if it must.
        val wanted = BUFFERED_FRAMES * SAMPLES_PER_FRAME * Float.SIZE_BYTES
        val bufferBytes = wanted

        track = try {
            AudioTrack.Builder()
                .setAudioAttributes(
                    AudioAttributes.Builder()
                        .setUsage(AudioAttributes.USAGE_GAME)
                        .setContentType(AudioAttributes.CONTENT_TYPE_MUSIC)
                        .build()
                )
                .setAudioFormat(
                    AudioFormat.Builder()
                        .setEncoding(AudioFormat.ENCODING_PCM_FLOAT)
                        .setSampleRate(SAMPLE_RATE)
                        .setChannelMask(AudioFormat.CHANNEL_OUT_MONO)
                        .build()
                )
                .setBufferSizeInBytes(bufferBytes)
                .setTransferMode(AudioTrack.MODE_STREAM)
                .apply {
                    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                        setPerformanceMode(AudioTrack.PERFORMANCE_MODE_LOW_LATENCY)
                    }
                }
                .build()
                .also { it.play() }
        } catch (e: Exception) {
            // A missing or busy audio device must not take the emulator down;
            // the caller falls back to timer-based pacing.
            Log.e(TAG, "Could not open the audio device", e)
            null
        }
    }

    /**
     * Queue [count] samples from [samples], blocking until they fit.
     *
     * @return true if the samples were handed to the audio device, false if
     *   there is no track and the caller has to pace itself.
     */
    fun write(samples: FloatArray, count: Int): Boolean {
        val active = track ?: return false
        // No samples means nothing to block on, so the caller has to pace
        // itself this frame. Returning true here let the emulation loop
        // free-run at whatever speed the CPU allowed.
        if (count <= 0) return false
        val written = active.write(samples, 0, count, AudioTrack.WRITE_BLOCKING)
        if (written < 0) {
            Log.e(TAG, "Audio write failed (code $written)")
            return false
        }
        return true
    }

    fun stop() {
        track?.run {
            pause()
            flush()
        }
    }

    fun resume() {
        track?.play()
    }

    fun release() {
        track?.run {
            stop()
            release()
        }
        track = null
    }
}
