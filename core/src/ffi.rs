use crate::{Gba, savestate::SaveState};
use std::ffi::c_void;

// ─── C FFI (used by iOS via bridging header, and by Android via JNI) ────────

/// Opaque handle to Gba instance for FFI consumers.
pub struct GbaHandle {
    inner: Gba,
}

/// Create a new GBA emulator instance.
#[no_mangle]
pub extern "C" fn geebee_create() -> *mut c_void {
    let handle = Box::new(GbaHandle { inner: Gba::new() });
    Box::into_raw(handle) as *mut c_void
}

/// Destroy a GBA emulator instance.
///
/// # Safety
/// `ptr` must have been returned by `geebee_create` and must not be used after this call.
#[no_mangle]
pub unsafe extern "C" fn geebee_destroy(ptr: *mut c_void) {
    if !ptr.is_null() {
        unsafe { drop(Box::from_raw(ptr as *mut GbaHandle)); }
    }
}

/// Load a ROM from a byte buffer.
///
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `ptr` must be a valid handle. `data` must point to `len` readable bytes.
#[no_mangle]
pub unsafe extern "C" fn geebee_load_rom(
    ptr: *mut c_void,
    data: *const u8,
    len: usize,
) -> i32 {
    if ptr.is_null() || data.is_null() {
        return -1;
    }
    let handle = unsafe { &mut *(ptr as *mut GbaHandle) };
    let rom = unsafe { std::slice::from_raw_parts(data, len) };
    match handle.inner.load_rom(rom) {
        Ok(()) => 0,
        Err(_) => -1,
    }
}

/// Run a single frame of emulation.
///
/// # Safety
/// `ptr` must be a valid handle.
#[no_mangle]
pub unsafe extern "C" fn geebee_run_frame(ptr: *mut c_void) {
    if ptr.is_null() {
        return;
    }
    let handle = unsafe { &mut *(ptr as *mut GbaHandle) };
    handle.inner.run_frame();
}

/// Run multiple frames (for fast-forward).
///
/// # Safety
/// `ptr` must be a valid handle.
#[no_mangle]
pub unsafe extern "C" fn geebee_run_frames(ptr: *mut c_void, count: u32) {
    if ptr.is_null() {
        return;
    }
    let handle = unsafe { &mut *(ptr as *mut GbaHandle) };
    handle.inner.run_frames(count);
}

/// Copy the current frame buffer (240x160 RGB888) into `out`.
///
/// `out` must point to at least 240*160*3 = 115200 bytes.
///
/// # Safety
/// `ptr` must be a valid handle. `out` must be writable for 115200 bytes.
#[no_mangle]
pub unsafe extern "C" fn geebee_frame_buffer_copy(ptr: *mut c_void, out: *mut u8) {
    if ptr.is_null() || out.is_null() {
        return;
    }
    let handle = unsafe { &*(ptr as *mut GbaHandle) };
    let fb = handle.inner.frame_buffer();
    unsafe {
        std::ptr::copy_nonoverlapping(fb.as_ptr(), out, 240 * 160 * 3);
    }
}

/// Get a pointer to the internal frame buffer (240x160 RGB888, 115200 bytes).
///
/// The pointer is valid until the next call to `geebee_run_frame`.
///
/// # Safety
/// `ptr` must be a valid handle.
#[no_mangle]
pub unsafe extern "C" fn geebee_frame_buffer_ptr(ptr: *mut c_void) -> *const u8 {
    if ptr.is_null() {
        return std::ptr::null();
    }
    let handle = unsafe { &*(ptr as *mut GbaHandle) };
    handle.inner.frame_buffer().as_ptr()
}

/// Copy audio samples (f32 mono) into `out`.
///
/// Returns the number of samples written.
///
/// # Safety
/// `ptr` must be a valid handle. `out` must be writable for `max_samples` f32 values.
#[no_mangle]
pub unsafe extern "C" fn geebee_audio_copy(
    ptr: *mut c_void,
    out: *mut f32,
    max_samples: usize,
) -> usize {
    if ptr.is_null() || out.is_null() {
        return 0;
    }
    let handle = unsafe { &mut *(ptr as *mut GbaHandle) };
    let samples = handle.inner.apu_samples();
    let count = samples.len().min(max_samples);
    unsafe {
        std::ptr::copy_nonoverlapping(samples.as_ptr(), out, count);
    }
    handle.inner.clear_audio_buffer();
    count
}

/// Create a save state. Returns an opaque pointer.
///
/// # Safety
/// `ptr` must be a valid handle.
#[no_mangle]
pub unsafe extern "C" fn geebee_save_state_create(ptr: *mut c_void) -> *mut c_void {
    if ptr.is_null() {
        return std::ptr::null_mut();
    }
    let handle = unsafe { &*(ptr as *mut GbaHandle) };
    let state = Box::new(handle.inner.save_state());
    Box::into_raw(state) as *mut c_void
}

/// Restore from a save state.
///
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `ptr` must be a valid handle. `state_ptr` must have been returned by
/// `geebee_save_state_create`.
#[no_mangle]
pub unsafe extern "C" fn geebee_load_state(
    ptr: *mut c_void,
    state_ptr: *mut c_void,
) -> i32 {
    if ptr.is_null() || state_ptr.is_null() {
        return -1;
    }
    let handle = unsafe { &mut *(ptr as *mut GbaHandle) };
    let state = unsafe { &*(state_ptr as *mut SaveState) };
    match handle.inner.load_state(state) {
        Ok(()) => 0,
        Err(_) => -1,
    }
}

