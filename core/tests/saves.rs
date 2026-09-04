//! Battery-backed cartridge saves.
//!
//! These start as the reproduction for a bug found by `gba_suite_save_*`:
//! `Cartridge::save_read`/`save_write` had no callers and `MemoryBus` had no
//! arm for 0x0E000000, so every in-game save write was silently discarded.

use geebeeayy_core::Gba;

const SAVE_BASE: u32 = 0x0E00_0000;

/// A minimal ROM whose header carries the marker the save-type detector looks
/// for. Padded past the 0xC0-byte header the cartridge loader requires.
fn rom_with(marker: &str) -> Vec<u8> {
    let mut data = vec![0u8; 0x400];
    data[0x100..0x100 + marker.len()].copy_from_slice(marker.as_bytes());
    data
}

fn gba_with(marker: &str) -> Gba {
    let mut gba = Gba::new();
    gba.load_rom(&rom_with(marker)).expect("ROM should load");
    gba
}

#[test]
fn sram_writes_reach_the_cartridge() {
    let mut gba = gba_with("SRAM_V100");
    gba.bus.write8(SAVE_BASE, 0x5A);
    assert_eq!(
        gba.bus.read8(SAVE_BASE),
        0x5A,
        "a write to the save region never reached the cartridge"
    );
}

#[test]
fn uninitialised_save_memory_reads_ff() {
    // gba-suite's save ROMs all begin with this check: erased SRAM/Flash is
    // 0xFF, not 0x00.
    let gba = gba_with("SRAM_V100");
    assert_eq!(gba.bus.read8(SAVE_BASE), 0xFF);
    assert_eq!(gba.bus.read8(SAVE_BASE + 0x7FFF), 0xFF);
}

#[test]
fn a_cartridge_with_no_save_type_reads_ff() {
    let gba = gba_with("NOTHING");
    assert_eq!(gba.bus.read8(SAVE_BASE), 0xFF);
}

#[test]
fn save_data_round_trips_through_the_cartridge() {
    let mut gba = gba_with("SRAM_V100");
    gba.bus.write8(SAVE_BASE + 0x10, 0xAB);
    let saved = gba.save_data().expect("an SRAM cart has save data");
    assert_eq!(saved[0x10], 0xAB);

    let mut restored = gba_with("SRAM_V100");
    restored.load_save(&saved);
    assert_eq!(restored.bus.read8(SAVE_BASE + 0x10), 0xAB);
}

#[test]
fn writing_the_save_region_marks_it_dirty() {
    let mut gba = gba_with("SRAM_V100");
    assert!(!gba.take_save_dirty(), "a fresh cart is not dirty");
    gba.bus.write8(SAVE_BASE, 1);
    assert!(
        gba.take_save_dirty(),
        "a save write must set the dirty flag"
    );
    assert!(!gba.take_save_dirty(), "taking the flag must clear it");
}

#[test]
fn flash_chip_erase_needs_the_second_unlock() {
    // The full sequence is AA,55,80,AA,55,10. Consuming the command straight
    // after 0x80 silently skips the erase, which is how a game ends up seeing
    // its old save after formatting.
    let mut gba = gba_with("FLASH_V123");
    let cmd = |g: &mut Gba, v: u8| g.bus.write8(SAVE_BASE + 0x5555, v);

    // Program a byte to something other than the erased value.
    cmd(&mut gba, 0xAA);
    cmd(&mut gba, 0x55);
    cmd(&mut gba, 0xA0);
    gba.bus.write8(SAVE_BASE, 0x00);
    assert_eq!(gba.bus.read8(SAVE_BASE), 0x00);

    // Erase the chip.
    cmd(&mut gba, 0xAA);
    cmd(&mut gba, 0x55);
    cmd(&mut gba, 0x80);
    cmd(&mut gba, 0xAA);
    cmd(&mut gba, 0x55);
    cmd(&mut gba, 0x10);
    assert_eq!(gba.bus.read8(SAVE_BASE), 0xFF, "chip erase did not run");
}

