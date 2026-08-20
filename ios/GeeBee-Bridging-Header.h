/// Bridging header for GeeBee-A Rust core FFI
///
/// This header exposes the C FFI functions from libgeebee_core.a
/// for use in Swift code.

#ifndef GeeBee_Bridging_Header_h
#define GeeBee_Bridging_Header_h

#include <stdint.h>
#include <stdbool.h>

/// Create a new GBA emulator instance.
void *geebee_create(void);

/// Destroy a GBA emulator instance.
void geebee_destroy(void *ptr);

/// Load a ROM from a byte buffer. Returns 0 on success, -1 on error.
int geebee_load_rom(void *ptr, const uint8_t *data, size_t len);

/// Run a single frame of emulation.
void geebee_run_frame(void *ptr);

/// Run multiple frames (for fast-forward).
void geebee_run_frames(void *ptr, uint32_t count);

/// Copy the current frame buffer (240x160 RGB888) into out.
void geebee_frame_buffer_copy(void *ptr, uint8_t *out);

/// Get a pointer to the internal frame buffer.
const uint8_t *geebee_frame_buffer_ptr(void *ptr);

/// Copy audio samples (f32 mono) into out. Returns number of samples written.
size_t geebee_audio_copy(void *ptr, float *out, size_t max_samples);

/// Create a save state. Returns an opaque pointer.
void *geebee_save_state_create(void *ptr);

/// Restore from a save state. Returns 0 on success, -1 on error.
int geebee_load_state(void *ptr, void *state_ptr);

/// Destroy a save state.
void geebee_save_state_destroy(void *state_ptr);

#endif /* GeeBee_Bridging_Header_h */
