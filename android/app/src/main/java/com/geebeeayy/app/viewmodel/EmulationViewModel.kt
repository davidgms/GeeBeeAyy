package com.geebeeayy.app.viewmodel

import android.app.Application
import android.hardware.display.DisplayManager
import android.util.Log
import android.view.Display
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import com.geebeeayy.app.engine.AudioOutput
import com.geebeeayy.app.engine.GbaEngine
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
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

        /** Ten slots per game, per the ROADMAP. */
        const val STATE_SLOT_COUNT = 10

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

    private val _isFastForward = MutableStateFlow(false)
    val isFastForward: StateFlow<Boolean> = _isFastForward

    private val _isLoading = MutableStateFlow(true)
    val isLoading: StateFlow<Boolean> = _isLoading

    private val _errorMessage = MutableStateFlow<String?>(null)
    val errorMessage: StateFlow<String?> = _errorMessage

    /** Transient feedback for save/load-state actions (slot N saved, load rejected, ...). */
    private val _stateMessage = MutableStateFlow<String?>(null)
    val stateMessage: StateFlow<String?> = _stateMessage

    private var emulationJob: Job? = null
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
            emulationJob = viewModelScope.launch(Dispatchers.Default) {
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
                            is StateCommand.LoadBytes -> applyStateBytes(cmd.bytes)
                        }
                    }

                    // A rejected load stops emulation from inside the block
                    // above. Job.cancel() flips isActive synchronously but
                    // this while loop only rechecks it at the top, so without
                    // this guard the loop would run one more frame on a
                    // machine just declared unreliable, and write audio to an
                    // AudioTrack that stopEmulation() just stopped.
                    if (!isActive) continue

                    val fastForward = isFastForward.value
                    if (fastForward) {
                        engine.runFrames(4)
                    } else {
                        engine.runFrame()
                    }
                    _frameBuffer.value = engine.getFrameBuffer().copyOf()

                    checkSaveDirty()

                    val count = engine.readAudio(audioSamples)
                    // Fast forward would be held back to real time by a blocking
                    // audio write, so drop the samples and pace off the timer.
                    val paced = !fastForward && audio.write(audioSamples, count)
                    if (!paced) {
                        delay(if (fastForward) (frameIntervalMs / 4).coerceAtLeast(1L) else frameIntervalMs)
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

    fun toggleFastForward() {
        _isFastForward.value = !_isFastForward.value
    }

    private fun stateFile(slot: Int): File = File(statesDir, "${romStateKey}_slot$slot.state")

    /**
     * Request a save-state capture into [slot] (0-9). Deferred to the
     * emulation loop thread while it is running, since the engine is a raw
     * pointer and only one thread may touch it at a time; called directly
     * when nothing else is running.
     */
    fun saveState(slot: Int) {
        if (!romLoaded || slot !in 0 until STATE_SLOT_COUNT) return
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
        if (!romLoaded || slot !in 0 until STATE_SLOT_COUNT) return
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
            // The core's restore() writes into the live machine as it parses,
            // so a rejected load (wrong version, truncated or corrupt file)
            // can leave a hybrid of the old and new states, not the original
            // untouched one. Stop rather than let the player keep going on a
            // machine that is neither.
            Log.e(TAG, "Save state load rejected; stopping emulation, ROM reload required")
            _stateMessage.value = "Save state incompatible or corrupt - reload the ROM"
            stopEmulation()
        }
    }

    fun clearStateMessage() {
        _stateMessage.value = null
    }

    override fun onCleared() {
        super.onCleared()
        emulationJob?.cancel()
        emulationJob = null
        audio.stop()
        // viewModelScope is already cancelled by the time onCleared() runs -
        // ViewModel.clear() closes it before calling this - so a launched
        // coroutine here would silently never run. flushSaveNow() has no
        // suspension points, so it is called directly instead.
        flushSaveNow()
        audio.release()
        engine.destroy()
    }
}
