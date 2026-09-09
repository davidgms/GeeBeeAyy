/// Bridging header for GeeBeeAyy Rust core FFI
///
/// This header exposes the C FFI functions from libgeebeeayy_core.a
/// for use in Swift code.

#ifndef GeeBeeAyy_Bridging_Header_h
#define GeeBeeAyy_Bridging_Header_h

#include <stdint.h>
#include <stdbool.h>

/// Create a new GBA emulator instance.
void *geebeeayy_create(void);

/// Destroy a GBA emulator instance.
void geebeeayy_destroy(void *ptr);

/// Load a ROM from a byte buffer. Returns 0 on success, -1 on error.
int geebeeayy_load_rom(void *ptr, const uint8_t *data, size_t len);

/// Run a single frame of emulation.
void geebeeayy_run_frame(void *ptr);

/// Run multiple frames (for fast-forward).
void geebeeayy_run_frames(void *ptr, uint32_t count);

/// Copy the current frame buffer (240x160 RGB888) into out.
void geebeeayy_frame_buffer_copy(void *ptr, uint8_t *out);

/// Get a pointer to the internal frame buffer.
const uint8_t *geebeeayy_frame_buffer_ptr(void *ptr);

/// Copy audio samples (f32 mono) into out. Returns number of samples written.
size_t geebeeayy_audio_copy(void *ptr, float *out, size_t max_samples);

/// Average each frame with the one before it (1) or not (0), the way the
/// GBA's LCD did. Off by default.
void geebeeayy_set_interframe_blend(void *ptr, int on);

/// Create a save state. Returns an opaque pointer.
void *geebeeayy_save_state_create(void *ptr);

/// Restore from a save state. Returns 0 on success, -1 on error.
int geebeeayy_load_state(void *ptr, void *state_ptr);

/// Destroy a save state.
void geebeeayy_save_state_destroy(void *state_ptr);

#endif /* GeeBeeAyy_Bridging_Header_h */
