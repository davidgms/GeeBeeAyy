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
    assert!(gba.take_save_dirty(), "a save write must set the dirty flag");
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

    assert_eq!(gba.cycles, cycles_expected, "cycle count diverged after restore");
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
        assert_eq!(gba.bus.read8(SAVE_BASE), (id & 0xFF) as u8, "manufacturer byte");
        assert_eq!(gba.bus.read8(SAVE_BASE + 1), (id >> 8) as u8, "device byte");

        cmd(&mut gba, 0xAA);
        cmd(&mut gba, 0x55);
        cmd(&mut gba, 0xF0);
        assert_eq!(gba.bus.read8(SAVE_BASE), 0xFF, "ID mode was never terminated");
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