#[test]
fn the_save_region_has_an_eight_bit_databus() {
    // GBATEK, GBA Cart Backup SRAM/FRAM: "the databus is restricted to 8 bits".
    // A halfword read returns the one byte replicated, and a halfword write
    // delivers only the byte selected by the accessed address.
    let mut gba = gba_with("SRAM_V100");
    gba.bus.write8(SAVE_BASE, 0x01);
    assert_eq!(gba.bus.read16(SAVE_BASE), 0x0101);
    assert_eq!(gba.bus.read32(SAVE_BASE), 0x0101_0101);

    gba.bus.write16(SAVE_BASE + 0x20, 0xAABB);
    assert_eq!(gba.bus.read8(SAVE_BASE + 0x20), 0xBB);
    gba.bus.write16(SAVE_BASE + 0x21, 0xAABB);
    assert_eq!(gba.bus.read8(SAVE_BASE + 0x21), 0xAA);
}

// --- Save states: bytes in, bytes out ---------------------------------------

#[test]
fn save_state_round_trips_through_bytes() {
    // The FFI hands the frontend a byte array, not an opaque handle, so a
    // state can actually reach a file. This is the shape the C ABI exposes.
    let mut gba = gba_with("SRAM_V100");
    gba.cpu.registers[0] = 0xDEAD;
    gba.bus.write8(SAVE_BASE, 0x77);
    let bytes = gba.save_state().data;

    gba.cpu.registers[0] = 0;
    let restored = geebeeayy_core::savestate::SaveState { data: bytes };
    gba.load_state(&restored).expect("state should restore");
    assert_eq!(gba.cpu.registers[0], 0xDEAD);
}

#[test]
fn a_truncated_save_state_is_rejected() {
    let mut gba = gba_with("SRAM_V100");
    let mut bytes = gba.save_state().data;
    bytes.truncate(bytes.len() / 2);
    let state = geebeeayy_core::savestate::SaveState { data: bytes };
    assert!(
        gba.load_state(&state).is_err(),
        "a truncated state must be rejected, not read short"
    );
}

#[test]
fn save_state_round_trip_reproduces_the_machine() {
    // The old test asserted only on registers[0] and would have passed against
    // a `restore` that did nothing else. This runs the machine, snapshots,
    // diverges, restores, and requires the same frames to come back.
    let mut gba = Gba::new();
    gba.load_rom(&rom_with("SRAM_V100")).unwrap();
    for _ in 0..3 {
        gba.run_frame();
    }

    let state = gba.save_state();
    for _ in 0..3 {
        gba.run_frame();
    }
    let expected: Vec<u8> = gba.frame_buffer().to_vec();
    let cycles_expected = gba.cycles;

    // Diverge hard, then restore and replay the same three frames.
    for _ in 0..10 {
        gba.run_frame();
    }
    gba.load_state(&state).expect("state should restore");
    for _ in 0..3 {
        gba.run_frame();
    }

    assert_eq!(
        gba.cycles, cycles_expected,
        "cycle count diverged after restore"
    );
    assert!(
        gba.frame_buffer().to_vec() == expected,
        "the frames after a restore differ from the frames before it"
    );
}

#[test]
fn a_rejected_save_state_leaves_the_machine_untouched() {
    // `restore` parses straight into the live machine, so a bad file used to
    // leave a hybrid of two states behind while reporting failure.
    let mut gba = gba_with("SRAM_V100");
    gba.run_frame();
    let before = gba.save_state().data;

    let mut corrupt = before.clone();
    corrupt.truncate(corrupt.len() - 64);
    let bad = geebeeayy_core::savestate::SaveState { data: corrupt };
    assert!(gba.load_state(&bad).is_err());

    assert_eq!(
        gba.save_state().data,
        before,
        "a rejected state must roll back, not half-apply"
    );
}

