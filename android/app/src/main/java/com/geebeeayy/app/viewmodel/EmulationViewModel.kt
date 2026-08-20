package com.geebeeayyayy.app.viewmodel

import android.app.Application
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import com.geebeeayyayy.app.engine.GbaEngine
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch

/**
 * ViewModel for the emulation screen.
 * Manages the GbaEngine lifecycle and frame rendering loop.
 */
class EmulationViewModel(application: Application) : AndroidViewModel(application) {

    private val engine = GbaEngine()

    // Frame buffer for rendering
    private val _frameBuffer = MutableStateFlow<ByteArray?>(null)
    val frameBuffer: StateFlow<ByteArray?> = _frameBuffer

    // Emulation state
    private val _isRunning = MutableStateFlow(false)
    val isRunning: StateFlow<Boolean> = _isRunning

    private val _isFastForward = MutableStateFlow(false)
    val isFastForward: StateFlow<Boolean> = _isFastForward

    private var emulationJob: Job? = null
    private var romLoaded = false

    init {
        engine.create()
    }

    /**
     * Load a ROM from a byte array.
     * @return true on success.
     */
    fun loadRom(data: ByteArray): Boolean {
        val result = engine.loadRom(data)
        romLoaded = result
        return result
    }

    /**
     * Start the emulation loop.
     * Runs frames at ~60fps and updates the frame buffer.
     */
    fun startEmulation() {
        if (romLoaded && emulationJob == null) {
            _isRunning.value = true
            emulationJob = viewModelScope.launch(Dispatchers.Default) {
                while (isActive) {
                    if (!isFastForward.value) {
                        // Normal speed: ~60fps = 16.67ms per frame
                        engine.runFrame()
                        _frameBuffer.value = engine.getFrameBuffer()
                        delay(16)
                    } else {
                        // Fast forward: run multiple frames
                        engine.runFrames(4)
                        _frameBuffer.value = engine.getFrameBuffer()
                        delay(4)
                    }
                }
            }
        }
    }

    /**
     * Stop the emulation loop.
     */
    fun stopEmulation() {
        emulationJob?.cancel()
        emulationJob = null
        _isRunning.value = false
    }

    /**
     * Toggle pause state.
     */
    fun togglePause() {
        if (_isRunning.value) {
            stopEmulation()
        } else {
            startEmulation()
        }
    }

    /**
     * Toggle fast forward mode.
     */
    fun toggleFastForward() {
        _isFastForward.value = !_isFastForward.value
    }

    /**
     * Save state to slot.
     */
    fun saveState(slot: Int = 0) {
        // TODO: Implement save state persistence
        val stateHandle = engine.saveStateCreate()
        // In a real app, save to file: saveStateToFile(stateHandle, slot)
    }

    /**
     * Load state from slot.
     */
    fun loadState(slot: Int = 0) {
        // TODO: Implement load state persistence
        // val stateHandle = loadStateFromFile(slot)
        // if (stateHandle != null) engine.loadState(stateHandle)
    }

    /**
     * Get the current engine instance for direct access.
     */
    fun getEngine(): GbaEngine = engine

    override fun onCleared() {
        super.onCleared()
        stopEmulation()
        engine.destroy()
    }
}
