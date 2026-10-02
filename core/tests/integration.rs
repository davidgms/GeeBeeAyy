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

// ---------------------------------------------------------------------------
// Peeking memory for achievements
// ---------------------------------------------------------------------------

/// The three regions an achievement runtime asks a GBA for, and nothing else.
#[test]
fn peek_sees_work_ram_and_nothing_it_should_not() {
    let mut gba = Gba::new();
    gba.bus.write8(0x0200_0100, 0xAB); // EWRAM
    gba.bus.write8(0x0300_0040, 0xCD); // IWRAM
    gba.bus.write8(0x0500_0000, 0xEF); // palette - deliberately not readable

    let mut out = [0u8; 1];
    gba.peek_memory(0x0200_0100, &mut out);
    assert_eq!(out[0], 0xAB, "external work RAM must be readable");
    gba.peek_memory(0x0300_0040, &mut out);
    assert_eq!(out[0], 0xCD, "internal work RAM must be readable");

    for address in [
        0x0000_0000u32,
        0x0400_0000,
        0x0500_0000,
        0x0600_0000,
        0x0800_0000,
    ] {
        gba.peek_memory(address, &mut out);
        assert_eq!(
            out[0], 0,
            "0x{address:08X} must not be visible to an achievement"
        );
    }
}

#[test]
fn peek_copies_a_run_of_bytes() {
    let mut gba = Gba::new();
    for i in 0..8u32 {
        gba.bus.write8(0x0200_0200 + i, (i as u8) + 1);
    }
    let mut out = [0u8; 8];
    assert_eq!(gba.peek_memory(0x0200_0200, &mut out), 8);
    assert_eq!(out, [1, 2, 3, 4, 5, 6, 7, 8]);
}

/// The reason `peek` exists at all rather than reusing `read8`.
///
/// A bus read of the EEPROM window answers "chip ready" so that a game polling
/// after a write can finish saving - the fix that turned *Yggdra Union*'s
/// "Save failed!" into a real save. An observer walking memory once a frame
/// must never be able to send that answer, so the whole window reads 0 here
/// whatever cart is in.
#[test]
fn peek_never_speaks_for_the_eeprom_chip() {
    let gba = Gba::new();
    let mut out = [0u8; 1];
    for address in [0x0DFF_FF00u32, 0x0DFF_FFFF, 0x0D00_0000] {
        gba.peek_memory(address, &mut out);
        assert_eq!(out[0], 0, "0x{address:08X} must read 0 to an observer");
    }
}

/// Running off the end of a region reads 0 rather than wrapping into another.
#[test]
fn peek_past_the_end_of_a_region_reads_zero() {
    let mut gba = Gba::new();
    gba.bus.write8(0x0300_7FFF, 0x5A); // last byte of internal work RAM
    let mut out = [0u8; 2];
    gba.peek_memory(0x0300_7FFF, &mut out);
    assert_eq!(out[0], 0x5A);
    assert_eq!(out[1], 0, "past the end of the region must read 0");
}

// ---------------------------------------------------------------------------
// Reset
// ---------------------------------------------------------------------------

/// A reset is not an eject. The cart stays, and so does its battery save -
/// a real GBA keeps SRAM across a reset, and a player resetting past a crash
/// must not lose their game doing it.
#[test]
fn reset_clears_work_ram_but_keeps_the_save() {
    let mut gba = Gba::new();
    // A cart the save-type detector recognises. Without one the save is
    // `SaveType::None` and every write goes nowhere, which would make this
    // test pass for the wrong reason.
    let mut rom = vec![0u8; 0x400];
    rom[0x100..0x100 + 9].copy_from_slice(b"SRAM_V100");
    gba.load_rom(&rom).expect("ROM should load");

    gba.bus.write8(0x0200_0100, 0xAB); // external work RAM
    gba.bus.write8(0x0300_0040, 0xCD); // internal work RAM
    gba.bus.write8(0x0500_0000, 0xEF); // palette
    gba.bus.cart.save_write(0x0E00_0000, 0x42);

    gba.reset();

    assert_eq!(gba.bus.read8(0x0200_0100), 0, "work RAM must be cleared");
    assert_eq!(
        gba.bus.read8(0x0300_0040),
        0,
        "internal work RAM must be cleared"
    );
    assert_eq!(gba.bus.read8(0x0500_0000), 0, "palette must be cleared");
    assert_eq!(
        gba.bus.cart.save_read(0x0E00_0000),
        0x42,
        "the battery save must survive a reset"
    );
    assert_eq!(
        gba.bus.read8(0x0800_0100),
        b'S',
        "a reset is not an eject - the ROM stays in the slot"
    );
}

