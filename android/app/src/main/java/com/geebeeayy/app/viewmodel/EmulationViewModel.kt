package com.geebeeayy.app.viewmodel

import android.app.Application
import android.hardware.display.DisplayManager
import android.util.Log
import android.view.Display
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import com.geebeeayy.app.data.DisplaySettings
import com.geebeeayy.app.data.LastPlayed
import com.geebeeayy.app.data.RomHeader
import com.geebeeayy.app.data.StateSlot
import com.geebeeayy.app.engine.AudioOutput
import com.geebeeayy.app.engine.GbaEngine
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withTimeoutOrNull
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.isActive
import kotlinx.coroutines.yield
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.io.File
import java.util.concurrent.atomic.AtomicInteger
import java.io.FileOutputStream
import kotlin.math.roundToLong

class EmulationViewModel(application: Application) : AndroidViewModel(application) {

    companion object {
        private const val TAG = "GeeBeeAyy/VM"

        /**
         * Slot 0 is the quick save; 1-7 are the named slots in the list.
         *
         * There used to be a second constant, `STATE_SLOT_COUNT = 10`, which
         * `saveState`/`loadState` validated against while the slot list only
         * enumerated 8. Slots 8 and 9 were writable and then invisible.
         */
        const val SLOT_COUNT = 8

        /**
         * How long `onCleared()` waits for the emulation loop to leave native
         * code before giving up and leaking the core handle.
         *
         * This blocks the main thread, so it is a deadline, not a budget: at
         * 1000 ms it was long enough to be an ANR on its own once the save
         * flush below is added to it. A frame is 16 ms and the loop only has
         * to reach its next lock release, so 250 ms is many frames' grace and
         * still well inside what the system tolerates.
         */
        const val SHUTDOWN_JOIN_TIMEOUT_MS = 250L

        /**
         * Rewind depth and cadence.
         *
         * A snapshot is about 500 KB, so 30 of them is roughly 15 MB - the
         * whole budget for this feature. At one every 12 frames that buys
         * about six seconds of history, which is the useful range for undoing
         * a mistake without turning the emulator into a memory hog.
         */
        const val REWIND_DEPTH = 30
        const val REWIND_INTERVAL_FRAMES = 12

        /**
         * How many times faster than real time the rewind runs.
         *
         * Popping one snapshot per frame made the whole ring unwind in about
         * a third of a second: 10 seconds of play vanished before the finger
         * left the button. Pacing the pops against the snapshot interval
         * gives a rewind you can aim.
         */
        const val REWIND_SPEED = 4

        /**
         * Emulated frames per iteration in *unlimited* fast forward.
         *
         * Unlimited skips the throttle entirely - RetroArch's
         * `fastforward_ratio = 0` and mGBA's unbounded mode do the same - so
         * this is not a speed multiplier. It only decides how much work
         * happens between two frame-buffer publishes: one lock, one JNI
         * crossing and one 150 KB copy per batch instead of per frame.
         *
         * At a fixed ratio the batch *is* the ratio, and the audio write
         * paces it, so there is no such trade to make.
         */
        const val FAST_FORWARD_BATCH = 4

        /**
         * Average each run of [ratio] samples in `samples` into one, in
         * place, and return how many are left.
         *
         * This is what makes a fast-forward ratio exact. `AudioOutput.write`
         * blocks until the device has room, so it is the frame clock: hand it
         * one real frame's worth of samples and it releases the loop one real
         * frame later, however many emulated frames went into them.
         *
         * Averaging, not taking every Nth sample. Plain decimation folds the
         * GBA's square waves back down over themselves and the result is
         * audible grit; a box average is a cheap low-pass and costs one add
         * per sample. Writing back over the input is safe because the output
         * index never overtakes the read index.
         *
         * The short run at the end is averaged too rather than dropped -
         * discarding it would shorten every batch and slowly drift the clock.
         */
        fun decimate(samples: FloatArray, count: Int, ratio: Int): Int {
            if (ratio <= 1 || count <= 0) return count
            var out = 0
            var i = 0
            while (i < count) {
                val run = minOf(ratio, count - i)
                var sum = 0f
                for (j in 0 until run) sum += samples[i + j]
                samples[out++] = sum / run
                i += run
            }
            return out
        }

        /**
         * Carry a pacing deadline forward by one `period`, or snap it to
         * `now` when the work already overran it.
         *
         * Carrying it forward rather than restarting from `now` is what keeps
         * the multiplier honest: a batch that finished 2 ms late shortens the
         * next sleep by 2 ms instead of letting the error accumulate. Snapping
         * when already behind is the other half - without it a device that
         * cannot reach the requested speed builds an ever-growing debt and
         * then sprints through frames trying to repay it.
         */
        fun advanceDeadline(deadline: Long, now: Long, period: Long): Long {
            val next = deadline + period
            return if (next <= now) now else next
        }

        /**
         * What a slot is called in a message, matching the slot list's labels.
         *
         * Slot 0 is the quick save the toolbar buttons use, and the rest are
         * numbered by their index. The messages used to say `slot ${slot + 1}`,
         * so saving into the row labelled "Slot 2" reported "slot 3".
         */
        fun slotName(slot: Int): String =
            if (slot == 0) "quick save slot" else "slot $slot"

        /**
         * A game saving touches thousands of bytes across many CPU cycles;
         * waiting this long after the last dirty flag before writing avoids a
         * file write per byte.
         */
        private const val SAVE_FLUSH_DEBOUNCE_MS = 2_000L
    }

