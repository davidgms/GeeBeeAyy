//! Rewind: the ring buffer, and that a rewind actually goes back in time.

use geebeeayy_core::rewind::Rewind;
use geebeeayy_core::Gba;

fn rom() -> Vec<u8> {
    // mov r0, #0 ; add r0, r0, #1 ; b .-4  - a counter that keeps climbing, so
    // "did we go back" is answerable by looking at r0.
    let code: [u32; 3] = [0xE3A0_0000, 0xE280_0001, 0xEAFF_FFFD];
    let mut data = vec![0u8; 0x400];
    for (i, &w) in code.iter().enumerate() {
        data[i * 4..i * 4 + 4].copy_from_slice(&w.to_le_bytes());
    }
    data
}

fn running_gba() -> Gba {
    let mut gba = Gba::new();
    gba.load_rom(&rom()).expect("ROM should load");
    gba
}

#[test]
fn rewind_returns_the_machine_to_an_earlier_frame() {
    let mut gba = running_gba();
    let mut rewind = Rewind::new(8);

    gba.run_frame();
    rewind.push(&gba);
    let at_snapshot = gba.cpu.registers[0];

    for _ in 0..4 {
        gba.run_frame();
    }
    assert!(
        gba.cpu.registers[0] > at_snapshot,
        "the test ROM should have counted further"
    );

    assert!(rewind.pop(&mut gba).expect("restore should succeed"));
    assert_eq!(gba.cpu.registers[0], at_snapshot, "rewind did not go back");
}

#[test]
fn the_ring_discards_the_oldest_past_capacity() {
    let mut gba = running_gba();
    let mut rewind = Rewind::new(3);
    for _ in 0..6 {
        gba.run_frame();
        rewind.push(&gba);
    }
    assert_eq!(rewind.len(), 3, "the ring must not grow past its capacity");
}

#[test]
fn popping_walks_backwards_and_then_runs_out() {
    let mut gba = running_gba();
    let mut rewind = Rewind::new(4);

    let mut marks = Vec::new();
    for _ in 0..3 {
        gba.run_frame();
        rewind.push(&gba);
        marks.push(gba.cpu.registers[0]);
    }

    // Each pop should land on the previous mark, newest first.
    for expected in marks.iter().rev() {
        assert!(rewind.pop(&mut gba).unwrap());
        assert_eq!(gba.cpu.registers[0], *expected);
    }
    assert!(
        !rewind.pop(&mut gba).unwrap(),
        "an empty ring reports false"
    );
    assert!(rewind.is_empty());
}

#[test]
fn memory_cost_is_reported_so_a_caller_can_size_itself() {
    let mut gba = running_gba();
    let mut rewind = Rewind::new(4);
    assert_eq!(rewind.memory_bytes(), 0);

    gba.run_frame();
    rewind.push(&gba);
    let one = rewind.memory_bytes();
    assert!(one > 0);

    rewind.push(&gba);
    assert_eq!(
        rewind.memory_bytes(),
        one * 2,
        "cost should scale with depth"
    );

    rewind.clear();
    assert_eq!(rewind.memory_bytes(), 0);
}

/// The FFI's rewind ring: a frontend configures a depth, pushes snapshots on
/// its own cadence and pops to walk backwards. The core had the ring since
/// 2026-08 but nothing could reach it - there was no FFI at all.
#[test]
fn the_rewind_ffi_walks_backwards_and_stops_when_empty() {
    use std::ffi::c_void;

    unsafe {
        let handle = geebeeayy_core::ffi::geebeeayy_create();
        assert!(!handle.is_null());

        let mut rom = vec![0u8; 0x200];
        // `b .` so the machine advances its clock and nothing else.
        rom[0..4].copy_from_slice(&0xEAFF_FFFEu32.to_le_bytes());
        geebeeayy_core::ffi::geebeeayy_load_rom(handle, rom.as_ptr(), rom.len());

        // Zero capacity is the default, so a push does nothing and a pop has
        // nothing to give.
        geebeeayy_core::ffi::geebeeayy_rewind_push(handle);
        assert_eq!(geebeeayy_core::ffi::geebeeayy_rewind_pop(handle), 0);
        assert_eq!(geebeeayy_core::ffi::geebeeayy_rewind_memory(handle), 0);

        geebeeayy_core::ffi::geebeeayy_rewind_configure(handle, 3);
        for _ in 0..5 {
            geebeeayy_core::ffi::geebeeayy_run_frame(handle);
            geebeeayy_core::ffi::geebeeayy_rewind_push(handle);
        }
        // The ring is bounded: five pushes into a depth of three keep three.
        assert!(geebeeayy_core::ffi::geebeeayy_rewind_memory(handle) > 0);
        for step in 0..3 {
            assert_eq!(
                geebeeayy_core::ffi::geebeeayy_rewind_pop(handle),
                1,
                "pop {step} should have restored a snapshot"
            );
        }
        assert_eq!(
            geebeeayy_core::ffi::geebeeayy_rewind_pop(handle),
            0,
            "a fourth pop must report the ring empty"
        );
        assert_eq!(geebeeayy_core::ffi::geebeeayy_rewind_memory(handle), 0);

        geebeeayy_core::ffi::geebeeayy_rewind_configure(handle, 2);
        geebeeayy_core::ffi::geebeeayy_rewind_push(handle);
        geebeeayy_core::ffi::geebeeayy_rewind_clear(handle);
        assert_eq!(geebeeayy_core::ffi::geebeeayy_rewind_pop(handle), 0);

        geebeeayy_core::ffi::geebeeayy_destroy(handle);
        let _: *mut c_void = std::ptr::null_mut();
    }
}
