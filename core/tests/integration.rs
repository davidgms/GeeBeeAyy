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

#[test]
fn a_register_written_during_hblank_takes_effect_on_the_next_line() {
    // A game's HBlank handler sets up the line that follows, so a line has to
    // be drawn before its own HBlank runs. Drawing it at the end of the line
    // instead ate those writes a line early, and only on the frames where the
    // handler beat the 272 cycles left to the line boundary - which is what
    // made Yggdra Union's per-line BG1VOFS panels flicker between two
    // positions every frame.
    let mut gba = Gba::new();
    gba.load_rom(&rom(&[0xEAFF_FFFE])).expect("ROM should load");

    // Mode 0 with no background enabled: every pixel is the backdrop, so
    // palette entry 0 alone decides the colour of the line.
    gba.bus.write16(0x0400_0000, 0x0000);
    gba.bus.write16(0x0500_0000, 0x001F); // red

    gba.ppu.tick(960, &mut gba.bus, &mut gba.dma); // HBlank of line 0
    gba.bus.write16(0x0500_0000, 0x7C00); // blue, as an HBlank handler would
    gba.ppu.tick(272, &mut gba.bus, &mut gba.dma); // end of line 0
    gba.ppu.tick(960, &mut gba.bus, &mut gba.dma); // HBlank of line 1

    let fb = gba.frame_buffer();
    let line0 = &fb[0..3];
    let line1 = &fb[240 * 3..240 * 3 + 3];
    assert!(
        line0[0] > 200 && line0[2] < 60,
        "line 0 should still be red, got {line0:?}"
    );
    assert!(
        line1[2] > 200 && line1[0] < 60,
        "line 1 should be blue, got {line1:?}"
    );
}