/// KEYINPUT is active low, so a zeroed register reads as every button held.
/// `MemoryBus::new` knows that; reset has to know it too.
#[test]
fn reset_leaves_every_button_released() {
    let mut gba = Gba::new();
    gba.bus.set_keys(0x03FF); // all ten held
    gba.reset();
    assert_eq!(
        gba.bus.read16(0x0400_0130),
        0x03FF,
        "after a reset no button may read as held"
    );
}

#[test]
fn reset_puts_the_clock_back_to_zero() {
    let mut gba = Gba::new();
    gba.run_frame();
    assert!(gba.cycles > 0, "sanity: a frame costs cycles");
    gba.reset();
    assert_eq!(gba.cycles, 0);
}

/// Loading a second ROM is a power cycle: nothing of the first game's CPU,
/// memory or I/O state may survive into the second. `load_rom` used to swap
/// the cartridge and reset only the PC.
#[test]
fn loading_a_rom_resets_the_machine() {
    let mut gba = Gba::new();
    gba.load_rom(&rom(&[0xEAFF_FFFE])).expect("ROM should load");
    gba.cpu.registers[0] = 0x1234_5678;
    gba.bus.write32(0x0200_0000, 0xDEAD_BEEF);
    gba.bus.write32(0x0300_0000, 0xDEAD_BEEF);
    gba.bus.write16(0x0500_0000, 0x7FFF);
    gba.bus.write16(0x0400_0000, 0x0403); // DISPCNT
    gba.bus.write16(0x0400_0200, 0x0001); // IE
    gba.bus.write16(0x0400_0100, 0xFF00);
    gba.bus.write16(0x0400_0102, 0x0080); // timer 0 on
    gba.run_frame();
    gba.set_interframe_blend(true);

    gba.load_rom(&rom(&[0xEAFF_FFFE])).expect("ROM should load");
    assert!(gba.ppu.interframe_blend(), "a display setting is not state");
    assert_eq!(gba.cpu.registers[0], 0, "CPU registers survived the load");
    assert_eq!(gba.bus.read32(0x0200_0000), 0, "EWRAM survived the load");
    assert_eq!(gba.bus.read32(0x0300_0000), 0, "IWRAM survived the load");
    assert_eq!(gba.bus.read16(0x0500_0000), 0, "palette survived the load");
    assert_eq!(gba.bus.read16(0x0400_0000), 0, "DISPCNT survived the load");
    assert_eq!(gba.bus.io.ie, 0, "IE survived the load");
    assert_eq!(gba.cycles, 0, "the cycle count survived the load");
    gba.run_frame();
    assert_eq!(gba.bus.read16(0x0400_0100), 0, "timer 0 is still running");
}

