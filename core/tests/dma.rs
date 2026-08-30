//! DMA wiring: the registers a game actually writes have to reach the DMA
//! controller. `Dma::write_sad`/`write_dad`/`write_count`/`write_control` had
//! no caller outside `dma.rs` for the project's whole history, so `channels[]`
//! kept its defaults forever and no game-initiated transfer ever ran.
//!
//! Everything here drives the machine the way a cartridge does: values go in
//! through `MemoryBus`, never into `Dma` directly.

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

/// A machine running `b .` forever, so `run_frame` only advances the clock.
fn spinning_gba() -> Gba {
    let mut gba = Gba::new();
    gba.load_rom(&rom(&[0xEAFF_FFFE])).expect("ROM should load");
    gba
}

/// DMAxSAD offsets from 0x04000000. Each channel is 12 bytes further on.
const SAD: [u32; 4] = [0x0400_00B0, 0x0400_00BC, 0x0400_00C8, 0x0400_00D4];

fn configure(gba: &mut Gba, ch: usize, src: u32, dst: u32, count: u16, control: u16) {
    let base = SAD[ch];
    gba.bus.write32(base, src);
    gba.bus.write32(base + 4, dst);
    gba.bus.write16(base + 8, count);
    gba.bus.write16(base + 10, control);
}

#[test]
fn immediate_dma_configured_through_the_bus_copies_memory() {
    let mut gba = spinning_gba();
    for i in 0..4u32 {
        gba.bus.write16(0x0200_0000 + i * 2, 0x1000 + i as u16);
    }

    // Enable, timing 0 (immediate), halfword units, 4 of them.
    configure(&mut gba, 0, 0x0200_0000, 0x0200_0100, 4, 0x8000);
    gba.step();

    for i in 0..4u32 {
        assert_eq!(
            gba.bus.read16(0x0200_0100 + i * 2),
            0x1000 + i as u16,
            "DMA0 never moved halfword {i}: the register writes never reached the controller"
        );
    }
}

#[test]
fn immediate_dma_clears_the_enable_bit_a_game_polls() {
    // The Yggdra Union hang: a THUMB loop at 0x08093030 polls DMA3CNT_H for
    // bit 15 to clear and never leaves it, because no transfer ever ran.
    // Same shape here in ARM:
    //   mov  r0, #0x04000000
    //   add  r0, r0, #0xDE      ; DMA3CNT_H
    //   ldrh r1, [r0]
    //   tst  r1, #0x8000
    //   bne  -3                 ; back to the ldrh
    //   mov  r2, #0xAB          ; escaped
    //   b    .
    let mut gba = Gba::new();
    gba.load_rom(&rom(&[
        0xE3A0_0301,
        0xE280_00DE,
        0xE1D0_10B0,
        0xE311_0C80,
        0x1AFF_FFFC,
        0xE3A0_20AB,
        0xEAFF_FFFE,
    ]))
    .expect("ROM should load");

    configure(&mut gba, 3, 0x0200_0000, 0x0200_0100, 4, 0x8000);
    gba.run_frame();

    assert_eq!(
        gba.bus.read16(0x0400_00DE) & 0x8000,
        0,
        "DMA3CNT_H bit 15 never cleared, so a game polling for completion hangs"
    );
    assert_eq!(
        gba.cpu.registers[2], 0xAB,
        "the poll loop never exited"
    );
}

#[test]
fn hblank_dma_repeats_across_scanlines() {
    let mut gba = spinning_gba();
    gba.bus.write16(0x0200_0000, 0xBEEF);

    // Enable | repeat | timing 2 (HBlank) | source fixed (bits 7-8) and dest
    // increment (bits 5-6), one halfword each.
    let control: u16 = 0x8000 | 0x0200 | (2 << 12) | (2 << 7);
    configure(&mut gba, 0, 0x0200_0000, 0x0200_0100, 1, control);
    gba.run_frame();

    for i in 0..3u32 {
        assert_eq!(
            gba.bus.read16(0x0200_0100 + i * 2),
            0xBEEF,
            "HBlank DMA did not fire on scanline {i}"
        );
    }
}

#[test]
fn vblank_dma_fires_once_per_frame() {
    let mut gba = spinning_gba();
    gba.bus.write16(0x0200_0000, 0xCAFE);
    gba.bus.write16(0x0200_0002, 0xF00D);

    // Enable | timing 1 (VBlank), two halfwords, no repeat.
    configure(&mut gba, 1, 0x0200_0000, 0x0200_0100, 2, 0x8000 | (1 << 12));
    gba.run_frame();

    assert_eq!(gba.bus.read16(0x0200_0100), 0xCAFE, "VBlank DMA never fired");
    assert_eq!(gba.bus.read16(0x0200_0102), 0xF00D);
    assert_eq!(
        gba.bus.read16(0x0400_00C6) & 0x8000,
        0,
        "a non-repeating VBlank DMA must clear its enable bit"
    );
}

#[test]
fn count_of_zero_means_the_maximum_length() {
    // GBATEK, GBA DMA Transfers: the word count is 14 bits for DMA0-2 and 16
    // for DMA3, and "the number of units is 0x4000 (or 0x10000) when zero".
    let mut gba = spinning_gba();
    gba.bus.write16(0x0200_0000, 0x1234);
    gba.bus.write16(0x0200_7FFE, 0x5678); // last halfword of a 0x4000-unit run

    configure(&mut gba, 0, 0x0200_0000, 0x0201_0000, 0, 0x8000);
    gba.step();

    assert_eq!(
        gba.bus.read16(0x0201_0000),
        0x1234,
        "a count of 0 must mean the maximum length, not a no-op"
    );
    assert_eq!(gba.bus.read16(0x0201_7FFE), 0x5678);
}
