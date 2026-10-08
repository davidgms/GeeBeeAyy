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
    assert_eq!(gba.cpu.registers[2], 0xAB, "the poll loop never exited");
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

    assert_eq!(
        gba.bus.read16(0x0200_0100),
        0xCAFE,
        "VBlank DMA never fired"
    );
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

#[test]
fn dma_sound_plays_at_the_rate_its_timer_overflows() {
    // Direct Sound is a timer, a FIFO and a DMA channel working together:
    // each timer overflow pops one byte from the FIFO into a latch the mixer
    // holds until the next overflow, and the DMA tops the FIFO up when it
    // falls half empty.
    //
    // `Apu::on_timer_overflow` used to be an empty stub while the mixer popped
    // the FIFO itself, once per output sample. Sound then played at the output
    // rate rather than the rate the game asked for, and drained far faster
    // than it was refilled. Separately the timers were never wired to the bus
    // at all, so nothing overflowed in the first place.
    let mut gba = spinning_gba();

    // Alternating +/- bytes: every FIFO byte flips the sign of the output, so
    // counting sign changes counts FIFO pops.
    for i in 0..0x800u32 {
        gba.bus
            .write8(0x0300_0000 + i, if i % 2 == 0 { 0x40 } else { 0xC0 });
    }

    // One overflow every 8 output samples: 8 * 964 cycles, prescaler 1.
    let period = 8 * 964u32;
    gba.bus.write16(0x0400_0100, (0x1_0000 - period) as u16);
    gba.bus.write16(0x0400_0102, 0x0080);

    // Master on; DMA A at full volume, both speakers, timer 0.
    gba.bus.write16(0x0400_0084, 0x0080);
    gba.bus.write16(0x0400_0082, 0x0304);

    // DMA1 -> FIFO A, 32-bit, repeat, special timing.
    gba.bus.write32(0x0400_00BC, 0x0300_0000);
    gba.bus.write32(0x0400_00C0, 0x0400_00A0);
    gba.bus.write16(0x0400_00C4, 4);
    gba.bus.write16(0x0400_00C6, 0xB600);

    gba.run_frame();
    let samples = gba.apu_samples();
    let non_zero = samples.iter().filter(|&&s| s != 0.0).count();
    assert!(
        non_zero > samples.len() * 9 / 10,
        "the FIFO ran dry: only {non_zero} of {} samples carry signal",
        samples.len()
    );

    let flips = samples
        .windows(2)
        .filter(|w| (w[0] < 0.0) != (w[1] < 0.0))
        .count();
    // 280896 cycles a frame at one overflow per 7712 is ~36 pops. Popping per
    // output sample instead would give ~291.
    assert!(
        (20..80).contains(&flips),
        "DMA sound played at the output rate, not the timer's: {flips} sign changes in {} samples",
        samples.len()
    );
}

/// FIFO DMA never raised its interrupt. `do_sound_transfer` is the only path a
/// sound DMA takes, and the IF request lives in `do_transfer`, which that path
/// never reaches - so a game swapping its audio double-buffer from the DMA1 or
/// DMA2 interrupt waited on one that could not arrive.
#[test]
fn sound_dma_raises_its_interrupt_when_the_channel_asked_for_one() {
    let mut gba = spinning_gba();

    // DMA1: source in EWRAM, destination FIFO A, timing 3 (special), repeat,
    // 32-bit, IRQ on end. DMACNT_H bit 14 is the IRQ enable.
    gba.bus.write32(0x0400_00BC, 0x0200_0000); // SAD
    gba.bus.write32(0x0400_00C0, 0x0400_00A0); // DAD = FIFO A
    gba.bus.write16(0x0400_00C4, 0); // count (ignored for FIFO)
                                     // enable | repeat | 32-bit | timing 3 | IRQ
    gba.bus
        .write16(0x0400_00C6, 0x8000 | 0x0200 | 0x0400 | 0x3000 | 0x4000);

    // IE: DMA1 is IF bit 9.
    gba.bus.write16(0x0400_0200, 0x0200);
    gba.bus.write16(0x0400_0202, 0xFFFF); // clear IF (write-1-to-clear)

    assert_eq!(
        gba.bus.read16(0x0400_0202) & 0x0200,
        0,
        "IF bit 9 should start clear"
    );

    gba.run_frame();

    assert_ne!(
        gba.bus.read16(0x0400_0202) & 0x0200,
        0,
        "a FIFO transfer with IRQ enabled never set IF bit 9"
    );
}

