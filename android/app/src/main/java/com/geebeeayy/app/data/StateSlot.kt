package com.geebeeayy.app.data

/** One save-state slot as the slot list sees it. */
data class StateSlot(
    val index: Int,
    val exists: Boolean,
    /** Epoch millis of the last write, or 0 when the slot is empty. */
    val savedAt: Long,
)