#[test]
fn flash_reports_its_chip_id() {
    // GBATEK, GBA Cart Backup Flash ROM: AA,55,90 enters ID mode, then
    // man = [0E000000], dev = [0E000001], with the published ID written
    // MSB = device, LSB = manufacturer. AA,55,F0 leaves ID mode.
    //
    // A game that probes this and gets flash contents back concludes the cart
    // has no save chip and refuses to save at all.
    for (marker, id) in [("FLASH_V123", 0x1B32u16), ("FLASH1M_V102", 0x1362u16)] {
        let mut gba = gba_with(marker);
        let cmd = |g: &mut Gba, v: u8| g.bus.write8(SAVE_BASE + 0x5555, v);

        // Data reads before entering ID mode are erased flash.
        assert_eq!(gba.bus.read8(SAVE_BASE), 0xFF);

        cmd(&mut gba, 0xAA);
        cmd(&mut gba, 0x55);
        cmd(&mut gba, 0x90);
        assert_eq!(
            gba.bus.read8(SAVE_BASE),
            (id & 0xFF) as u8,
            "manufacturer byte"
        );
        assert_eq!(gba.bus.read8(SAVE_BASE + 1), (id >> 8) as u8, "device byte");

        cmd(&mut gba, 0xAA);
        cmd(&mut gba, 0x55);
        cmd(&mut gba, 0xF0);
        assert_eq!(
            gba.bus.read8(SAVE_BASE),
            0xFF,
            "ID mode was never terminated"
        );
    }
}

#[test]
fn save_state_carries_the_apu() {
    // Without the APU in the format, every channel came back silent after a
    // load and a sustained note simply stopped.
    //
    // This asserts on the APU's own serialised state rather than on generated
    // audio, because the APU register map is currently broken in a way that
    // prevents any channel from being triggered at all - see ROADMAP.md. That
    // makes an audio-output test unable to tell a working restore from a
    // broken one. What this does prove is the part that was actually built:
    // the snapshot round-trips, and a changed APU produces a different
    // snapshot, so the two directions are symmetric.
    let mut gba = gba_with("SRAM_V100");
    gba.bus.write16(0x0400_0084, 0x0080); // master enable
    gba.bus.write16(0x0400_0062, 0xF780);
    gba.run_frame();

    let state = gba.save_state();
    let expected = gba.apu.snapshot();

    // Move the APU somewhere else and confirm that actually changed something.
    gba.bus.write16(0x0400_0084, 0x0000);
    gba.bus.write16(0x0400_0072, 0x00FF);
    gba.run_frame();
    assert_ne!(
        gba.apu.snapshot(),
        expected,
        "the test needs the APU state to actually diverge"
    );

    gba.load_state(&state).expect("state should restore");
    assert_eq!(
        gba.apu.snapshot(),
        expected,
        "the APU did not come back from the save state"
    );
}

#[test]
fn a_psg_channel_can_actually_be_triggered() {
    // Before the register map was rewritten at 16-bit granularity this was
    // impossible: 0x65, where SOUND1CNT_X's trigger bit lives, had no handler
    // at all, so no PSG channel could ever start.
    //
    // GBATEK, Sound Channel 1: SOUND1CNT_H bits 12-15 are the initial envelope
    // volume and 6-7 the duty; SOUND1CNT_X bits 0-10 are the frequency and bit
    // 15 restarts the sound. SOUNDCNT_X bit 7 is the master enable.
    let mut gba = gba_with("SRAM_V100");
    gba.bus.write16(0x0400_0084, 0x0080); // master enable
    gba.bus.write16(0x0400_0080, 0x0077); // both sides, full volume
    gba.bus.write16(0x0400_0062, 0xF080); // volume 15, 50% duty
    gba.bus.write16(0x0400_0064, 0x8400); // frequency, restart
    gba.run_frame();

    assert!(
        gba.apu_samples().iter().any(|&s| s != 0.0),
        "channel 1 produced silence after being triggered"
    );
}

#[test]
fn the_master_enable_silences_the_apu() {
    // SOUNDCNT_X bit 7 clear means "both PSG and FIFO sounds are disabled"
    // (GBATEK). This register was not routed to the APU at all.
    let mut gba = gba_with("SRAM_V100");
    gba.bus.write16(0x0400_0080, 0x0077);
    gba.bus.write16(0x0400_0062, 0xF080);
    gba.bus.write16(0x0400_0064, 0x8400);
    gba.bus.write16(0x0400_0084, 0x0000); // master enable OFF
    gba.run_frame();

    assert!(
        gba.apu_samples().iter().all(|&s| s == 0.0),
        "sound played with the master enable off"
    );
}

// --- EEPROM: a serial device, not addressable memory -----------------------

