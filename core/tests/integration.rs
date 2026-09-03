//! End-to-end smoke test: a hand-assembled ROM drives the CPU, the memory bus
//! and the PPU through one full frame.

use geebeeayy_core::Gba;

/// Build a minimal ROM image: `code` at the entry point, zero-padded past the
/// 0xC0-byte header the cartridge loader requires.
fn rom(code: &[u32]) -> Vec<u8> {
    let mut data = vec![0u8; 0x200];
    for (i, &word) in code.iter().enumerate() {
        data[i * 4..i * 4 + 4].copy_from_slice(&word.to_le_bytes());
    }
    data
}

#[test]
fn mode3_pixel_reaches_the_frame_buffer() {
    // mov  r0, #0x04000000     ; I/O base
    // mov  r1, #0x400          ; BG2 enable
    // add  r1, r1, #3          ; | mode 3
    // str  r1, [r0]            ; DISPCNT = 0x0403
    // mov  r0, #0x06000000     ; VRAM base
    // mvn  r1, #0              ; 0xFFFFFFFF
    // strh r1, [r0]            ; first pixel = white
    // b    .                   ; spin
    let mut gba = Gba::new();
    gba.load_rom(&rom(&[
        0xE3A0_0301,
        0xE3A0_1B01,
        0xE281_1003,
        0xE580_1000,
        0xE3A0_0406,
        0xE3E0_1000,
        0xE1C0_10B0,
        0xEAFF_FFFE,
    ]))
    .expect("ROM should load");

    gba.run_frame();

    assert_eq!(
        gba.bus.read16(0x0600_0000),
        0xFFFF,
        "the ROM never wrote VRAM"
    );

    let fb = gba.frame_buffer();
    assert!(
        fb[0..3].iter().any(|&c| c != 0),
        "PPU did not render the mode 3 pixel"
    );
}

#[test]
fn save_state_round_trip_preserves_registers() {
    let mut gba = Gba::new();
    gba.load_rom(&rom(&[0xE3A0_00AB, 0xEAFF_FFFE])) // mov r0, #0xAB ; b .
        .expect("ROM should load");
    gba.run_frame();
    assert_eq!(gba.cpu.registers[0], 0xAB);

    let state = gba.save_state();
    gba.cpu.registers[0] = 0;
    gba.load_state(&state).expect("state should restore");
    assert_eq!(gba.cpu.registers[0], 0xAB);
}