    private val engine = GbaEngine()
    private val audio = AudioOutput()

    /**
     * Reused across frames. Sized from the real per-frame sample count and
     * the highest fast-forward ratio, plus room to spare: a whole batch has
     * to fit, or `geebeeayy_audio_copy` truncates its tail and the sound
     * develops a periodic click.
     */
    private val audioSamples = FloatArray(
        AudioOutput.MAX_SAMPLES_PER_FRAME * (DisplaySettings.MAX_FAST_FORWARD_RATIO + 2)
    )

    /**
     * Fallback pacing when there is no audio device to block on. Measured
     * once from the display rather than assumed to be 60 Hz - audio remains
     * the timing master when it is available, this is only about not
     * fighting the display when it is not.
     */
    private val frameIntervalMs: Long = run {
        val hz = getApplication<Application>()
            .getSystemService(DisplayManager::class.java)
            ?.getDisplay(Display.DEFAULT_DISPLAY)
            ?.refreshRate
            ?.takeIf { it > 1f }
            ?: 60f
        (1000f / hz).roundToLong().coerceAtLeast(1L)
    }

    private val _frameBuffer = MutableStateFlow<ByteArray?>(null)
    val frameBuffer: StateFlow<ByteArray?> = _frameBuffer

    private val _isRunning = MutableStateFlow(false)
    val isRunning: StateFlow<Boolean> = _isRunning

    /**
     * True while fast forward is on. The speed comes from
     * [DisplaySettings.getFastForwardRatio].
     *
     * This used to be unthrottled with no speed to pick, on the grounds that
     * a fixed multiplier the device could not reach was a lie. Pacing the
     * batch with the audio write inverts that: the ratio is now enforced by
     * the clock rather than hoped for, and a device that falls short degrades
     * to what it can do instead of ignoring the setting.
     */
    private val _fastForward = MutableStateFlow(false)
    val fastForward: StateFlow<Boolean> = _fastForward

    /**
     * Emulated frames per real frame period while fast-forwarding, or 0 for
     * unlimited. Re-read on every press rather than at ROM load: this is the
     * one setting a player changes in order to feel the difference straight
     * away.
     */
    private var fastForwardRatio = 2

    /** Whether fast forward plays silence. See [DisplaySettings.getMuteOnFastForward]. */
    private var muteOnFastForward = false

    fun toggleFastForward() {
        val on = !_fastForward.value
        if (on) {
            val settings = DisplaySettings(getApplication())
            fastForwardRatio = settings.getFastForwardRatio()
            muteOnFastForward = settings.getMuteOnFastForward()
        }
        _fastForward.value = on
    }

    /** True while the player is holding the rewind button. */
    private val _isRewinding = MutableStateFlow(false)
    val isRewinding: StateFlow<Boolean> = _isRewinding

    /** Frames between rewind snapshots, and how many the core keeps. */
    private var rewindFrameCounter = 0

    private val _isLoading = MutableStateFlow(true)
    val isLoading: StateFlow<Boolean> = _isLoading

    private val _errorMessage = MutableStateFlow<String?>(null)
    val errorMessage: StateFlow<String?> = _errorMessage

