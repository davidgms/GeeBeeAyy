package com.geebeeayy.app.viewmodel

/**
 * What the performance overlay shows.
 *
 * [underruns] is cumulative for the life of the audio track, not per second:
 * the number worth watching is whether it is still climbing.
 */
data class PerfStats(val fps: Int, val underruns: Int)