// --- Source rules from the mGBA suite's DMA tests (src/dma.c) ----------------
//
// Its hardware-recorded expectations settle two things GBATEK does not spell
// out: a gamepak ROM source always increments, and a DMA read of the BIOS or
// of anything below EWRAM returns the channel's own last transferred value.

const ENABLE: u16 = 0x8000;
const WORD: u16 = 0x0400;
const SRC_DEC: u16 = 1 << 7;
const SRC_FIXED: u16 = 2 << 7;
const DST_FIXED: u16 = 2 << 5;

/// The suite's `srcR` table, 0xDEADBEEC.. 0xDEADBEF2, at ROM offset 0x100.
const ROM_TABLE: u32 = 0x0800_0100;

/// A spinning machine whose ROM carries `srcR` at [`ROM_TABLE`].
fn gba_with_rom_table() -> Gba {
    let mut data = rom(&[0xEAFF_FFFE]);
    for i in 0..7u32 {
        let at = 0x100 + i as usize * 4;
        data[at..at + 4].copy_from_slice(&(0xDEAD_BEEC + i).to_le_bytes());
    }
    let mut gba = Gba::new();
    gba.load_rom(&data).expect("ROM should load");
    gba
}

/// Configure a channel and let the next step start it.
fn transfer(gba: &mut Gba, ch: usize, src: u32, dst: u32, count: u16, control: u16) {
    configure(gba, ch, src, dst, count, control);
    gba.step();
}

/// Suite "N Imm W =ROM/=IWRAM" and "N Imm W -ROM/=IWRAM": four words from
/// `&srcR[3]` into one fixed word end on 0xDEADBEF2 - the source walked up
/// through the table whatever its address control said.
#[test]
fn a_rom_source_increments_even_when_fixed_or_decrementing() {
    for ch in 1..4 {
        for (mode, name) in [(SRC_FIXED, "fixed"), (SRC_DEC, "decrementing")] {
            let mut gba = gba_with_rom_table();
            let control = ENABLE | WORD | mode | DST_FIXED;
            transfer(&mut gba, ch, ROM_TABLE + 12, 0x0300_0000, 4, control);
            assert_eq!(
                gba.bus.read32(0x0300_0000),
                0xDEAD_BEF2,
                "DMA{ch}, {name} ROM source: the address did not increment"
            );
        }
    }
}

/// Suite "N Imm H =ROM/=IWRAM": halfwords BEEF, DEAD, BEF0, DEAD.
#[test]
fn a_fixed_rom_source_increments_in_halfwords_too() {
    let mut gba = gba_with_rom_table();
    transfer(
        &mut gba,
        3,
        ROM_TABLE + 12,
        0x0300_0000,
        4,
        ENABLE | SRC_FIXED | DST_FIXED,
    );
    assert_eq!(gba.bus.read16(0x0300_0000), 0xDEAD);
}

/// Suite "N Imm H/W =BIOS/...", "R+0x10": a DMA cannot read the BIOS (nor
/// anything else below EWRAM) and gets its last transferred value instead.
/// A halfword is latched on both halves. GBATEK's BIOS read protection
/// ("the most recent successfully fetched BIOS opcode") describes the CPU;
/// the suite shows a DMA sees its own latch.
#[test]
fn a_dma_read_below_ewram_returns_the_channels_last_value() {
    for src in [0x0000_000C, 0x0000_0010, 0x0000_4000, 0x0100_0000] {
        let mut gba = spinning_gba();
        gba.bus.write16(0x0200_0000, 0xBABE);
        transfer(&mut gba, 3, 0x0200_0000, 0x0200_0100, 1, ENABLE);

        transfer(&mut gba, 3, src, 0x0200_0200, 2, ENABLE | WORD);
        assert_eq!(
            gba.bus.read32(0x0200_0200),
            0xBABE_BABE,
            "word from {src:#X}"
        );
        assert_eq!(
            gba.bus.read32(0x0200_0204),
            0xBABE_BABE,
            "word from {src:#X}"
        );

        transfer(&mut gba, 3, src, 0x0200_0300, 1, ENABLE);
        assert_eq!(
            gba.bus.read16(0x0200_0300),
            0xBABE,
            "halfword from {src:#X}"
        );
    }
}