    /** Transient feedback for save/load-state actions (slot N saved, load rejected, ...). */
    private val _stateMessage = MutableStateFlow<String?>(null)
    val stateMessage: StateFlow<String?> = _stateMessage

    private var emulationJob: Job? = null

    /**
     * Serialises every call into [engine].
     *
     * `GbaEngine` hands the core a raw pointer, so two threads inside it at
     * once is memory corruption, not a race on a value. Cancelling the loop
     * only *requests* a stop, and a job parked in `engine.runFrame()` is still
     * in native code long after `stopEmulation()` returns - so pausing and
     * resuming quickly, or `loadRomFromPath` running while the loop is alive,
     * could put two threads on the same `Gba`.
     *
     * Handing the next loop the previous job to join was the first attempt and
     * it does not hold: only the most recent cancelled job was kept, and the
     * join ran inside the new job, so a job cancelled before it was ever
     * dispatched dropped its predecessor unjoined. A lock has no such ordering
     * to get wrong. It is taken per frame rather than for the whole loop, so
     * the blocking audio write never holds it.
     */
    private val engineLock = Mutex()
    private var romLoaded = false

    /**
     * The path [loadRomFromPath] last loaded, so asking for the same one again
     * resumes instead of reloading.
     *
     * The emulation composable is a nav back-stack entry: opening Settings
     * over it disposes it, and coming back re-runs its `LaunchedEffect`. That
     * used to reload the ROM from byte zero and throw away unsaved progress.
     */
    private var loadedRomPath: String? = null

    /** True if backgrounding paused a session the player had not paused themselves. */
    private var pausedByBackground = false

    /**
     * Aggregate button bitmask, written from the input thread and read by the
     * emulation loop, hence `@Volatile`.
     *
     * The loop applies it once per frame rather than letting the input thread
     * call into the engine directly: `GbaEngine` hands the core a raw pointer,
     * so a `setKeys` racing a `runFrame` would be two threads mutating the same
     * `Gba` at once. The cost is at most one frame of input latency.
     */
    @Volatile
    private var keyState = 0

    /**
     * Buttons pressed since the emulation loop last looked, whether or not they
     * are still held. Cleared as it is consumed.
     */
    private val transientPresses = AtomicInteger(0)

    // --- Battery save persistence ------------------------------------------------
    //
    // The core exposes a dirty flag and raw bytes but does no I/O of its own
    // (see docs/save-data.md). Everything below is the frontend's flush
    // policy: debounce writes, land next to the ROM, fall back to app-private
    // storage if that fails.

    private val saveFlushLock = Any()
    private var saveFlushJob: Job? = null

    /** Bytes captured on the engine thread, written to disk on Dispatchers.IO. */
    @Volatile
    private var pendingSaveBytes: ByteArray? = null

    /** Where the next battery-save flush lands; set once the ROM's save is resolved. */
    @Volatile
    private var saveFilePath: String? = null

    @Volatile
    private var fallbackSavePath: String? = null

    // --- Save states ---------------------------------------------------------
    //
    // Keyed by the ROM header's title (0xA0) and game code (0xAC) so two
    // different carts never collide on the same slot file, even if their
    // filenames happen to match.

    private sealed class StateCommand {
        data class Save(val slot: Int) : StateCommand()
        data class LoadBytes(val bytes: ByteArray) : StateCommand()
    }

    /**
     * Single-slot mailbox for a user-triggered save/load-state request,
     * drained by the emulation loop once per frame - same ownership pattern
     * as [keyState]. A second request before the first is drained overwrites
     * it; save/load-state is a discrete user action, not a stream, so this is
     * an acceptable loss under the same "at most one frame of latency" logic.
     */
    @Volatile
    private var pendingStateCommand: StateCommand? = null

    @Volatile
    private var romStateKey: String? = null

    /** The loaded ROM's stable identity - title + game code, not its file
     *  path - for anything keyed "per game" rather than per file, such as
     *  which control layout it uses. Null until [loadRomFromPath] finishes. */
    fun currentRomKey(): String? = romStateKey

    private val statesDir: File by lazy {
        File(getApplication<Application>().filesDir, "states").apply { mkdirs() }
    }

    init {
        engine.create()
    }