/// Drive the EEPROM's serial line the way a game's DMA would.
fn eeprom_send(gba: &mut Gba, bits: &[bool]) {
    for &b in bits {
        gba.bus.write16(0x0D00_0000, b as u16);
    }
}

fn eeprom_recv(gba: &mut Gba, count: usize) -> Vec<bool> {
    (0..count)
        .map(|_| gba.bus.read16_mut(0x0D00_0000) & 1 == 1)
        .collect()
}

/// MSB-first bits of `value`, `n` wide.
fn bits_of(value: u64, n: usize) -> Vec<bool> {
    (0..n).map(|i| (value >> (n - 1 - i)) & 1 == 1).collect()
}

#[test]
fn eeprom_round_trips_a_64_bit_block() {
    // GBATEK, GBA Cart Backup EEPROM. Write: "10", 6 address bits, 64 data
    // bits, "0". Read: "11", 6 address bits, "0", then 68 bits back of which
    // the first 4 are discarded.
    //
    // This is a serial device driven by DMA, not addressable RAM - the old
    // implementation treated 0x0D000000 as bytes and no real game would have
    // read back what it wrote.
    let mut gba = gba_with("EEPROM_V122");
    let payload: u64 = 0x0123_4567_89AB_CDEF;

    let mut write = vec![true, false]; // "10" = write
    write.extend(bits_of(5, 6)); // block 5
    write.extend(bits_of(payload, 64));
    write.push(false);
    eeprom_send(&mut gba, &write);

    let mut read = vec![true, true]; // "11" = read
    read.extend(bits_of(5, 6));
    read.push(false);
    eeprom_send(&mut gba, &read);

    let out = eeprom_recv(&mut gba, 68);
    let mut got: u64 = 0;
    for &b in &out[4..] {
        got = (got << 1) | b as u64;
    }
    assert_eq!(
        got, payload,
        "EEPROM did not return the block that was written"
    );
}

#[test]
fn eeprom_writes_mark_the_save_dirty() {
    let mut gba = gba_with("EEPROM_V122");
    assert!(!gba.take_save_dirty());

    let mut write = vec![true, false];
    write.extend(bits_of(0, 6));
    write.extend(bits_of(0xDEAD_BEEF_0000_0000, 64));
    write.push(false);
    eeprom_send(&mut gba, &write);

    assert!(
        gba.take_save_dirty(),
        "an EEPROM write must mark the save dirty"
    );
}

#[test]
fn eeprom_marker_is_detected() {
    let gba = gba_with("EEPROM_V122");
    assert!(
        gba.cartridge().uses_eeprom(),
        "the EEPROM_V marker was not detected, so the save type is wrong"
    );
}

/// GBATEK, GBA Cartridge Header: the complement check at 0xBD covers bytes
/// 0xA0 through 0xBC. The range used to start at 0x0A, sweeping in the
/// Nintendo logo and the entry point, so every commercial ROM reported a bad
/// checksum on load - which is only a warning, so nothing but this test says
/// the range is right.
#[test]
fn the_header_checksum_covers_only_0xa0_to_0xbc() {
    let mut rom = vec![0u8; 0x200];
    rom[0xA0..0xAC].copy_from_slice(b"CHECKSUMTEST");
    let before = geebeeayy_core::cart::header_checksum(&rom);

    // Junk anywhere below 0xA0 is outside the checked range and must not
    // change the answer.
    for (i, b) in rom[0x00..0xA0].iter_mut().enumerate() {
        *b = (i as u8).wrapping_mul(7).wrapping_add(3);
    }
    assert_eq!(
        geebeeayy_core::cart::header_checksum(&rom),
        before,
        "bytes below 0xA0 must not be part of the header checksum"
    );

    // A byte inside the range must.
    rom[0xB0] = 0x5A;
    assert_ne!(
        geebeeayy_core::cart::header_checksum(&rom),
        before,
        "bytes in 0xA0..=0xBC must be part of the header checksum"
    );

    // And a header whose complement matches loads without complaint.
    rom[0xBD] = geebeeayy_core::cart::header_checksum(&rom);
    let mut gba = Gba::new();
    gba.load_rom(&rom).expect("ROM should load");
    assert_eq!(gba.cartridge().title(), "CHECKSUMTEST");
}