/// Suite "0 Imm W =ROM/...": DMA0's source is 27 bits, so a ROM address
/// lands below EWRAM and the channel reads its latch.
#[test]
fn dma0_cannot_read_rom_and_returns_its_latch() {
    let mut gba = gba_with_rom_table();
    gba.bus.write16(0x0300_0000, 0xCAFE);
    transfer(&mut gba, 0, 0x0300_0000, 0x0300_0100, 1, ENABLE);
    transfer(&mut gba, 0, ROM_TABLE, 0x0300_0200, 1, ENABLE | WORD);
    assert_eq!(gba.bus.read32(0x0300_0200), 0xCAFE_CAFE);
}

/// The suite skips "0 Imm W R+0x10" because "DMA 0 R+0x10 would latch based
/// on the last DMA 0 test": the latch belongs to the channel, not the bus.
#[test]
fn each_channel_keeps_its_own_latch() {
    let mut gba = spinning_gba();
    gba.bus.write16(0x0200_0000, 0x1111);
    gba.bus.write16(0x0200_0002, 0x2222);
    transfer(&mut gba, 1, 0x0200_0000, 0x0200_0100, 1, ENABLE);
    transfer(&mut gba, 2, 0x0200_0002, 0x0200_0100, 1, ENABLE);
    transfer(&mut gba, 1, 0x0000_0010, 0x0200_0200, 1, ENABLE | WORD);
    assert_eq!(gba.bus.read32(0x0200_0200), 0x1111_1111);
}

/// A halfword written from a word latch takes the lane its destination sits
/// on, as a 16-bit store of the 32-bit bus value does.
#[test]
fn a_halfword_from_a_word_latch_takes_the_destination_lane() {
    let mut gba = gba_with_rom_table();
    transfer(&mut gba, 3, ROM_TABLE + 24, 0x0200_0000, 1, ENABLE | WORD);
    transfer(&mut gba, 3, 0x0000_0010, 0x0200_0100, 2, ENABLE);
    assert_eq!(gba.bus.read16(0x0200_0100), 0xBEF2);
    assert_eq!(gba.bus.read16(0x0200_0102), 0xDEAD);
}

/// mGBA suite, Memory tests "SRAM load/store | DMAn 32 (unaligned k)": a DMA
/// drops the low address bits its unit width cannot address. Only the 8-bit
/// SRAM bus shows it, because there the CPU's own wide access keeps them.
#[test]
fn a_dma_aligns_its_addresses_to_the_unit_width() {
    let mut data = rom(&[0xEAFF_FFFE]);
    data[0x100..0x109].copy_from_slice(b"SRAM_V113");
    let mut gba = Gba::new();
    gba.load_rom(&data).expect("ROM should load");
    gba.bus.write8(0x0E00_0000, 0x47);
    gba.bus.write8(0x0E00_0001, 0x61);

    transfer(&mut gba, 3, 0x0E00_0001, 0x0200_0000, 1, ENABLE | WORD);
    assert_eq!(gba.bus.read32(0x0200_0000), 0x4747_4747, "word source");
    transfer(&mut gba, 3, 0x0E00_0001, 0x0200_0100, 1, ENABLE);
    assert_eq!(gba.bus.read16(0x0200_0100), 0x4747, "halfword source");

    gba.bus.write32(0x0200_0200, 0x0000_66D8);
    transfer(&mut gba, 3, 0x0200_0200, 0x0E00_0011, 1, ENABLE | WORD);
    assert_eq!(gba.bus.read8(0x0E00_0010), 0xD8, "word destination");
}