    /**
     * Update one button's press state and push the new bitmask to the core.
     *
     * @param key One of [GbaEngine]'s `KEY_*` bit constants.
     */
    fun setKey(key: Int, pressed: Boolean) {
        keyState = if (pressed) keyState or key else keyState and key.inv()
        // Also latch the press. The loop samples `keyState` once per frame, so
        // a press shorter than ~16 ms would otherwise be dropped entirely -
        // measured at 3 ms for a synthetic tap. Latching guarantees every press
        // reaches the core for at least one frame.
        if (pressed) transientPresses.updateAndGet { it or key }
    }

    fun loadRomFromPath(filePath: String) {
        // Already playing this ROM: this is a return from another screen, not
        // a new game. Resume what leaving paused and keep the session.
        if (romLoaded && loadedRomPath == filePath) {
            _isLoading.value = false
            onAppForegrounded()
            return
        }
        viewModelScope.launch(Dispatchers.IO) {
            _isLoading.value = true
            _errorMessage.value = null
            try {
                val file = File(filePath)
                if (!file.exists()) {
                    Log.e(TAG, "ROM not found: $filePath")
                    _errorMessage.value = "ROM file not found"
                    _isLoading.value = false
                    return@launch
                }
                val data = file.readBytes()
                if (data.size < 0xC0) {
                    _errorMessage.value = "File too small (${data.size} bytes)"
                    _isLoading.value = false
                    return@launch
                }
                // Stop first, then take the lock: a running loop is calling
                // into the same handle, and loading a ROM under it is the one
                // engine access that was never funnelled onto the loop thread.
                stopEmulation()
                val success = engineLock.withLock { engine.loadRom(data) }
                if (success) {
                    romLoaded = true
                    loadedRomPath = filePath
                    LastPlayed(getApplication()).record(filePath)
                    engineLock.withLock {
                        // A fresh ROM means a fresh history; the ring is
                        // allocated here rather than at create() so a cold
                        // handle costs nothing.
                        engine.rewindConfigure(REWIND_DEPTH)
                        rewindFrameCounter = 0
                        // Read per ROM launch, the same as the scale mode and
                        // screen filter, so a change in Settings takes effect
                        // the next time a game is opened.
                        engine.setInterframeBlend(
                            DisplaySettings(getApplication()).getInterframeBlend()
                        )
                        resolveSavePaths(file, data)
                        loadExistingSave()
                    }
                    _isLoading.value = false
                    startEmulation()
                } else {
                    _errorMessage.value = "Bad ROM header"
                    _isLoading.value = false
                }
            } catch (e: Exception) {
                Log.e(TAG, "Error loading ROM", e)
                _errorMessage.value = "Error: ${e.message}"
                _isLoading.value = false
            }
        }
    }

    /**
     * Resolve where this ROM's battery save and save states live, from its
     * header. Must run before [startEmulation] - it is plain field
     * assignment plus a header read, no engine calls, so it is safe on the
     * loading coroutine's IO thread.
     */
    private fun resolveSavePaths(romFile: File, romData: ByteArray) {
        romStateKey = computeRomStateKey(romData)
        val primaryDir = romFile.parentFile
        val baseName = romFile.nameWithoutExtension
        saveFilePath = if (primaryDir != null) File(primaryDir, "$baseName.sav").absolutePath else null
        fallbackSavePath = File(fallbackSaveDir(), "${romStateKey}.sav").absolutePath
        if (saveFilePath == null) {
            saveFilePath = fallbackSavePath
        }
    }

    private fun fallbackSaveDir(): File =
        File(getApplication<Application>().filesDir, "saves").apply { mkdirs() }

    /**
     * The filename fragment this cart's saves and states are keyed on.
     *
     * The parsing lives in [RomHeader] because the ROM browser's info dialog
     * needs the same two fields, and two copies of a rule that decides where
     * save states live is one copy too many.
     */
    private fun computeRomStateKey(romData: ByteArray): String =
        RomHeader.from(romData)?.stateKey() ?: "rom"

