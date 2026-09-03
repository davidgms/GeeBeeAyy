package com.geebeeayy.app.viewmodel

import android.app.Application
import android.hardware.display.DisplayManager
import android.util.Log
import android.view.Display
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import com.geebeeayy.app.data.LastPlayed
import com.geebeeayy.app.data.StateSlot
import com.geebeeayy.app.engine.AudioOutput
import com.geebeeayy.app.engine.GbaEngine
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.cancelAndJoin
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeoutOrNull
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.isActive
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
         * Rewind depth and cadence.
         *
         * A snapshot is around 500 KB, so 20 of them is roughly 10 MB - the
         * whole budget for this feature. At one every 30 frames that buys
         * about ten seconds of history, which is the useful range for undoing
         * a mistake without turning the emulator into a memory hog.
         */
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
         * code before giving up and leaking the core handle. One frame is
         * 16 ms; this is generous enough to cover a slow save-state write.
         */
        const val SHUTDOWN_JOIN_TIMEOUT_MS = 1_000L

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
         * A game saving touches thousands of bytes across many CPU cycles;
         * waiting this long after the last dirty flag before writing avoids a
         * file write per byte.
         */
        private const val SAVE_FLUSH_DEBOUNCE_MS = 2_000L
    }

    private val engine = GbaEngine()
    private val audio = AudioOutput()

    /** Reused across frames; four frames of headroom covers a fast-forward burst. */
    private val audioSamples = FloatArray(AudioOutput.SAMPLES_PER_FRAME * 8)

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

    /** 0 is normal speed; otherwise how many emulated frames run per real
     *  frame tick - 2x, 4x or 8x, cycling in that order back to 0. Powers of
     *  two: they are exact (no rounding in the frame count or the pacing
     *  math below) and each step is still slow enough to follow by eye. */
    private val _fastForwardSpeed = MutableStateFlow(0)
    val fastForwardSpeed: StateFlow<Int> = _fastForwardSpeed

    fun cycleFastForward() {
        _fastForwardSpeed.value = when (_fastForwardSpeed.value) {
            0 -> 2
            2 -> 4
            4 -> 8
            else -> 0
        }
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
     * The loop that was asked to stop but may still be inside a native call.
     * The next `startEmulation()` joins it before touching the engine.
     */
    private var stoppingJob: Job? = null
    private var romLoaded = false

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
                val success = engine.loadRom(data)
                if (success) {
                    romLoaded = true
                    LastPlayed(getApplication()).record(filePath)
                    // A fresh ROM means a fresh history; the ring is allocated
                    // here rather than at create() so a cold handle costs
                    // nothing.
                    engine.rewindConfigure(REWIND_DEPTH)
                    rewindFrameCounter = 0
                    resolveSavePaths(file, data)
                    loadExistingSave()
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

    /** Title (0xA0, 12 bytes) + game code (0xAC, 4 bytes), sanitized to a safe filename fragment. */
    private fun computeRomStateKey(romData: ByteArray): String {
        fun readAscii(offset: Int, len: Int): String =
            String(romData, offset, len, Charsets.US_ASCII).substringBefore('\u0000').trim()

        val title = readAscii(0xA0, 12)
        val gameCode = readAscii(0xAC, 4)
        val raw = when {
            gameCode.isNotBlank() && title.isNotBlank() -> "${title}_$gameCode"
            gameCode.isNotBlank() -> gameCode
            title.isNotBlank() -> title
            else -> "rom"
        }
        return raw.uppercase().replace(Regex("[^A-Z0-9_]"), "_").ifBlank { "rom" }
    }

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

    fun startEmulation() {
        if (romLoaded && emulationJob == null) {
            _isRunning.value = true
            audio.start()
            audio.resume()
            // `stopEmulation()` only *requests* cancellation, and the loop
            // rechecks `isActive` at the top - so a job parked inside
            // `engine.runFrame()` is still in native code after it returns.
            // Toggling pause twice quickly, or backgrounding and coming
            // straight back, would start this loop alongside the old one and
            // put two threads on the same `Gba` through a raw pointer. Wait
            // the previous one out here, off the main thread.
            val previous = stoppingJob
            stoppingJob = null
            emulationJob = viewModelScope.launch(Dispatchers.Default) {
                previous?.cancelAndJoin()
                while (isActive) {
                    // Applied here so every call into the core happens on this
                    // one thread. See the note on `keyState`.
                    // Held keys, plus anything pressed and released since the
                    // last frame, which `keyState` alone has already forgotten.
                    engine.setKeys(keyState or transientPresses.getAndSet(0))

                    pendingStateCommand?.let { cmd ->
                        pendingStateCommand = null
                        when (cmd) {
                            is StateCommand.Save -> performSaveState(cmd.slot)
                            is StateCommand.LoadBytes -> {
                applyStateBytes(cmd.bytes)
                // The history belongs to the timeline we just left.
                engine.rewindClear()
                rewindFrameCounter = 0
            }
                        }
                    }

                    // A rejected load stops emulation from inside the block
                    // above. Job.cancel() flips isActive synchronously but
                    // this while loop only rechecks it at the top, so without
                    // this guard the loop would run one more frame on a
                    // machine just declared unreliable, and write audio to an
                    // AudioTrack that stopEmulation() just stopped.
                    if (!isActive) continue

                    val speed = fastForwardSpeed.value
                    val rewinding = isRewinding.value

                    if (rewinding) {
                        // Walking back through the ring one snapshot at a
                        // time, paced so REWIND_INTERVAL_FRAMES of play take
                        // REWIND_INTERVAL_FRAMES / REWIND_SPEED to undo.
                        // Reaching the oldest snapshot holds there rather
                        // than resuming play under the player's finger.
                        if (engine.rewindPop()) {
                            _frameBuffer.value = engine.getFrameBuffer().copyOf()
                        }
                        delay(
                            (frameIntervalMs * REWIND_INTERVAL_FRAMES / REWIND_SPEED)
                                .coerceAtLeast(1L)
                        )
                        continue
                    }

                    if (speed > 0) {
                        engine.runFrames(speed)
                    } else {
                        engine.runFrame()
                    }
                    _frameBuffer.value = engine.getFrameBuffer().copyOf()

                    // Snapshot on a cadence, not every frame: a state is about
                    // 500 KB, so REWIND_DEPTH * REWIND_INTERVAL decides both
                    // the memory cost and how far back the player can go.
                    if (++rewindFrameCounter >= REWIND_INTERVAL_FRAMES) {
                        rewindFrameCounter = 0
                        engine.rewindPush()
                    }

                    checkSaveDirty()

                    val count = engine.readAudio(audioSamples)
                    // Fast forward would be held back to real time by a blocking
                    // audio write, so drop the samples and pace off the timer.
                    //
                    // The timer always waits one real frame interval, whether
                    // fast-forwarding or not - dividing it by the speed too,
                    // on top of already running `speed` emulated frames per
                    // iteration, used to compound into a speed the button's
                    // label had no relation to (running 4 frames AND waiting
                    // a quarter of the interval multiplied out to ~16x, not
                    // 4x). `speed` frames landing inside one real interval is
                    // what makes the multiplier exact instead of a guess -
                    // and self-limiting on a device too slow to keep up,
                    // since compute time then eats into the same window
                    // rather than being added on top of it.
                    val paced = speed == 0 && audio.write(audioSamples, count)
                    if (!paced) {
                        delay(frameIntervalMs)
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
        emulationJob?.let {
            it.cancel()
            // Kept so the next `startEmulation()` can join it. Nulling this
            // without a handle to join was what let two loops overlap.
            stoppingJob = it
        }
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
            _fastForwardSpeed.value = 0
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
            // ponytail: brief TOCTOU window if the player resumes emulation in
            // the next few ms while this direct call is in flight - a Mutex
            // around engine access would close it if it's ever observed.
            viewModelScope.launch(Dispatchers.Default) { performSaveState(slot) }
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
                _stateMessage.value = "Saved to slot ${slot + 1}"
            } else {
                _stateMessage.value = "Failed to write save state slot ${slot + 1}"
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
                _stateMessage.value = "No save state in slot ${slot + 1}"
                return@launch
            }
            val bytes = try {
                file.readBytes()
            } catch (e: Exception) {
                Log.e(TAG, "Failed reading save state slot $slot", e)
                _stateMessage.value = "Could not read save state slot ${slot + 1}"
                return@launch
            }
            if (_isRunning.value) {
                pendingStateCommand = StateCommand.LoadBytes(bytes)
            } else {
                // ponytail: see the note in saveState() - same TOCTOU window.
                withContext(Dispatchers.Default) { applyStateBytes(bytes) }
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
        val job = emulationJob
        emulationJob = null
        val stopped = runBlocking {
            withTimeoutOrNull(SHUTDOWN_JOIN_TIMEOUT_MS) {
                job?.cancelAndJoin()
                stoppingJob?.cancelAndJoin()
                true
            }
        } == true
        audio.stop()
        // viewModelScope is already cancelled by the time onCleared() runs -
        // ViewModel.clear() closes it before calling this - so a launched
        // coroutine here would silently never run. flushSaveNow() has no
        // suspension points, so it is called directly instead.
        flushSaveNow()
        audio.release()
        if (stopped) {
            engine.destroy()
        } else {
            Log.w(TAG, "Emulation thread still running; leaking the core handle rather than freeing it under a live call")
        }
    }
}