/// Destroy a save state created by `geebee_save_state_create`.
///
/// # Safety
/// `state_ptr` must have been returned by `geebee_save_state_create`.
#[no_mangle]
pub unsafe extern "C" fn geebee_save_state_destroy(state_ptr: *mut c_void) {
    if !state_ptr.is_null() {
        unsafe { drop(Box::from_raw(state_ptr as *mut SaveState)); }
    }
}

// ─── Android JNI Bindings ───────────────────────────────────────────────────

#[cfg(target_os = "android")]
pub mod android {
    use super::*;
    use jni::JNIEnv;
    use jni::objects::{JClass, JByteArray, JFloatArray};
    use jni::sys::{jint, jlong, jbyte};

    #[no_mangle]
    pub extern "system" fn Java_com_geebee_app_engine_GbaEngine_nativeCreate(
        _env: JNIEnv,
        _class: JClass,
    ) -> jlong {
        let ptr = unsafe { geebee_create() };
        ptr as jlong
    }

    #[no_mangle]
    pub extern "system" fn Java_com_geebee_app_engine_GbaEngine_nativeDestroy(
        _env: JNIEnv,
        _class: JClass,
        handle: jlong,
    ) {
        unsafe { geebee_destroy(handle as *mut c_void); }
    }

    #[no_mangle]
    pub extern "system" fn Java_com_geebee_app_engine_GbaEngine_nativeLoadRom(
        env: JNIEnv,
        _class: JClass,
        handle: jlong,
        data: JByteArray,
    ) -> jint {
        let len = env.get_array_length(&data).unwrap_or(0) as usize;
        let buf = vec![0i8; len];
        let buf_ptr = env.get_byte_array_region(&data, 0, &mut buf.clone()).ok();
        // Simplified: just return error if JNI fails
        match buf_ptr {
            Ok(()) => {
                let rom_slice = unsafe { std::slice::from_raw_parts(buf.as_ptr() as *const u8, len) };
                unsafe { geebee_load_rom(handle as *mut c_void, rom_slice.as_ptr(), len) }
            }
            Err(_) => -1,
        }
    }

    #[no_mangle]
    pub extern "system" fn Java_com_geebee_app_engine_GbaEngine_nativeRunFrame(
        _env: JNIEnv,
        _class: JClass,
        handle: jlong,
    ) {
        unsafe { geebee_run_frame(handle as *mut c_void); }
    }

    #[no_mangle]
    pub extern "system" fn Java_com_geebee_app_engine_GbaEngine_nativeRunFrames(
        _env: JNIEnv,
        _class: JClass,
        handle: jlong,
        count: jint,
    ) {
        unsafe { geebee_run_frames(handle as *mut c_void, count as u32); }
    }

    #[no_mangle]
    pub extern "system" fn Java_com_geebee_app_engine_GbaEngine_nativeFrameBufferCopy(
        env: JNIEnv,
        _class: JClass,
        handle: jlong,
        out: JByteArray,
    ) {
        let len = env.get_array_length(&out).unwrap_or(0) as usize;
        if len >= 240 * 160 * 3 {
            let mut buf = vec![0u8; 240 * 160 * 3];
            unsafe { geebee_frame_buffer_copy(handle as *mut c_void, buf.as_mut_ptr()); }
            let _ = env.set_byte_array_region(&out, 0, unsafe {
                std::slice::from_raw_parts(buf.as_ptr() as *const i8, buf.len())
            });
        }
    }

    #[no_mangle]
    pub extern "system" fn Java_com_geebee_app_engine_GbaEngine_nativeAudioCopy(
        env: JNIEnv,
        _class: JClass,
        handle: jlong,
        out: JFloatArray,
        max_samples: jint,
    ) -> jint {
        let mut buf = vec![0.0f32; max_samples as usize];
        let count = unsafe {
            geebee_audio_copy(
                handle as *mut c_void,
                buf.as_mut_ptr(),
                max_samples as usize,
            )
        };
        let _ = env.set_float_array_region(&out, 0, &buf[..count]);
        count as jint
    }

    #[no_mangle]
    pub extern "system" fn Java_com_geebee_app_engine_GbaEngine_nativeSaveStateCreate(
        _env: JNIEnv,
        _class: JClass,
        handle: jlong,
    ) -> jlong {
        let ptr = unsafe { geebee_save_state_create(handle as *mut c_void) };
        ptr as jlong
    }

    #[no_mangle]
    pub extern "system" fn Java_com_geebee_app_engine_GbaEngine_nativeLoadState(
        _env: JNIEnv,
        _class: JClass,
        handle: jlong,
        state_handle: jlong,
    ) -> jint {
        unsafe {
            geebee_load_state(
                handle as *mut c_void,
                state_handle as *mut c_void,
            )
        }
    }

    #[no_mangle]
    pub extern "system" fn Java_com_geebee_app_engine_GbaEngine_nativeSaveStateDestroy(
        _env: JNIEnv,
        _class: JClass,
        state_handle: jlong,
    ) {
        unsafe { geebee_save_state_destroy(state_handle as *mut c_void); }
    }
}