    /**
     * Load a battery save into the core before the first frame runs, or the
     * game would overwrite a save it never saw. Prefers the ROM-adjacent
     * `.sav`; falls back to the app-private copy if that one is missing but
     * the fallback isn't (e.g. a previous run's write landed there because
     * the ROM folder wasn't writable).
     */
    private fun loadExistingSave() {
        val primary = saveFilePath?.let { File(it) }
        val fallback = fallbackSavePath?.let { File(it) }
        val (file, path) = when {
            primary != null && primary.exists() -> primary to primary.absolutePath
            fallback != null && fallback.exists() -> fallback to fallback.absolutePath
            else -> return
        }
        try {
            val bytes = file.readBytes()
            if (bytes.isNotEmpty()) {
                engine.writeSave(bytes)
                saveFilePath = path
                Log.i(TAG, "Loaded battery save from $path (${bytes.size} bytes)")
            }
        } catch (e: Exception) {
            Log.e(TAG, "Failed reading battery save at $path", e)
        }
    }

    /** Wall-clock instant the batch being paced is allowed to end. */
    private var frameDeadlineNanos = 0L

    /**
     * Sleep whatever is left of this batch's slice, and no more.
     *
     * RetroArch's `limit_frame_time` and DeSmuME's `SpeedThrottle` both carry
     * a deadline forward by a fixed period and sleep only the remainder. The
     * loop used to sleep a whole period *after* the work instead, which adds
     * the emulation time to the wait rather than hiding it inside: a device
     * needing 60 ms to emulate 8 frames spent 60 ms plus a full interval on
     * them, so the "8x" button delivered under 2x.
     *
     * A batch of `speed` frames is one console frame period wide, so the
     * multiplier is exact - the display still gets ~60 updates a second while
     * the core runs `speed` times as many frames. Falling behind snaps the
     * deadline to now rather than building up a debt the loop would then try
     * to sprint off, and that is also what makes the speed self-limiting on a
     * device that cannot reach the requested multiplier.
     */
    private suspend fun delayUntilDeadline() {
        val period = frameIntervalMs * 1_000_000L
        val now = System.nanoTime()
        frameDeadlineNanos = advanceDeadline(frameDeadlineNanos, now, period)
        val remaining = frameDeadlineNanos - now
        if (remaining > 0L) {
            delay(remaining / 1_000_000L)
        }
    }

    fun startEmulation() {
        if (romLoaded && emulationJob == null) {
            _isRunning.value = true
            audio.start()
            audio.resume()
            emulationJob = viewModelScope.launch(Dispatchers.Default) {
                while (isActive) {
                    val fastForwarding = fastForward.value
                    // Sampled once per iteration: the batch size and the
                    // audio squeeze have to agree, and the player can change
                    // this mid-frame from Settings.
                    val ratio = if (fastForwarding) fastForwardRatio else 1
                    val rewinding = isRewinding.value
                    // Everything that reaches into the core happens under the
                    // lock. The blocking audio write below deliberately does
                    // not: it is the frame clock, and holding the lock across
                    // it would stall a ROM load for as long as the device
                    // takes to drain its buffer.
                    val count = engineLock.withLock {
                        // Applied here so every call into the core happens on
                        // this one thread. See the note on `keyState`.
                        // Held keys, plus anything pressed and released since
                        // the last frame, which `keyState` alone has already
                        // forgotten.
                        engine.setKeys(keyState or transientPresses.getAndSet(0))

                        pendingStateCommand?.let { cmd ->
                            pendingStateCommand = null
                            when (cmd) {
                                is StateCommand.Save -> performSaveState(cmd.slot)
                                is StateCommand.LoadBytes -> {
                                    applyStateBytes(cmd.bytes)
                                    // The history belongs to the timeline we
                                    // just left.
                                    engine.rewindClear()
                                    rewindFrameCounter = 0
                                }
                            }
                        }

                        if (rewinding) {
                            // Walking back through the ring one snapshot at a
                            // time, paced so REWIND_INTERVAL_FRAMES of play
                            // take REWIND_INTERVAL_FRAMES / REWIND_SPEED to
                            // undo. Reaching the oldest snapshot holds there
                            // rather than resuming play under the finger.
                            if (engine.rewindPop()) {
                                _frameBuffer.value = engine.getFrameBuffer().copyOf()
                            }
                            -1
                        } else {
                            if (fastForwarding) {
                                // At a fixed ratio the batch is the ratio, so
                                // the audio write below covers exactly one
                                // real frame period and paces it. Unlimited
                                // has no clock, so the batch is only there to
                                // keep the copies off the critical path.
                                engine.runFrames(
                                    if (ratio == 0) FAST_FORWARD_BATCH else ratio
                                )
                            } else {
                                engine.runFrame()
                            }
                            _frameBuffer.value = engine.getFrameBuffer().copyOf()

                            // Snapshot on a cadence, not every frame: a state
                            // is about 500 KB, so REWIND_DEPTH * the interval
                            // decides both the memory cost and how far back
                            // the player can go.
                            if (++rewindFrameCounter >= REWIND_INTERVAL_FRAMES) {
                                rewindFrameCounter = 0
                                engine.rewindPush()
                            }

                            checkSaveDirty()
                            engine.readAudio(audioSamples)
                        }
                    }

                    if (count < 0) {
                        delay(
                            (frameIntervalMs * REWIND_INTERVAL_FRAMES / REWIND_SPEED)
                                .coerceAtLeast(1L)
                        )
                        continue
                    }

                    // A rejected load stops emulation from inside the block
                    // above. Job.cancel() flips isActive synchronously but
                    // this while loop only rechecks it at the top, so without
                    // this guard the loop would write audio to an AudioTrack
                    // that stopEmulation() just stopped.
                    if (!isActive) continue

                    // Unlimited fast forward has no clock: it neither writes
                    // the samples - a blocking audio write would hold it to
                    // real time - nor sleeps. `yield` is still needed, or the
                    // loop would never hand this dispatcher thread to
                    // anything else.
                    if (fastForwarding && ratio == 0) {
                        yield()
                        continue
                    }
                    // A ratio batch holds `ratio` frames of samples. Squeezed
                    // down to one frame's worth, the write blocks for one real
                    // frame period, which is what makes the ratio exact and
                    // publishes the picture at the display's own rate instead
                    // of once per batch.
                    val toWrite =
                        if (fastForwarding) decimate(audioSamples, count, ratio) else count
                    // Silence, not skipping the write: the write is the clock.
                    if (fastForwarding && muteOnFastForward) {
                        audioSamples.fill(0f, 0, toWrite)
                    }
                    if (!audio.write(audioSamples, toWrite)) {
                        // No audio device: one deadline per iteration is one
                        // real frame period per batch, so the ratio still
                        // holds without any arithmetic here.
                        delayUntilDeadline()
                    }
                }
            }
        }
    }

