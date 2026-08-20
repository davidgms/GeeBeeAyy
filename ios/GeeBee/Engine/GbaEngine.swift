import Foundation

/// Swift wrapper for the C FFI bridge to the GeeBee-A Rust core.
///
/// Usage:
///   let engine = GbaEngine()
///   engine.create()
///   engine.loadRom(romData)
///   engine.runFrame()
///   let pixels = engine.getFrameBuffer()
class GbaEngine {
    private var handle: UnsafeMutableRawPointer?
    private var frameBuffer = [UInt8](repeating: 0, count: 240 * 160 * 3)

    static let screenWidth = 240
    static let screenHeight = 160
    static let bytesPerPixel = 3
    static let frameBufferSize = screenWidth * screenHeight * bytesPerPixel

    var isLoaded: Bool { handle != nil }

    /// Create a new emulator instance.
    func create() {
        handle = geebee_create()
    }

    /// Destroy the emulator instance.
    func destroy() {
        guard let h = handle else { return }
        geebee_destroy(h)
        handle = nil
    }

    /// Load a ROM from data.
    /// - Returns: true on success.
    func loadRom(_ data: Data) -> Bool {
        guard let h = handle else { return false }
        return data.withUnsafeBytes { ptr in
            guard let baseAddress = ptr.baseAddress else { return false }
            return geebee_load_rom(h, baseAddress.assumingMemoryBound(to: UInt8.self), data.count) == 0
        }
    }

    /// Run a single frame of emulation.
    func runFrame() {
        guard let h = handle else { return }
        geebee_run_frame(h)
        frameBuffer.withUnsafeMutableBytes { ptr in
            guard let baseAddress = ptr.baseAddress else { return }
            geebee_frame_buffer_copy(h, baseAddress.assumingMemoryBound(to: UInt8.self))
        }
    }

    /// Run multiple frames (fast-forward).
    func runFrames(_ count: UInt32) {
        guard let h = handle else { return }
        geebee_run_frames(h, count)
        frameBuffer.withUnsafeMutableBytes { ptr in
            guard let baseAddress = ptr.baseAddress else { return }
            geebee_frame_buffer_copy(h, baseAddress.assumingMemoryBound(to: UInt8.self))
        }
    }

    /// Get the current frame buffer as Data (240x160 RGB888).
    func getFrameBuffer() -> Data {
        return Data(frameBuffer)
    }

    /// Get audio samples (f32 mono).
    func getAudioSamples(maxSamples: Int = 4096) -> [Float] {
        guard let h = handle else { return [] }
        var samples = [Float](repeating: 0, count: maxSamples)
        let count = samples.withUnsafeMutableBufferPointer { ptr in
            geebee_audio_copy(h, ptr.baseAddress, maxSamples)
        }
        return Array(samples.prefix(count))
    }

    /// Create a save state.
    func saveStateCreate() -> UnsafeMutableRawPointer? {
        guard let h = handle else { return nil }
        return geebee_save_state_create(h)
    }

    /// Restore from a save state.
    func loadState(_ stateHandle: UnsafeMutableRawPointer) -> Bool {
        guard let h = handle else { return false }
        return geebee_load_state(h, stateHandle) == 0
    }

    /// Destroy a save state.
    func saveStateDestroy(_ stateHandle: UnsafeMutableRawPointer) {
        geebee_save_state_destroy(stateHandle)
    }
}
