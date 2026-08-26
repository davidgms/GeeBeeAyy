package com.geebeeayy.app.viewmodel

import android.app.Application
import android.util.Log
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
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

    init {
        engine.create()
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
            emulationJob = viewModelScope.launch(Dispatchers.Default) {
                while (isActive) {
                    if (!isFastForward.value) {
                        engine.runFrame()
                        _frameBuffer.value = engine.getFrameBuffer().copyOf()
                        delay(16)
                    } else {
                        engine.runFrames(4)
                        _frameBuffer.value = engine.getFrameBuffer().copyOf()
                        delay(4)
                    }
                }
            }
        }
    }

    fun stopEmulation() {
        emulationJob?.cancel()
        emulationJob = null
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
        engine.destroy()
    }
}
