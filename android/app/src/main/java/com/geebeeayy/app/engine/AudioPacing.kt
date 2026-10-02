package com.geebeeayy.app.engine

/**
 * Decides when to skip *publishing* a frame so a struggling device spends its
 * time on emulation and audio instead. The emulation itself is never skipped:
 * the core still runs every frame, only the picture copy is dropped.
 *
 * Starving means the track's buffer is nearly empty or its underrun count grew
 * since the last call. Skips are capped so the screen still updates.
 */
class FrameSkipGovernor(
    private val maxConsecutive: Int = 3,
    private val lowFillSamples: Int = AudioOutput.SAMPLES_PER_FRAME,
) {
    private var lastUnderruns = -1
    private var consecutive = 0

    /** [bufferedSamples] is negative when there is no audio track to ask. */
    fun shouldSkip(underruns: Int, bufferedSamples: Int): Boolean {
        val grew = lastUnderruns >= 0 && underruns > lastUnderruns
        lastUnderruns = underruns
        val starving = grew || (bufferedSamples in 0 until lowFillSamples)
        if (starving && consecutive < maxConsecutive) {
            consecutive++
            return true
        }
        consecutive = 0
        return false
    }
}

/**
 * Notices a track that stopped consuming (device change, dead object): the
 * playback head frozen, or the write failing, for [limit] frames in a row.
 */
class AudioWatchdog(private val limit: Int = 100) {
    private var lastHead = -1L
    private var stalled = 0

    /** @return true once, when the track should be rebuilt. */
    fun observe(head: Long, wrote: Boolean): Boolean {
        val stuck = !wrote || head == lastHead
        lastHead = head
        stalled = if (stuck) stalled + 1 else 0
        if (stalled >= limit) {
            stalled = 0
            return true
        }
        return false
    }
}

/** Rounds [frames] up to a whole number of [burst]-frame device bursts; never shrinks. */
fun alignToBurst(frames: Int, burst: Int): Int =
    if (burst <= 0) frames else (frames + burst - 1) / burst * burst