    /**
     * Ordering contract from the core: take-dirty must be called before
     * read, or a write landing between the two is lost. Runs on the
     * emulation loop thread, once per frame.
     */
    private fun checkSaveDirty() {
        if (engine.saveTakeDirty()) {
            pendingSaveBytes = engine.readSave()
            scheduleSaveFlush()
        }
    }

    private fun scheduleSaveFlush() {
        synchronized(saveFlushLock) {
            saveFlushJob?.cancel()
            saveFlushJob = viewModelScope.launch(Dispatchers.IO) {
                delay(SAVE_FLUSH_DEBOUNCE_MS)
                flushPendingSave()
            }
        }
    }

    /** Writes whatever is pending right now, skipping the debounce. Plain blocking I/O, no suspension. */
    private fun flushSaveNow() {
        synchronized(saveFlushLock) {
            saveFlushJob?.cancel()
            saveFlushJob = null
        }
        flushPendingSave()
    }

    private fun flushPendingSave() {
        val bytes = pendingSaveBytes ?: return
        val primary = saveFilePath ?: return
        if (!tryAtomicWrite(primary, bytes)) {
            val fallback = fallbackSavePath
            if (fallback != null && fallback != primary) {
                Log.w(TAG, "Battery save write to $primary failed, falling back to app storage")
                if (tryAtomicWrite(fallback, bytes)) {
                    saveFilePath = fallback
                } else {
                    Log.e(TAG, "Battery save write failed at both $primary and $fallback")
                }
            }
        }
        pendingSaveBytes = null
    }

    /** Temp file then rename, so a crash mid-write never leaves a half-written save on disk. */
    private fun tryAtomicWrite(path: String, bytes: ByteArray): Boolean {
        return try {
            val target = File(path)
            target.parentFile?.mkdirs()
            val tmp = File(target.parentFile, "${target.name}.tmp")
            // fsync before the rename. Without it the rename can land while the
            // bytes are still only in the page cache, so a power loss leaves an
            // atomically-renamed but empty save - the exact failure this
            // function exists to prevent.
            FileOutputStream(tmp).use { out ->
                out.write(bytes)
                out.flush()
                out.fd.sync()
            }
            tmp.renameTo(target)
        } catch (e: Exception) {
            Log.e(TAG, "Failed writing $path", e)
            false
        }
    }