/// The mGBA suite's I/O read test (`src/io-read.c`): write 0xFFFF to a
/// register, then `ldrh` it from THUMB code in ROM whose next-but-one
/// halfword is 0xDEAD. A readable register returns only its readable bits; a
/// write-only or unused one returns the open bus, which for THUMB code in ROM
/// is the prefetched `[$+4]` in both halves (GBATEK, Reading from Unused
/// Memory); a few unused ones read as zero. Expected values are the suite's,
/// recorded on hardware, and agree with GBATEK's register descriptions.
#[test]
fn io_reads_return_readable_bits_and_open_bus_like_the_mgba_io_read_suite() {
    use geebeeayy_core::cpu::Cpu;
    use geebeeayy_core::memory::MemoryBus;

    let mut data = vec![0u8; 0x200];
    for (i, half) in [
        0x8808u16, // ldrh r0, [r1]
        0xE001,    // b +2, over the data
        0xDEAD, 0xDEAD,
    ]
    .iter()
    .enumerate()
    {
        data[i * 2..i * 2 + 2].copy_from_slice(&half.to_le_bytes());
    }
    let cases: [(u32, u16); 16] = [
        (0x0400_0008, 0xDFFF), // BG0CNT: bit 13 only exists for BG2/BG3
        (0x0400_000C, 0xFFFF), // BG2CNT
        (0x0400_0010, 0xDEAD), // BG0HOFS: write-only
        (0x0400_0048, 0x3F3F), // WININ
        (0x0400_0054, 0xDEAD), // BLDY: write-only
        (0x0400_0062, 0xFFC0), // SOUND1CNT_H: length is write-only
        (0x0400_0064, 0x4000), // SOUND1CNT_X: only the length flag reads
        (0x0400_0066, 0x0000), // unused, reads zero
        (0x0400_0082, 0x770F), // SOUNDCNT_H
        (0x0400_00A0, 0xDEAD), // FIFO_A: write-only
        (0x0400_00B8, 0x0000), // DMA0CNT_L: write-only, reads zero
        (0x0400_00BA, 0xF7E0), // DMA0CNT_H: no gamepak DRQ below DMA3
        (0x0400_00DE, 0xFFE0), // DMA3CNT_H
        (0x0400_00E0, 0xDEAD), // unused, open bus
        (0x0400_0302, 0x0000), // unused, reads zero
        (0x0400_100C, 0xDEAD), // past the I/O block, open bus
    ];
    for (address, expected) in cases {
        let mut bus = MemoryBus::new();
        bus.load_rom(&data);
        let mut cpu = Cpu::new();
        cpu.registers[15] = 0x0800_0000;
        cpu.cpsr |= 0x20; // THUMB
        cpu.registers[1] = address;
        bus.write16(address, 0xFFFF);
        cpu.step(&mut bus);
        assert_eq!(
            cpu.registers[0] as u16, expected,
            "ldrh from {address:08X} after writing 0xFFFF"
        );
    }
}

/// The masks above are what the CPU sees, not what the hardware latched: the
/// PPU and APU still need the write-only bits a game stored.
#[test]
fn io_read_masks_do_not_hide_write_only_bits_from_the_ppu() {
    let mut gba = Gba::new();
    gba.load_rom(&rom(&[0xEAFF_FFFE])).expect("ROM should load");
    gba.bus.write16(0x0400_0010, 0x0123); // BG0HOFS, write-only
    assert_eq!(gba.bus.io_read16(0x0400_0010), 0x0123);
}

/// Commercial games run from boot without the PC ever leaving mapped memory,
/// and at a per-frame instruction count close to master's (a9d34a8, first 300
/// frames from boot: Yggdra Union 29.9k, Mario Tennis 142.6k - it busy-waits).
/// The BIOS IRQ stub regression of 2026-10-02 put Yggdra at ~245k a frame
/// with the PC in open bus. The ROMs are never committed: a missing one is
/// skipped.
#[test]
fn commercial_games_stay_in_mapped_memory_at_their_usual_cost() {
    const FRAMES: u32 = 300;
    for (name, ceiling) in [("yggdra", 40_000), ("mariotennis", 160_000)] {
        let path = format!("{}/../temp/roms/{name}.gba", env!("CARGO_MANIFEST_DIR"));
        let Ok(data) = std::fs::read(&path) else {
            eprintln!("skipping '{name}': {path} not present");
            continue;
        };
        let mut gba = Gba::new();
        gba.load_rom(&data).expect("ROM should load");
        let (mut steps, mut stray) = (0u64, None);
        for frame in 0..FRAMES {
            // Tap Start, then A, so the game gets past its title screens.
            let keys = match frame % 120 {
                0..=4 => 1 << 3,
                61..=64 => 1,
                _ => 0,
            };
            gba.bus.set_keys(keys);
            let target = gba.cycles + 280_896;
            while gba.cycles < target {
                let pc = gba.cpu.registers[15];
                let mapped = pc < 0x4000 || matches!(pc >> 24, 0x02 | 0x03 | 0x05..=0x0D);
                if !mapped && stray.is_none() {
                    stray = Some((frame, pc));
                }
                gba.step();
                steps += 1;
            }
        }
        assert_eq!(stray, None, "{name}: PC in open-bus memory (frame, pc)");
        let per_frame = steps / u64::from(FRAMES);
        assert!(
            per_frame < ceiling,
            "{name}: {per_frame} instructions a frame - something is spinning"
        );
    }
}