/// A ROM larger than 16 MB, carrying the EEPROM marker, with `filler` written
/// at the ROM offset that the WS2 mirror puts at 0x0D000000.
fn big_eeprom_rom(filler: u8) -> Vec<u8> {
    let mut data = vec![0u8; 17 * 1024 * 1024];
    data[0x100..0x100 + 11].copy_from_slice(b"EEPROM_V126");
    data[0x0100_0000] = filler;
    data
}

/// GBATEK, *GBA Cart Backup EEPROM*: on a cartridge over 16 MB the chip
/// answers only at 0x0DFFFF00-0x0DFFFFFF, because the ROM itself reaches the
/// rest of that range through the WS2 mirror.
///
/// This is the bug that stopped Yggdra Union (32 MB, EEPROM_V126) saving: the
/// window covered all of 0x0D000000-0x0DFFFFFF, so the upper half of the
/// cartridge read back as EEPROM bits and every command the game sent landed
/// in an already-desynchronised state machine.
#[test]
fn a_large_rom_keeps_its_ws2_mirror_outside_the_eeprom_window() {
    let mut gba = Gba::new();
    gba.load_rom(&big_eeprom_rom(0xA5))
        .expect("ROM should load");
    assert!(gba.cartridge().uses_eeprom());

    assert_eq!(
        gba.bus.read8(0x0D00_0000),
        0xA5,
        "0x0D000000 on a 17 MB cart is the ROM's WS2 mirror, not the EEPROM"
    );
    assert_eq!(
        gba.bus.read16_mut(0x0D00_0000),
        0x00A5,
        "a halfword read of the mirror must not clock the EEPROM either"
    );

    // The chip is still reachable at the top of the window: idle EEPROM
    // reports ready.
    assert_eq!(gba.bus.read16_mut(0x0DFF_FF00) & 1, 1);
}

/// A 16 MB-or-smaller cartridge keeps the whole region, which is where every
/// small EEPROM game talks to its chip.
#[test]
fn a_small_rom_keeps_the_whole_eeprom_window() {
    let mut gba = gba_with("EEPROM_V122");
    assert_eq!(gba.bus.read16_mut(0x0D00_0000) & 1, 1);
}

/// The address width comes from the DMA's length, not from the chip size.
///
/// A game detects its EEPROM by trying a 6-bit command first, so an 8 KB part
/// has to answer 6-bit commands during detection. Deriving the width from the
/// chip size alone makes every one of those commands land at the wrong phase.
#[test]
fn the_eeprom_address_width_follows_the_dma_length() {
    let mut gba = Gba::new();
    gba.load_rom(&big_eeprom_rom(0)).expect("ROM should load");

    let window = 0x0DFF_FF00;
    let payload: u64 = 0xCAFE_BABE_1234_5678;

    // 73 halfwords = 2 opcode + 6 address + 64 data + 1 stop: a 6-bit write.
    let mut write = vec![true, false];
    write.extend(bits_of(9, 6));
    write.extend(bits_of(payload, 64));
    write.push(false);
    assert_eq!(write.len(), 73);
    gba.bus.eeprom_begin_dma(window, write.len());
    for &b in &write {
        gba.bus.write16(window, b as u16);
    }

    // 9 halfwords = 2 + 6 + 1: a 6-bit set-read-address.
    let mut read = vec![true, true];
    read.extend(bits_of(9, 6));
    read.push(false);
    assert_eq!(read.len(), 9);
    gba.bus.eeprom_begin_dma(window, read.len());
    for &b in &read {
        gba.bus.write16(window, b as u16);
    }

    gba.bus.eeprom_begin_dma(window, 68);
    let out: Vec<bool> = (0..68)
        .map(|_| gba.bus.read16_mut(window) & 1 == 1)
        .collect();
    let mut got: u64 = 0;
    for &b in &out[4..] {
        got = (got << 1) | b as u64;
    }
    assert_eq!(
        got, payload,
        "an 8 KB chip must still answer the 6-bit commands a game probes with"
    );
}

