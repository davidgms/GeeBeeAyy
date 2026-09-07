//! Every `external fun` on the Kotlin side must have a `#[no_mangle]` JNI
//! export in `ffi.rs`, or the app crashes with `UnsatisfiedLinkError` the
//! first time that call is reached - at runtime, on a device, with nothing
//! at build time to catch it.
//!
//! This is not hypothetical: `nativeStateWrite` shipped without its
//! `#[no_mangle]` (a rewind block was inserted between the attribute and the
//! function), so loading a save state killed the process.

use std::path::Path;

#[test]
fn every_kotlin_external_fun_is_exported() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let kotlin = std::fs::read_to_string(
        root.join("android/app/src/main/java/com/geebeeayy/app/engine/GbaEngine.kt"),
    )
    .expect("GbaEngine.kt");
    let ffi = std::fs::read_to_string(root.join("core/src/ffi.rs")).expect("ffi.rs");

    let declared: Vec<&str> = kotlin
        .lines()
        .filter_map(|line| line.trim().strip_prefix("private external fun "))
        .chain(
            kotlin
                .lines()
                .filter_map(|line| line.trim().strip_prefix("external fun ")),
        )
        .map(|rest| rest.split('(').next().unwrap_or("").trim())
        .filter(|name| !name.is_empty())
        .collect();

    assert!(
        !declared.is_empty(),
        "found no external declarations - has GbaEngine.kt moved?"
    );

    let missing: Vec<&&str> = declared
        .iter()
        .filter(|name| {
            let symbol = format!("Java_com_geebeeayy_app_engine_GbaEngine_{name}");
            // The attribute has to be on the function itself, so look for the
            // pair rather than for either half on its own.
            match ffi.find(&format!("fn {symbol}")) {
                // Walk back to the start of the signature's line, then check
                // that the line above it is the attribute.
                Some(at) => {
                    let line_start = ffi[..at].rfind('\n').map_or(0, |n| n + 1);
                    !ffi[..line_start].trim_end().ends_with("#[no_mangle]")
                }
                None => true,
            }
        })
        .collect();

    assert!(
        missing.is_empty(),
        "declared in GbaEngine.kt but not exported with #[no_mangle] from ffi.rs: {missing:?}"
    );
}

/// `geebeeayy_load_rom`'s debug line used to index the first four bytes before
/// anything validated the length, so a short buffer panicked - and a panic
/// unwinding out of an `extern "C"` function aborts the process rather than
/// returning an error.
#[test]
fn load_rom_rejects_a_buffer_too_short_to_index() {
    unsafe {
        let handle = geebeeayy_core::ffi::geebeeayy_create();
        assert!(!handle.is_null());
        for len in 0..4usize {
            let data = vec![0u8; len];
            let rc = geebeeayy_core::ffi::geebeeayy_load_rom(handle, data.as_ptr(), len);
            assert_eq!(rc, -1, "a {len}-byte ROM should be rejected, not panic");
        }
        geebeeayy_core::ffi::geebeeayy_destroy(handle);
    }
}
