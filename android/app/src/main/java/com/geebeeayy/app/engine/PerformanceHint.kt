package com.geebeeayy.app.engine

import android.content.Context
import android.os.Build
import android.os.PerformanceHintManager
import android.os.Process

/**
 * Tells the scheduler how long one emulated frame of work is allowed to take,
 * so it can pick a CPU frequency and core for the emulation thread. A no-op
 * below API 31 and on devices without the service.
 *
 * Call [report] from the emulation thread every frame and [close] when
 * emulation stops. The session is bound to a thread id, so it is recreated if
 * the coroutine migrates to another pool thread.
 */
class PerformanceHint(context: Context, private val targetNanos: Long) {
    private val manager: PerformanceHintManager? =
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            context.getSystemService(PerformanceHintManager::class.java)
        } else {
            null
        }
    private var session: PerformanceHintManager.Session? = null
    private var sessionTid = 0

    /** [workNanos] is the time spent emulating, excluding the blocking audio write. */
    fun report(workNanos: Long) {
        val m = manager ?: return
        val tid = Process.myTid()
        if (session == null || tid != sessionTid) {
            session?.close()
            session = m.createHintSession(intArrayOf(tid), targetNanos)
            sessionTid = tid
        }
        session?.reportActualWorkDuration(workNanos)
    }

    fun close() {
        session?.close()
        session = null
    }
}