/// A repeating DMA reloads its unit count from DMAxCNT_L on every repeat, so
/// a count written after the enable edge applies from the second transfer on
/// (mGBA suite, Misc. edge cases, "DMA count latching": hardware copies
/// src[0..2] on the first VBlank and src[2..6] on the second, leaving 1 then
/// 5 in a fixed destination). The count latched at the enable edge was being
/// reused instead, which gave 3.
#[test]
fn a_repeating_dma_reloads_its_count_from_the_register() {
    let mut gba = spinning_gba();
    for i in 0..32u32 {
        gba.bus.write16(0x0200_0000 + i * 2, i as u16);
    }
    // VBlank, repeat, 16-bit, source increment, destination fixed, count 2.
    configure(
        &mut gba,
        3,
        0x0200_0000,
        0x0200_0100,
        2,
        0x8000 | 0x1000 | 0x0200 | 0x0040,
    );
    // The enable edge latches the count at the end of the next instruction.
    gba.step();
    // Count 4 for the repeats; the running transfer keeps its latched 2.
    gba.bus.write16(0x0400_00DC, 4);
    gba.run_frame();
    assert_eq!(
        gba.bus.read16(0x0200_0100),
        1,
        "first VBlank copies two units"
    );
    gba.run_frame();
    assert_eq!(
        gba.bus.read16(0x0200_0100),
        5,
        "the repeat must copy CNT_L's four units, not the two latched at enable"
    );
}

// --- Stalls: what a transfer costs the CPU ------------------------------------

/// A sound DMA that refills the FIFO while the CPU is halted stops nothing
/// that runs, so the CPU owes it nothing once an interrupt wakes it. The
/// halted step cleared the pending stall *before* `post_tick` ran the FIFO
/// refills, so the refill of the waking step survived and was charged to the
/// first instruction after the wake: two IWRAM `mov`s cost 1 and 31.
#[test]
fn a_sound_dma_during_halt_is_not_charged_after_the_wake() {
    let mut gba = spinning_gba();
    for i in 0..2 {
        gba.bus.write32(0x0300_0000 + 4 * i, 0xE1A0_0000); // mov r0, r0
    }
    gba.cpu.registers[15] = 0x0300_0000;
    // DMA1: EWRAM -> FIFO A, 32-bit, repeat, timing 3. The FIFO starts
    // empty, so the halted step's `post_tick` refills it.
    configure(&mut gba, 1, 0x0200_0000, 0x0400_00A0, 4, 0xB600);
    // IE & IF already set: the halted step wakes the CPU (IME stays 0).
    gba.bus.io.ie = 0x0001;
    gba.bus.io.request_interrupt(0x0001);
    gba.bus.io.halt = true;

    gba.step();
    assert!(!gba.bus.io.halt, "IE & IF should have ended the HALT");
    assert_eq!(
        [gba.step(), gba.step()],
        [1, 1],
        "a transfer that ran during HALT was charged to the code after it"
    );
}

/// A DMA on the GamePak bus moves the cartridge's address counter, so the
/// CPU's next ROM opcode fetch the prefetch buffer does not hold restarts
/// with a non-sequential access. Nothing told the CPU, and that fetch was
/// charged S. THUMB code in ROM, prefetch off, WAITCNT 0: S is 3 cycles, N 5.
/// The transfer is enabled by a bus write rather than a CPU store, because a
/// store already makes the next fetch N on its own.
#[test]
fn the_opcode_fetch_after_a_gamepak_dma_is_non_sequential() {
    let mut gba = Gba::new();
    gba.load_rom(&vec![0u8; 0x400]).expect("ROM should load"); // lsls r0, r0, #0
    gba.bus.write16(0x0400_0204, 0x0000);
    gba.cpu.cpsr |= 0x20;
    gba.cpu.registers[15] = 0x0800_0100;
    gba.step();
    assert_eq!(gba.step(), 3, "a sequential THUMB fetch from WS0");

    // DMA3, ROM -> EWRAM, one halfword, immediate: it runs after the next
    // instruction and stalls the CPU there.
    configure(&mut gba, 3, 0x0800_0000, 0x0200_0000, 1, 0x8000);
    gba.step();
    assert_eq!(
        gba.step(),
        5,
        "the fetch after a GamePak DMA must be non-sequential"
    );
}
