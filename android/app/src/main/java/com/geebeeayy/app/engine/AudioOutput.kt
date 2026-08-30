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

        /** The core emits one sample every 964 cycles of the 16.78 MHz clock. */
        const val SAMPLE_RATE = 16_777_216 / 964

        /** Samples the core produces per video frame, at ~59.73 Hz. */
        const val SAMPLES_PER_FRAME = SAMPLE_RATE / 60

        /** Buffer depth in frames. Below three, brief scheduling hiccups underrun. */
        private const val BUFFERED_FRAMES = 3
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
        val bufferBytes = maxOf(minBytes, BUFFERED_FRAMES * SAMPLES_PER_FRAME * Float.SIZE_BYTES)

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
