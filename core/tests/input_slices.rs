//! A frame run in slices, with a key update between them.
//!
//! The frontend can poll input several times a frame (NanoBoyAdvance does it
//! four times) by calling `run_frame_slice(i, n)` for `i` in `0..n` and
//! pushing keys in between. Slicing must not change the emulation: the last
//! slice ends on exactly the cycle a single `run_frame` would.

use geebeeayy_core::Gba;

/// `code` at the entry point, zero-padded past the cartridge header.
fn rom(code: &[u32]) -> Vec<u8> {
    let mut data = vec![0u8; 0x200];
    for (i, &word) in code.iter().enumerate() {
        data[i * 4..i * 4 + 4].copy_from_slice(&word.to_le_bytes());
    }
    data
}

/// Mode 3, then a loop writing a running counter across VRAM, so the frame
/// buffer depends on exactly how many instructions ran before each scanline.
fn busy_rom() -> Vec<u8> {
    rom(&[
        0xE3A0_0301, // mov  r0, #0x04000000
        0xE3A0_1B01, // mov  r1, #0x400
        0xE281_1003, // add  r1, r1, #3
        0xE580_1000, // str  r1, [r0]          ; DISPCNT = mode 3 + BG2
        0xE3A0_3406, // mov  r3, #0x06000000
        0xE3A0_2000, // mov  r2, #0
        0xE282_2001, // loop: add r2, r2, #1
        0xE1A0_4882, // mov  r4, r2, lsl #17
        0xE1A0_4824, // mov  r4, r4, lsr #16    ; (r2 & 0x7FFF) * 2
        0xE183_20B4, // strh r2, [r3, r4]
        0xEAFF_FFFA, // b    loop
    ])
}

/// Run `frames` frames on two machines - one whole, one in `slices` parts -
/// and require the same cycle count, frame buffer and audio after each, and
/// the same machine state at the end.
fn assert_slicing_is_invisible(rom: &[u8], frames: u32, slices: u32) {
    let mut whole = Gba::new();
    let mut sliced = Gba::new();
    whole.load_rom(rom).expect("ROM should load");
    sliced.load_rom(rom).expect("ROM should load");

    for frame in 0..frames {
        whole.run_frame();
        for i in 0..slices {
            sliced.run_frame_slice(i, slices);
        }
        assert_eq!(
            whole.cycles, sliced.cycles,
            "frame {frame}: sliced run ended on a different cycle"
        );
        assert!(
            whole.frame_buffer()[..] == sliced.frame_buffer()[..],
            "frame {frame}: frame buffers differ"
        );
        assert_eq!(
            whole.apu_samples(),
            sliced.apu_samples(),
            "frame {frame}: audio differs"
        );
    }
    assert!(
        whole.save_state().data == sliced.save_state().data,
        "machine state differs after {frames} frames"
    );
}

#[test]
fn slicing_a_frame_matches_one_run_frame() {
    for slices in [1, 2, 3, 4, 7] {
        assert_slicing_is_invisible(&busy_rom(), 10, slices);
    }
}

/// The same check on real games, which bring interrupts, DMA, timers and
/// sound into it. Skips any ROM not fetched into `temp/roms/`.
#[test]
fn slicing_a_frame_matches_one_run_frame_on_real_roms() {
    for name in ["240p", "celeste", "tone", "arm"] {
        let path = format!("{}/../temp/roms/{name}.gba", env!("CARGO_MANIFEST_DIR"));
        let Ok(data) = std::fs::read(&path) else {
            eprintln!("skipping slice check on '{name}': {path} not present");
            continue;
        };
        assert_slicing_is_invisible(&data, 30, 4);
    }
}

#[test]
fn a_key_pressed_between_slices_is_seen_mid_frame() {
    // Spin until KEYINPUT shows A pressed, then record VCOUNT (| 0x8000 as a
    // "written" marker) in EWRAM.
    let code = rom(&[
        0xE3A0_0301, // mov  r0, #0x04000000
        0xE3A0_2402, // mov  r2, #0x02000000
        0xE280_6C01, // add  r6, r0, #0x100    ; ldrh offsets stop at 0xFF
        0xE1D6_13B0, // loop: ldrh r1, [r6, #0x30]   ; KEYINPUT
        0xE311_0001, // tst  r1, #1                   ; A released?
        0x1AFF_FFFC, // bne  loop
        0xE1D0_50B6, // ldrh r5, [r0, #6]             ; VCOUNT
        0xE385_5902, // orr  r5, r5, #0x8000
        0xE1C2_50B0, // strh r5, [r2]
        0xEAFF_FFFE, // b    .
    ]);
    let mut gba = Gba::new();
    gba.load_rom(&code).expect("ROM should load");

    gba.run_frame_slice(0, 4);
    assert_eq!(
        gba.bus.read16(0x0200_0000),
        0,
        "A seen before it was pressed"
    );

    gba.bus.set_keys(1); // A
    gba.run_frame_slice(1, 4);
    let seen = gba.bus.read16(0x0200_0000);
    assert_ne!(
        seen & 0x8000,
        0,
        "A press never reached the game this slice"
    );
    // A quarter of the 228-line frame has gone by.
    let line = seen & 0xFF;
    assert!(
        (56..=58).contains(&line),
        "A seen on scanline {line}, expected the start of the second quarter"
    );
}
