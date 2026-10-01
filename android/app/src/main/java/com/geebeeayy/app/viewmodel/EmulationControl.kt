package com.geebeeayy.app.viewmodel

import java.util.concurrent.ConcurrentLinkedQueue

/** A user-triggered save/load-state request, run on the engine thread. */
internal sealed class StateCommand {
    data class Save(val slot: Int) : StateCommand()

    /** [sidecar] is the achievement runtime's state, restored only if the core accepts [bytes]. */
    class LoadBytes(val bytes: ByteArray, val sidecar: ByteArray?) : StateCommand()
}

/**
 * Mailbox from any thread to the emulation loop. It used to be a single
 * volatile slot, so a second request before the loop's next frame silently
 * replaced the first. Requests are discrete user actions, so an unbounded
 * queue is fine: the loop empties it every frame.
 */
internal class CommandQueue<T : Any> {
    private val queue = ConcurrentLinkedQueue<T>()

    fun offer(command: T) {
        queue.add(command)
    }

    /** Hand every queued command to [handle], oldest first. */
    fun drain(handle: (T) -> Unit) {
        while (true) handle(queue.poll() ?: return)
    }
}

/**
 * Apply a loaded state, and restore the achievement sidecar only if the core
 * accepted it. A rejected state rolls the machine back, so restoring first
 * left the achievement runtime at a moment the game was no longer at.
 */
internal fun applyStateLoad(
    bytes: ByteArray,
    sidecar: ByteArray?,
    writeState: (ByteArray) -> Boolean,
    restoreProgress: (ByteArray) -> Unit,
): Boolean {
    if (!writeState(bytes)) return false
    if (sidecar != null) runCatching { restoreProgress(sidecar) }
    return true
}

/**
 * Whether the loop may run. Starting is gated on the screen being resumed:
 * a ROM that finishes loading after `onPause`, or a restored process, used to
 * start emulating behind a lock screen.
 */
internal class RunGate {
    /** True between the screen's resume and pause. */
    @Volatile
    var resumed = false

    /** True after the player pressed pause; backgrounding must not undo it. */
    @Volatile
    var userPaused = false

    fun mayStart(romLoaded: Boolean): Boolean = romLoaded && resumed

    fun shouldAutoResume(romLoaded: Boolean, running: Boolean): Boolean =
        mayStart(romLoaded) && !userPaused && !running
}
