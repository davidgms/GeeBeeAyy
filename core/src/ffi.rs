use crate::{savestate::SaveState, Gba};
use std::ffi::c_void;

// ─── C FFI (used by iOS via bridging header, and by Android via JNI) ────────

/// Opaque handle to Gba instance for FFI consumers.
pub struct GbaHandle {
    inner: Gba,
    /// Bounded ring of snapshots for rewind. Empty until the frontend gives
    /// it a capacity, because a state is around 500 KB and the core has no
    /// business guessing how much of the device's memory it may have.
    rewind: crate::rewind::Rewind,
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
    let handle = Box::new(GbaHandle {
        inner: Gba::new(),
        rewind: crate::rewind::Rewind::new(0),
    });
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
        unsafe {
            drop(Box::from_raw(ptr as *mut GbaHandle));
        }
    }
}

/// Load a ROM from a byte buffer.
///
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `ptr` must be a valid handle. `data` must point to `len` readable bytes.
#[no_mangle]
pub unsafe extern "C" fn geebeeayy_load_rom(ptr: *mut c_void, data: *const u8, len: usize) -> i32 {
    if ptr.is_null() || data.is_null() {
        return -1;
    }
    let handle = unsafe { &mut *(ptr as *mut GbaHandle) };
    let rom = unsafe { std::slice::from_raw_parts(data, len) };
    eprintln!(
        "[GeeBeeAyy] load_rom: {} bytes, first4={:02X}{:02X}{:02X}{:02X}",
        len, rom[0], rom[1], rom[2], rom[3]
    );
    match handle.inner.load_rom(rom) {
        Ok(()) => {
            eprintln!(
                "[GeeBeeAyy] load_rom: OK, CPU PC={:08X} CPSR={:08X}",
                handle.inner.cpu.registers[15], handle.inner.cpu.cpsr
            );
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
            if chunk[0] != 0 || chunk[1] != 0 || chunk[2] != 0 {
                non_zero += 1;
            }
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

/// Set the pressed-button bitmask.
///
/// `keys` uses the GBATEK bit order (0=A, 1=B, 2=Select, 3=Start, 4=Right,
/// 5=Left, 6=Up, 7=Down, 8=R, 9=L) with **1 = pressed**, which is the natural
/// polarity for a caller. The inversion to the hardware's active-low KEYINPUT
/// happens inside the core, so a frontend never deals with it. Bits above 9
/// are ignored.
///
/// # Safety
/// `ptr` must be a valid handle from `geebeeayy_create`.
#[no_mangle]
pub unsafe extern "C" fn geebeeayy_set_keys(ptr: *mut c_void, keys: u16) {
    if ptr.is_null() {
        return;
    }
    let handle = unsafe { &mut *(ptr as *mut GbaHandle) };
    handle.inner.bus.set_keys(keys);
}

/// Size of the cartridge's battery save in bytes, or 0 if it has no save chip.
///
/// # Safety
/// `ptr` must be a valid handle from `geebeeayy_create`.
#[no_mangle]
pub unsafe extern "C" fn geebeeayy_save_size(ptr: *mut c_void) -> usize {
    if ptr.is_null() {
        return 0;
    }
    let handle = unsafe { &*(ptr as *const GbaHandle) };
    handle.inner.save_data().map_or(0, |d| d.len())
}

/// Copy the battery save into `out`, returning the number of bytes written.
///
/// Call `geebeeayy_save_take_dirty` **before** this, never after: a write that
/// lands between the read and the clear would be dropped on the floor.
///
/// # Safety
/// `ptr` must be a valid handle and `out` must have room for `max_len` bytes.
#[no_mangle]
pub unsafe extern "C" fn geebeeayy_save_read(
    ptr: *mut c_void,
    out: *mut u8,
    max_len: usize,
) -> usize {
    if ptr.is_null() || out.is_null() {
        return 0;
    }
    let handle = unsafe { &*(ptr as *const GbaHandle) };
    let Some(data) = handle.inner.save_data() else {
        return 0;
    };
    let count = data.len().min(max_len);
    unsafe {
        std::ptr::copy_nonoverlapping(data.as_ptr(), out, count);
    }
    count
}

/// Restore a battery save. Returns 0 on success, -1 on a null argument.
///
/// # Safety
/// `ptr` must be a valid handle and `data` must point to `len` readable bytes.
#[no_mangle]
pub unsafe extern "C" fn geebeeayy_save_write(
    ptr: *mut c_void,
    data: *const u8,
    len: usize,
) -> i32 {
    if ptr.is_null() || data.is_null() {
        return -1;
    }
    let handle = unsafe { &mut *(ptr as *mut GbaHandle) };
    let bytes = unsafe { std::slice::from_raw_parts(data, len) };
    handle.inner.load_save(bytes);
    0
}

/// Whether save memory changed since the last call, clearing the flag.
///
/// Returns 1 if dirty, 0 otherwise. Check this first, then call
/// `geebeeayy_save_read` - see its note on ordering.
///
/// # Safety
/// `ptr` must be a valid handle from `geebeeayy_create`.
#[no_mangle]
pub unsafe extern "C" fn geebeeayy_save_take_dirty(ptr: *mut c_void) -> i32 {
    if ptr.is_null() {
        return 0;
    }
    let handle = unsafe { &mut *(ptr as *mut GbaHandle) };
    handle.inner.take_save_dirty() as i32
}

/// Size in bytes of a save state taken right now.
///
/// # Safety
/// `ptr` must be a valid handle from `geebeeayy_create`.
#[no_mangle]
pub unsafe extern "C" fn geebeeayy_state_size(ptr: *mut c_void) -> usize {
    if ptr.is_null() {
        return 0;
    }
    let handle = unsafe { &*(ptr as *const GbaHandle) };
    handle.inner.save_state().data.len()
}

/// Copy a save state into `out`, returning the number of bytes written, or 0
/// if `out` is too small - check `geebeeayy_state_size` first.
///
/// A state snapshots our internal layout, not the game's own save; see
/// `docs/save-data.md` for why the two are stored differently.
///
/// # Safety
/// `ptr` must be a valid handle and `out` must have room for `max_len` bytes.
/// Set how many rewind snapshots to keep, and drop any already held.
///
/// A snapshot is about 500 KB, so a depth of 20 costs roughly 10 MB. The
/// frontend decides the cadence as well as the depth - the core has no clock,
/// the same reason it does not decide when to flush a battery save.
///
/// # Safety
/// `ptr` must be a valid handle from `geebeeayy_create`.
#[no_mangle]
pub unsafe extern "C" fn geebeeayy_rewind_configure(ptr: *mut c_void, capacity: usize) {
    if ptr.is_null() {
        return;
    }
    let handle = unsafe { &mut *(ptr as *mut GbaHandle) };
    handle.rewind = crate::rewind::Rewind::new(capacity);
}

/// Snapshot the machine into the rewind ring, discarding the oldest entry if
/// it is full. A no-op while the capacity is zero.
///
/// # Safety
/// `ptr` must be a valid handle from `geebeeayy_create`.
#[no_mangle]
pub unsafe extern "C" fn geebeeayy_rewind_push(ptr: *mut c_void) {
    if ptr.is_null() {
        return;
    }
    let handle = unsafe { &mut *(ptr as *mut GbaHandle) };
    if handle.rewind.capacity() == 0 {
        return;
    }
    handle.rewind.push(&handle.inner);
}

/// Restore the most recent snapshot and drop it, so repeated calls walk
/// backwards. Returns 1 on success, 0 when the ring is empty, -1 if the
/// snapshot was rejected - in which case the machine is unchanged, because
/// `SaveState::restore` rolls back on failure.
///
/// # Safety
/// `ptr` must be a valid handle from `geebeeayy_create`.
#[no_mangle]
pub unsafe extern "C" fn geebeeayy_rewind_pop(ptr: *mut c_void) -> i32 {
    if ptr.is_null() {
        return 0;
    }
    let handle = unsafe { &mut *(ptr as *mut GbaHandle) };
    let GbaHandle { inner, rewind } = handle;
    match rewind.pop(inner) {
        Ok(true) => 1,
        Ok(false) => 0,
        Err(_) => -1,
    }
}

/// Bytes the rewind ring is holding right now, so a frontend can size its
/// depth against the device instead of guessing.
///
/// # Safety
/// `ptr` must be a valid handle from `geebeeayy_create`.
#[no_mangle]
pub unsafe extern "C" fn geebeeayy_rewind_memory(ptr: *mut c_void) -> usize {
    if ptr.is_null() {
        return 0;
    }
    let handle = unsafe { &*(ptr as *const GbaHandle) };
    handle.rewind.memory_bytes()
}

/// Drop every snapshot, for a ROM change or a save-state load.
///
/// # Safety
/// `ptr` must be a valid handle from `geebeeayy_create`.
#[no_mangle]
pub unsafe extern "C" fn geebeeayy_rewind_clear(ptr: *mut c_void) {
    if ptr.is_null() {
        return;
    }
    let handle = unsafe { &mut *(ptr as *mut GbaHandle) };
    handle.rewind.clear();
}

#[no_mangle]
pub unsafe extern "C" fn geebeeayy_state_read(
    ptr: *mut c_void,
    out: *mut u8,
    max_len: usize,
) -> usize {
    if ptr.is_null() || out.is_null() {
        return 0;
    }
    let handle = unsafe { &*(ptr as *const GbaHandle) };
    let state = handle.inner.save_state();
    if state.data.len() > max_len {
        return 0;
    }
    unsafe {
        std::ptr::copy_nonoverlapping(state.data.as_ptr(), out, state.data.len());
    }
    state.data.len()
}

/// Restore a save state. Returns 0 on success, -1 if the data is rejected.
///
/// A rejected state leaves the emulator **partially restored**: `restore`
/// writes into the live machine as it parses. Treat -1 as "reload the ROM",
/// not as "carry on". See `docs/save-data.md`.
///
/// # Safety
/// `ptr` must be a valid handle and `data` must point to `len` readable bytes.
#[no_mangle]
pub unsafe extern "C" fn geebeeayy_state_write(
    ptr: *mut c_void,
    data: *const u8,
    len: usize,
) -> i32 {
    if ptr.is_null() || data.is_null() {
        return -1;
    }
    let handle = unsafe { &mut *(ptr as *mut GbaHandle) };
    let bytes = unsafe { std::slice::from_raw_parts(data, len) };
    let state = crate::savestate::SaveState {
        data: bytes.to_vec(),
    };
    match handle.inner.load_state(&state) {
        Ok(()) => 0,
        Err(_) => -1,
    }
}

#[cfg(target_os = "android")]
pub mod android {
    use super::*;
    use jni::objects::{JByteArray, JClass, JFloatArray, ReleaseMode};
    use jni::sys::{jint, jlong};
    use jni::JNIEnv;

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
            unsafe {
                geebeeayy_destroy(handle as *mut c_void);
            }
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
            unsafe {
                geebeeayy_run_frame(handle as *mut c_void);
            }
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
            unsafe {
                geebeeayy_run_frames(handle as *mut c_void, count as u32);
            }
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
        unsafe {
            geebeeayy_frame_buffer_copy(handle as *mut c_void, buf.as_mut_ptr());
        }
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
    pub extern "system" fn Java_com_geebeeayy_app_engine_GbaEngine_nativeSetKeys(
        _env: JNIEnv,
        _class: JClass,
        handle: jlong,
        keys: jint,
    ) {
        if handle == 0 {
            return;
        }
        let gba = unsafe { &mut *(handle as *mut GbaHandle) };
        gba.inner.bus.set_keys(keys as u16);
    }

    #[no_mangle]
    pub extern "system" fn Java_com_geebeeayy_app_engine_GbaEngine_nativeSaveTakeDirty(
        _env: JNIEnv,
        _class: JClass,
        handle: jlong,
    ) -> jint {
        if handle == 0 {
            return 0;
        }
        let gba = unsafe { &mut *(handle as *mut GbaHandle) };
        gba.inner.take_save_dirty() as jint
    }

    #[no_mangle]
    pub extern "system" fn Java_com_geebeeayy_app_engine_GbaEngine_nativeSaveRead<'local>(
        mut env: JNIEnv<'local>,
        _class: JClass,
        handle: jlong,
    ) -> JByteArray<'local> {
        let empty = env.new_byte_array(0).unwrap_or_default();
        if handle == 0 {
            return empty;
        }
        let gba = unsafe { &*(handle as *const GbaHandle) };
        let Some(data) = gba.inner.save_data() else {
            return empty;
        };
        let signed: Vec<i8> = data.iter().map(|&b| b as i8).collect();
        match env.new_byte_array(signed.len() as i32) {
            Ok(array) => {
                let _ = env.set_byte_array_region(&array, 0, &signed);
                array
            }
            Err(_) => empty,
        }
    }

    #[no_mangle]
    pub extern "system" fn Java_com_geebeeayy_app_engine_GbaEngine_nativeSaveWrite(
        mut env: JNIEnv,
        _class: JClass,
        handle: jlong,
        data: JByteArray,
    ) -> jint {
        if handle == 0 {
            return -1;
        }
        let gba = unsafe { &mut *(handle as *mut GbaHandle) };
        let Ok(bytes) = (unsafe { env.get_array_elements(&data, ReleaseMode::NoCopyBack) }) else {
            return -1;
        };
        let buf: Vec<u8> = bytes.iter().map(|&b| b as u8).collect();
        gba.inner.load_save(&buf);
        0
    }

    #[no_mangle]
    pub extern "system" fn Java_com_geebeeayy_app_engine_GbaEngine_nativeStateRead<'local>(
        mut env: JNIEnv<'local>,
        _class: JClass,
        handle: jlong,
    ) -> JByteArray<'local> {
        let empty = env.new_byte_array(0).unwrap_or_default();
        if handle == 0 {
            return empty;
        }
        let gba = unsafe { &*(handle as *const GbaHandle) };
        let state = gba.inner.save_state();
        let signed: Vec<i8> = state.data.iter().map(|&b| b as i8).collect();
        match env.new_byte_array(signed.len() as i32) {
            Ok(array) => {
                let _ = env.set_byte_array_region(&array, 0, &signed);
                array
            }
            Err(_) => empty,
        }
    }

    /// Rewind: configure the ring's depth, take a snapshot, step back one.
    #[no_mangle]
    pub extern "system" fn Java_com_geebeeayy_app_engine_GbaEngine_nativeRewindConfigure(
        _env: JNIEnv,
        _class: JClass,
        handle: jlong,
        capacity: jint,
    ) {
        if handle == 0 {
            return;
        }
        unsafe { geebeeayy_rewind_configure(handle as *mut c_void, capacity.max(0) as usize) };
    }

    #[no_mangle]
    pub extern "system" fn Java_com_geebeeayy_app_engine_GbaEngine_nativeRewindPush(
        _env: JNIEnv,
        _class: JClass,
        handle: jlong,
    ) {
        if handle == 0 {
            return;
        }
        unsafe { geebeeayy_rewind_push(handle as *mut c_void) };
    }

    #[no_mangle]
    pub extern "system" fn Java_com_geebeeayy_app_engine_GbaEngine_nativeRewindPop(
        _env: JNIEnv,
        _class: JClass,
        handle: jlong,
    ) -> jint {
        if handle == 0 {
            return 0;
        }
        unsafe { geebeeayy_rewind_pop(handle as *mut c_void) }
    }

    #[no_mangle]
    pub extern "system" fn Java_com_geebeeayy_app_engine_GbaEngine_nativeRewindClear(
        _env: JNIEnv,
        _class: JClass,
        handle: jlong,
    ) {
        if handle == 0 {
            return;
        }
        unsafe { geebeeayy_rewind_clear(handle as *mut c_void) };
    }

    #[no_mangle]
    pub extern "system" fn Java_com_geebeeayy_app_engine_GbaEngine_nativeStateWrite(
        mut env: JNIEnv,
        _class: JClass,
        handle: jlong,
        data: JByteArray,
    ) -> jint {
        if handle == 0 {
            return -1;
        }
        let gba = unsafe { &mut *(handle as *mut GbaHandle) };
        let Ok(bytes) = (unsafe { env.get_array_elements(&data, ReleaseMode::NoCopyBack) }) else {
            return -1;
        };
        let buf: Vec<u8> = bytes.iter().map(|&b| b as u8).collect();
        let state = crate::savestate::SaveState { data: buf };
        match gba.inner.load_state(&state) {
            Ok(()) => 0,
            Err(_) => -1,
        }
    }
}
