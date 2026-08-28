package com.geebeeayy.app.viewmodel

import android.app.Application
import android.util.Log
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
import java.io.File

class EmulationViewModel(application: Application) : AndroidViewModel(application) {

    companion object {
        private const val TAG = "GeeBeeAyy/VM"
    }

    private val engine = GbaEngine()
    private val audio = AudioOutput()

    /** Reused across frames; four frames of headroom covers a fast-forward burst. */
    private val audioSamples = FloatArray(AudioOutput.SAMPLES_PER_FRAME * 8)

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

    private var emulationJob: Job? = null
    private var romLoaded = false

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

    fun startEmulation() {
        if (romLoaded && emulationJob == null) {
            _isRunning.value = true
            audio.start()
            audio.resume()
            emulationJob = viewModelScope.launch(Dispatchers.Default) {
                while (isActive) {
                    // Applied here so every call into the core happens on this
                    // one thread. See the note on `keyState`.
                    engine.setKeys(keyState)
                    val fastForward = isFastForward.value
                    if (fastForward) {
                        engine.runFrames(4)
                    } else {
                        engine.runFrame()
                    }
                    _frameBuffer.value = engine.getFrameBuffer().copyOf()

                    val count = engine.readAudio(audioSamples)
                    // Fast forward would be held back to real time by a blocking
                    // audio write, so drop the samples and pace off the timer.
                    val paced = !fastForward && audio.write(audioSamples, count)
                    if (!paced) {
                        delay(if (fastForward) 4 else 16)
                    }
                }
            }
        }
    }

    fun stopEmulation() {
        emulationJob?.cancel()
        emulationJob = null
        audio.stop()
        _isRunning.value = false
    }

    fun togglePause() {
        if (_isRunning.value) {
            stopEmulation()
        } else if (romLoaded) {
            startEmulation()
        }
    }

    fun toggleFastForward() {
        _isFastForward.value = !_isFastForward.value
    }

    fun saveState(slot: Int = 0) {
        engine.saveStateCreate()
    }

    fun loadState(slot: Int = 0) { }

    override fun onCleared() {
        super.onCleared()
        stopEmulation()
        audio.release()
        engine.destroy()
    }
}
