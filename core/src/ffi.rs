use crate::{Gba, savestate::SaveState};
use std::ffi::c_void;

// ─── C FFI (used by iOS via bridging header, and by Android via JNI) ────────

/// Opaque handle to Gba instance for FFI consumers.
pub struct GbaHandle {
    inner: Gba,
}

/// Create a new GBA emulator instance.
#[no_mangle]
pub extern "C" fn geebeeayy_create() -> *mut c_void {
    #[cfg(target_os = "android")]
    {
        let _ = android_logger::init_once(
            android_logger::Config::default()
                .with_max_level(log::LevelFilter::Warn)
                .with_tag("GeeBeeAyy"),
        );
    }
    eprintln!("[GeeBeeAyy] === Creating new GBA instance ===");
    let handle = Box::new(GbaHandle { inner: Gba::new() });
    eprintln!("[GeeBeeAyy] Created, handle={:p}", handle.as_ref());
    Box::into_raw(handle) as *mut c_void
}

/// Destroy a GBA emulator instance.
///
/// # Safety
/// `ptr` must have been returned by `geebeeayy_create` and must not be used after this call.
#[no_mangle]
pub unsafe extern "C" fn geebeeayy_destroy(ptr: *mut c_void) {
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
pub unsafe extern "C" fn geebeeayy_load_rom(
    ptr: *mut c_void,
    data: *const u8,
    len: usize,
) -> i32 {
    if ptr.is_null() || data.is_null() {
        return -1;
    }
    let handle = unsafe { &mut *(ptr as *mut GbaHandle) };
    let rom = unsafe { std::slice::from_raw_parts(data, len) };
    eprintln!("[GeeBeeAyy] load_rom: {} bytes, first4={:02X}{:02X}{:02X}{:02X}", len, rom[0], rom[1], rom[2], rom[3]);
    match handle.inner.load_rom(rom) {
        Ok(()) => {
            eprintln!("[GeeBeeAyy] load_rom: OK, CPU PC={:08X} CPSR={:08X}", handle.inner.cpu.registers[15], handle.inner.cpu.cpsr);
            0
        }
        Err(e) => {
            eprintln!("[GeeBeeAyy] load_rom: FAILED - {:?}", e);
            -1
        }
    }
}

/// Run a single frame of emulation.
///
/// # Safety
/// `ptr` must be a valid handle.
#[no_mangle]
pub unsafe extern "C" fn geebeeayy_run_frame(ptr: *mut c_void) {
    if ptr.is_null() {
        return;
    }
    let handle = unsafe { &mut *(ptr as *mut GbaHandle) };
    handle.inner.run_frame_counter += 1;
    let fc = handle.inner.run_frame_counter;
    if fc <= 5 || fc % 60 == 0 {
        eprintln!(
            "[GeeBeeAyy] frame={} PC={:08X} CPSR={:08X} halted={} io_halt={} IME={} IE={:04X} IF={:04X} scanline={} cycles={}",
            fc,
            handle.inner.cpu.registers[15],
            handle.inner.cpu.cpsr,
            handle.inner.cpu.halted,
            handle.inner.bus.io.halt,
            handle.inner.bus.io.ime,
            handle.inner.bus.io.ie,
            handle.inner.bus.io.if_,
            handle.inner.ppu.scanline,
            handle.inner.cycles,
        );
    }
    handle.inner.run_frame();
    if fc <= 5 || fc % 60 == 0 {
        let fb = handle.inner.frame_buffer();
        let mut non_zero = 0u32;
        for chunk in fb.chunks(3) {
            if chunk[0] != 0 || chunk[1] != 0 || chunk[2] != 0 { non_zero += 1; }
        }
        eprintln!(
            "[GeeBeeAyy] frame={} AFTER: PC={:08X} CPSR={:08X} halted={} IME={} IE={:04X} IF={:04X} non_zero_px={}",
            fc,
            handle.inner.cpu.registers[15],
            handle.inner.cpu.cpsr,
            handle.inner.cpu.halted,
            handle.inner.bus.io.ime,
            handle.inner.bus.io.ie,
            handle.inner.bus.io.if_,
            non_zero,
        );
    }
}

/// Run multiple frames (for fast-forward).
///
/// # Safety
/// `ptr` must be a valid handle.
#[no_mangle]
pub unsafe extern "C" fn geebeeayy_run_frames(ptr: *mut c_void, count: u32) {
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
pub unsafe extern "C" fn geebeeayy_frame_buffer_copy(ptr: *mut c_void, out: *mut u8) {
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
/// The pointer is valid until the next call to `geebeeayy_run_frame`.
///
/// # Safety
/// `ptr` must be a valid handle.
#[no_mangle]
pub unsafe extern "C" fn geebeeayy_frame_buffer_ptr(ptr: *mut c_void) -> *const u8 {
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
pub unsafe extern "C" fn geebeeayy_audio_copy(
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
pub unsafe extern "C" fn geebeeayy_save_state_create(ptr: *mut c_void) -> *mut c_void {
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
/// `geebeeayy_save_state_create`.
#[no_mangle]
pub unsafe extern "C" fn geebeeayy_load_state(
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

/// Destroy a save state created by `geebeeayy_save_state_create`.
///
/// # Safety
/// `state_ptr` must have been returned by `geebeeayy_save_state_create`.
#[no_mangle]
pub unsafe extern "C" fn geebeeayy_save_state_destroy(state_ptr: *mut c_void) {
    if !state_ptr.is_null() {
        unsafe { drop(Box::from_raw(state_ptr as *mut SaveState)); }
    }
}

// ─── Android JNI Bindings ───────────────────────────────────────────────────

#[cfg(target_os = "android")]
pub mod android {
    use super::*;
    use jni::JNIEnv;
    use jni::objects::{JClass, JByteArray, JFloatArray, ReleaseMode};
    use jni::sys::{jint, jlong};

    #[no_mangle]
    pub extern "system" fn Java_com_geebeeayy_app_engine_GbaEngine_nativeCreate(
        _env: JNIEnv,
        _class: JClass,
    ) -> jlong {
        let ptr = geebeeayy_create();
        ptr as jlong
    }

    #[no_mangle]
    pub extern "system" fn Java_com_geebeeayy_app_engine_GbaEngine_nativeDestroy(
        _env: JNIEnv,
        _class: JClass,
        handle: jlong,
    ) {
        if handle != 0 {
            unsafe { geebeeayy_destroy(handle as *mut c_void); }
        }
    }

    #[no_mangle]
    pub extern "system" fn Java_com_geebeeayy_app_engine_GbaEngine_nativeLoadRom(
        mut env: JNIEnv,
        _class: JClass,
        handle: jlong,
        data: JByteArray,
    ) -> jint {
        if handle == 0 {
            return -1;
        }
        let bytes = unsafe {
            match env.get_array_elements(&data, ReleaseMode::CopyBack) {
                Ok(b) => b,
                Err(_) => return -1,
            }
        };
        let len = bytes.len();
        let ptr = bytes.as_ptr() as *const u8;
        unsafe { geebeeayy_load_rom(handle as *mut c_void, ptr, len) }
    }

    #[no_mangle]
    pub extern "system" fn Java_com_geebeeayy_app_engine_GbaEngine_nativeRunFrame(
        _env: JNIEnv,
        _class: JClass,
        handle: jlong,
    ) {
        if handle != 0 {
            unsafe { geebeeayy_run_frame(handle as *mut c_void); }
        }
    }

    #[no_mangle]
    pub extern "system" fn Java_com_geebeeayy_app_engine_GbaEngine_nativeRunFrames(
        _env: JNIEnv,
        _class: JClass,
        handle: jlong,
        count: jint,
    ) {
        if handle != 0 {
            unsafe { geebeeayy_run_frames(handle as *mut c_void, count as u32); }
        }
    }

    #[no_mangle]
    pub extern "system" fn Java_com_geebeeayy_app_engine_GbaEngine_nativeFrameBufferCopy(
        env: JNIEnv,
        _class: JClass,
        handle: jlong,
        out: JByteArray,
    ) {
        if handle == 0 {
            return;
        }
        let mut buf = vec![0u8; 240 * 160 * 3];
        unsafe { geebeeayy_frame_buffer_copy(handle as *mut c_void, buf.as_mut_ptr()); }
        let signed: Vec<i8> = buf.iter().map(|&b| b as i8).collect();
        let _ = env.set_byte_array_region(&out, 0, &signed);
    }

    #[no_mangle]
    pub extern "system" fn Java_com_geebeeayy_app_engine_GbaEngine_nativeAudioCopy(
        env: JNIEnv,
        _class: JClass,
        handle: jlong,
        out: JFloatArray,
        max_samples: jint,
    ) -> jint {
        if handle == 0 {
            return 0;
        }
        let mut buf = vec![0.0f32; max_samples as usize];
        let count = unsafe {
            geebeeayy_audio_copy(
                handle as *mut c_void,
                buf.as_mut_ptr(),
                max_samples as usize,
            )
        };
        let _ = env.set_float_array_region(&out, 0, &buf[..count]);
        count as jint
    }

    #[no_mangle]
    pub extern "system" fn Java_com_geebeeayy_app_engine_GbaEngine_nativeSaveStateCreate(
        _env: JNIEnv,
        _class: JClass,
        handle: jlong,
    ) -> jlong {
        if handle == 0 {
            return 0;
        }
        let ptr = unsafe { geebeeayy_save_state_create(handle as *mut c_void) };
        ptr as jlong
    }

    #[no_mangle]
    pub extern "system" fn Java_com_geebeeayy_app_engine_GbaEngine_nativeLoadState(
        _env: JNIEnv,
        _class: JClass,
        handle: jlong,
        state_handle: jlong,
    ) -> jint {
        if handle == 0 || state_handle == 0 {
            return -1;
        }
        unsafe {
            geebeeayy_load_state(
                handle as *mut c_void,
                state_handle as *mut c_void,
            )
        }
    }

    #[no_mangle]
    pub extern "system" fn Java_com_geebeeayy_app_engine_GbaEngine_nativeSaveStateDestroy(
        _env: JNIEnv,
        _class: JClass,
        state_handle: jlong,
    ) {
        if state_handle != 0 {
            unsafe { geebeeayy_save_state_destroy(state_handle as *mut c_void); }
        }
    }
}