    fun stopEmulation() {
        // A cancelled loop may still be inside a native call; `engineLock` is
        // what keeps the next one out of the engine until it leaves, so there
        // is nothing to join here.
        emulationJob?.cancel()
        emulationJob = null
        audio.stop()
        _isRunning.value = false
        viewModelScope.launch(Dispatchers.IO) { flushSaveNow() }
    }

    fun togglePause() {
        if (_isRunning.value) {
            pausedByBackground = false
            stopEmulation()
        } else if (romLoaded) {
            startEmulation()
        }
    }

    /** Called when the app leaves the foreground. Does not run the loop behind a lock screen. */
    fun onAppBackgrounded() {
        if (_isRunning.value) {
            pausedByBackground = true
            stopEmulation()
        }
    }

    /** Called when the app returns to the foreground. Only resumes what backgrounding paused. */
    fun onAppForegrounded() {
        if (pausedByBackground && romLoaded) {
            pausedByBackground = false
            startEmulation()
        }
    }


    /**
     * Hold to walk backwards through the rewind ring; release to resume.
     *
     * Rewinding while fast-forwarding makes no sense, so this cancels it.
     */
    fun setRewinding(active: Boolean) {
        if (active) {
            _fastForward.value = false
        }
        _isRewinding.value = active
    }

    private fun stateFile(slot: Int): File = File(statesDir, "${romStateKey}_slot$slot.state")


    /**
     * What each save-state slot holds, for the slot list.
     *
     * Slot 0 is the quick-save the toolbar writes; the rest exist only
     * through the slot list.
     */
    fun stateSlots(): List<StateSlot> = (0 until SLOT_COUNT).map { slot ->
        val file = stateFile(slot)
        StateSlot(slot, file.isFile && file.length() > 0, file.lastModified())
    }

    /**
     * Write the current frame to the device's Pictures/GeeBeeAyy folder as a
     * PNG, at native 240x160.
     *
     * Deliberately not upscaled: a screenshot is a record of what the emulator
     * produced, and the scaling is a display choice the viewer can make again.
     */
    fun takeScreenshot() {
        val frame = _frameBuffer.value
        if (frame == null || frame.size < GbaEngine.FRAME_BUFFER_SIZE) {
            _stateMessage.value = "No frame to capture yet"
            return
        }
        val name = "${romStateKey ?: "GeeBeeAyy"}_${System.currentTimeMillis()}.png"
        viewModelScope.launch(Dispatchers.IO) {
            val w = GbaEngine.SCREEN_WIDTH
            val h = GbaEngine.SCREEN_HEIGHT
            val pixels = IntArray(w * h)
            for (i in pixels.indices) {
                val o = i * 3
                pixels[i] = (0xFF shl 24) or
                    ((frame[o].toInt() and 0xFF) shl 16) or
                    ((frame[o + 1].toInt() and 0xFF) shl 8) or
                    (frame[o + 2].toInt() and 0xFF)
            }
            val bitmap = android.graphics.Bitmap.createBitmap(
                pixels, w, h, android.graphics.Bitmap.Config.ARGB_8888,
            )
            val message = try {
                val dir = File(
                    android.os.Environment.getExternalStoragePublicDirectory(
                        android.os.Environment.DIRECTORY_PICTURES,
                    ),
                    "GeeBeeAyy",
                ).apply { mkdirs() }
                val out = File(dir, name)
                out.outputStream().use { stream ->
                    bitmap.compress(android.graphics.Bitmap.CompressFormat.PNG, 100, stream)
                }
                "Screenshot saved to Pictures/GeeBeeAyy"
            } catch (e: Exception) {
                Log.e(TAG, "Screenshot failed", e)
                "Could not save the screenshot: ${e.message}"
            } finally {
                bitmap.recycle()
            }
            _stateMessage.value = message
        }
    }