/// The length fields inside a save state come off disk unvalidated. A corrupt
/// one used to be handed straight to `vec![0u8; len]`, so a state claiming
/// 0xFFFFFFFF bytes asked for a 4 GB allocation and aborted the process
/// instead of returning an error.
#[test]
fn a_save_state_with_an_absurd_length_field_is_rejected_not_allocated() {
    let mut gba = Gba::new();
    gba.load_rom(&vec![0u8; 0x400]).expect("ROM should load");
    let mut bytes = gba.save_state().data;

    // Tail layout is `save_len: u32` then `cycles: u64`; with no cartridge
    // save present, `save_len` is zero, which pins the offset.
    let at = bytes.len() - 12;
    assert_eq!(
        &bytes[at..at + 4],
        &[0, 0, 0, 0],
        "expected the save-length field at len-12"
    );
    bytes[at..at + 4].copy_from_slice(&0xFFFF_FFFFu32.to_le_bytes());

    let state = geebeeayy_core::savestate::SaveState { data: bytes };
    assert!(
        state.restore(&mut gba).is_err(),
        "a length past the end of the blob must be an error, not an allocation"
    );
}

/// Wave RAM is two banks of 16 bytes, and GBATEK is explicit that the CPU
/// window at 0x04000090 addresses the bank that is **not** playing: "The
/// currently selected Bank Number (Bit 6) will be played back, while
/// reading/writing to/from wave RAM will address the other (not selected)
/// bank." Writes used to land at a fixed offset 0 whatever the bank bit said,
/// so the upper 16 bytes were dead and only one waveform ever existed.
#[test]
fn wave_ram_writes_reach_the_bank_that_is_not_playing() {
    /// Where in the APU snapshot a marker written through the register window
    /// ends up, with `select` as the bank bit of SOUND3CNT_L.
    fn marker_offset(select: u16) -> usize {
        let mut gba = gba_with("SRAM_V100");
        gba.bus.write16(0x0400_0070, select);
        gba.bus.write16(0x0400_0090, 0xABCD);
        // Sound register writes are queued and applied by the frame, not at
        // the store, so nothing reaches the APU until this runs.
        gba.run_frame();
        let snap = gba.apu.snapshot();
        snap.iter()
            .position(|&b| b == 0xCD)
            .expect("the marker never reached wave RAM at all")
    }

    let bank1 = marker_offset(0x0000); // bank 0 plays, so the CPU sees bank 1
    let bank0 = marker_offset(0x0040); // bank 1 plays, so the CPU sees bank 0

    assert_eq!(
        bank1 - bank0,
        16,
        "the two bank selections must write 16 bytes apart; landing at the \
         same offset means the bank bit is ignored and half of wave RAM is dead"
    );
}

/// SOUND3CNT_L bit 5 is the Wave RAM Dimension, and it was not read at all -
/// 64-digit mode did not exist, so the second bank never sounded.
#[test]
fn two_bank_mode_plays_a_different_waveform_than_one_bank_mode() {
    fn play(dimension: u16) -> Vec<f32> {
        let mut gba = gba_with("SRAM_V100");
        gba.bus.write16(0x0400_0084, 0x0080);
        gba.bus.write16(0x0400_0080, 0x0077);

        // Bank 0 selected, so the window writes bank 1: fill it flat.
        gba.bus.write16(0x0400_0070, 0x0000);
        for i in (0..16u32).step_by(2) {
            gba.bus.write16(0x0400_0090 + i, 0x0000);
        }
        // Bank 1 selected, so the window writes bank 0: fill it loud.
        gba.bus.write16(0x0400_0070, 0x0040);
        for i in (0..16u32).step_by(2) {
            gba.bus.write16(0x0400_0090 + i, 0xFFFF);
        }
        // Play bank 0 (the loud one), with the dimension under test.
        gba.bus.write16(0x0400_0070, dimension | 0x0080);
        gba.bus.write16(0x0400_0072, 0x2000);
        gba.bus.write16(0x0400_0074, 0x8400);
        gba.run_frame();
        gba.apu_samples().to_vec()
    }

    let one_bank = play(0x0000);
    let two_banks = play(0x0020);
    assert_ne!(
        one_bank, two_banks,
        "the dimension bit had no effect, so 64-digit mode is still missing"
    );
}