    /**
     * Request a save-state capture into [slot] (0-9). Deferred to the
     * emulation loop thread while it is running, since the engine is a raw
     * pointer and only one thread may touch it at a time; called directly
     * when nothing else is running.
     */
    fun saveState(slot: Int) {
        if (!romLoaded || slot !in 0 until SLOT_COUNT) return
        if (_isRunning.value) {
            pendingStateCommand = StateCommand.Save(slot)
        } else {
            // Under the lock, not merely off the loop: `_isRunning` flips from
            // the UI thread, so resuming a few ms after tapping save would
            // otherwise put `startEmulation()`'s loop and this one-shot inside
            // the same raw handle at once.
            viewModelScope.launch(Dispatchers.Default) {
                engineLock.withLock { performSaveState(slot) }
            }
        }
    }

    /** Runs on the engine thread (loop or the one-shot fallback above). */
    private fun performSaveState(slot: Int) {
        val bytes = engine.readState()
        if (bytes.isEmpty()) {
            _stateMessage.value = "Failed to capture save state"
            return
        }
        viewModelScope.launch(Dispatchers.IO) {
            if (tryAtomicWrite(stateFile(slot).absolutePath, bytes)) {
                _stateMessage.value = "Saved to ${slotName(slot)}"
            } else {
                _stateMessage.value = "Failed to write ${slotName(slot)}"
            }
        }
    }

    /**
     * Request restoring [slot] (0-9). The file read happens on
     * Dispatchers.IO; applying the bytes to the machine is handed to the
     * engine thread the same way [saveState] is.
     */
    fun loadState(slot: Int) {
        if (!romLoaded || slot !in 0 until SLOT_COUNT) return
        viewModelScope.launch(Dispatchers.IO) {
            val file = stateFile(slot)
            if (!file.exists()) {
                _stateMessage.value = "Nothing in ${slotName(slot)}"
                return@launch
            }
            val bytes = try {
                file.readBytes()
            } catch (e: Exception) {
                Log.e(TAG, "Failed reading save state slot $slot", e)
                _stateMessage.value = "Could not read ${slotName(slot)}"
                return@launch
            }
            if (_isRunning.value) {
                pendingStateCommand = StateCommand.LoadBytes(bytes)
            } else {
                // Same reasoning as saveState(): the lock, not just the
                // dispatcher, is what keeps a resumed loop out of the engine.
                withContext(Dispatchers.Default) {
                    engineLock.withLock { applyStateBytes(bytes) }
                }
            }
        }
    }

    /** Runs on the engine thread (loop or the one-shot fallback above). */
    private fun applyStateBytes(bytes: ByteArray) {
        if (engine.writeState(bytes)) {
            _stateMessage.value = "Save state loaded"
        } else {
            // `SaveState::restore` snapshots the machine and rolls back when
            // the parse fails, so a rejected load (wrong version, truncated
            // or corrupt file) is a no-op on a machine that is still valid.
            // This used to call stopEmulation() and demand a ROM reload,
            // which killed a perfectly healthy session over a bad file.
            Log.e(TAG, "Save state load rejected; the machine is unchanged")
            _stateMessage.value = "Save state incompatible or corrupt"
        }
    }

    fun clearStateMessage() {
        _stateMessage.value = null
    }

    override fun onCleared() {
        super.onCleared()
        // `engine.destroy()` below frees the Box<GbaHandle>. Cancelling is a
        // request, not a stop, so without waiting here the handle could be
        // freed while the loop is still inside a native call. Blocking the
        // main thread for at most one frame is the cost of not doing that;
        // if the wait times out, leak the handle rather than free it live.
        emulationJob?.cancel()
        emulationJob = null
        audio.stop()
        // Acquiring the lock is the proof the loop has left native code -
        // stronger than joining the job, which says nothing about a coroutine
        // parked inside `engine.runFrame()`.
        val stopped = runBlocking {
            withTimeoutOrNull(SHUTDOWN_JOIN_TIMEOUT_MS) { engineLock.lock(); true }
        } == true
        // viewModelScope is already cancelled by the time onCleared() runs -
        // ViewModel.clear() closes it before calling this - so a launched
        // coroutine here would silently never run. flushSaveNow() has no
        // suspension points, so it is called directly instead.
        flushSaveNow()
        if (stopped) {
            // `audio.release()` belongs here too, not before the check: a loop
            // that has not stopped in a second is almost certainly blocked in
            // `AudioTrack.write`, and releasing the track under a live write
            // is a native crash. Leaking both beats a SIGSEGV on the way out.
            audio.release()
            engine.destroy()
        } else {
            Log.w(TAG, "Emulation thread still running; leaking the core handle and the audio track rather than freeing them under a live call")
        }
    }
}
