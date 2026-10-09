//! Save-state restore against hostile or foreign bytes (Phase 5 security
//! audit, docs/security-audit-phase5.md). Each case is a value a state file
//! can carry but the machine can never produce, found by the audit probes
//! (temp/phase5/probes) with the byte offsets they used. Every one must be
//! rejected, and a rejected state must leave the running machine untouched.

use geebeeayy_core::savestate::SaveState;
use geebeeayy_core::Gba;

/// A 0x400-byte ROM running `code` from 0x08000000, with `title` in the
/// header and a save-library `marker` at 0x300 - the probes' `cart()`.
fn cart(code: &[u32], title: &[u8], marker: &[u8]) -> Vec<u8> {
    let mut rom = vec![0u8; 0x400];
    for (i, w) in code.iter().enumerate() {
        rom[i * 4..i * 4 + 4].copy_from_slice(&w.to_le_bytes());
    }
    rom[0xA0..0xA0 + title.len()].copy_from_slice(title);
    rom[0xAC..0xB0].copy_from_slice(b"ZZZZ");
    rom[0x300..0x300 + marker.len()].copy_from_slice(marker);
    rom
}

fn running(rom: &[u8]) -> Gba {
    let mut gba = Gba::new();
    gba.load_rom(rom).expect("ROM should load");
    gba
}

/// Load `state` into `gba`, require it to be refused, and require the machine
/// to come out byte-for-byte as it went in.
fn assert_rejected(gba: &mut Gba, state: Vec<u8>, what: &str) {
    let before = gba.save_state().data;
    let result = gba.load_state(&SaveState { data: state });
    assert!(result.is_err(), "{what}: state was accepted");
    assert_eq!(
        gba.save_state().data,
        before,
        "{what}: a rejected state changed the machine"
    );
}

// --- EMU-CORE-14: a state from another game ---------------------------------

/// Two carts with the same header (a ROM hack keeps the original's title and
/// game code), each writing its own byte to SRAM[0]. The frontend keys state
/// slots by header, so A's state is one tap away from B - and it carried A's
/// whole battery save into B, which B's next in-game save then wrote to disk.
/// Probe `emu foreign`, temp/phase5/emu-foreign.log.
#[test]
fn a_state_from_a_different_rom_is_rejected_and_keeps_the_battery_save() {
    // mov r0,#0x0E000000 ; mov r1,#0x41 ; strb r1,[r0] ; b .
    let a = cart(
        &[0xE3A0_040E, 0xE3A0_1041, 0xE5C0_1000, 0xEAFF_FFFE],
        b"POKEMON FIRE",
        b"SRAM_V",
    );
    // mov r0,#0x0E000000 ; add r0,r0,#0x100 ; mov r1,#0x43 ; strb r1,[r0] ; b .
    let b = cart(
        &[
            0xE3A0_040E,
            0xE280_0C01,
            0xE3A0_1043,
            0xE5C0_1000,
            0xEAFF_FFFE,
        ],
        b"POKEMON FIRE",
        b"SRAM_V",
    );
    let mut ga = running(&a);
    ga.run_frame();
    let foreign = ga.save_state().data;

    let mut gb = running(&b);
    gb.load_save(&vec![0x42u8; 32 * 1024]);
    gb.run_frame();
    assert_rejected(&mut gb, foreign, "A's state on B");
    assert_eq!(
        gb.save_data().unwrap()[0],
        0x42,
        "B's own save was replaced"
    );

    // A different save chip entirely: a Flash128 cart's state into SRAM.
    let mut gf = running(&cart(&[0xEAFF_FFFE], b"OTHER", b"FLASH1M_V"));
    gf.load_save(&vec![0x5Au8; 128 * 1024]);
    assert_rejected(&mut gb, gf.save_state().data, "Flash128 state on SRAM");
}

/// The same ROM loads its own state, as it always did.
#[test]
fn a_state_from_the_same_rom_still_loads() {
    let rom = cart(&[0xEAFF_FFFE], b"SAME", b"SRAM_V");
    let mut gba = running(&rom);
    gba.run_frame();
    let state = gba.save_state();
    assert!(running(&rom).load_state(&state).is_ok());
}

/// Players have v9 states on their phones, written before the ROM identity
/// existed. Those carry no identity to compare, so they still load - even onto
/// another ROM, which is no worse than before.
#[test]
fn a_version_9_state_without_a_rom_identity_still_loads() {
    let rom = cart(&[0xEAFF_FFFE], b"OLD", b"SRAM_V");
    let mut gba = running(&rom);
    gba.run_frame();
    let mut v9 = gba.save_state().data;
    v9.truncate(v9.len() - 4);
    v9[4..8].copy_from_slice(&9u32.to_le_bytes());
    let mut other = running(&cart(&[0xE1A0_0000, 0xEAFF_FFFD], b"OLD", b"SRAM_V"));
    assert!(other.load_state(&SaveState { data: v9 }).is_ok());
}

// --- Offsets the probes used ------------------------------------------------

/// Timer 0's block: header 8, CPU 152 + 24 + 1, PPU 68, frame buffer 115200.
/// Each timer is counter u32, reload u32, control u16, prescaler u32,
/// tick counter u32, then three flags.
const TIMER0_AT: usize = 8 + 152 + 24 + 1 + 68 + 115_200;
/// The APU blob's length field; the blob opens with `sample_accum: u64`.
const APU_LEN_AT: usize = 511_886;
const SAMPLE_ACCUM_AT: usize = APU_LEN_AT + 4;
/// Tail after `cycles`: chip state 18, DMA latches 16, SIO 4, ROM CRC 4.
const CYCLES_FROM_END: usize = 8 + 18 + 16 + 4 + 4;
const CHIP_FROM_END: usize = 18 + 16 + 4 + 4;

fn idle() -> Gba {
    let mut gba = running(&cart(&[0xEAFF_FFFE], b"IDLE", b""));
    gba.run_frame();
    gba
}

// --- CORE-2: sample_accum ---------------------------------------------------

/// Probe `byte 511897 EE`: the top byte of `sample_accum` made the first
/// frame push ~10^12 samples and abort on allocation ("memory allocation of
/// 4294967296 bytes failed") - an abort, which no `catch_unwind` stops.
#[test]
fn a_huge_restored_sample_accumulator_is_rejected() {
    let mut gba = idle();
    let mut state = gba.save_state().data;
    let apu_len = u32::from_le_bytes(state[APU_LEN_AT..APU_LEN_AT + 4].try_into().unwrap());
    assert!(
        (100..1000).contains(&apu_len),
        "APU length field moved: {apu_len}"
    );
    state[SAMPLE_ACCUM_AT + 7] = 0xEE;
    assert_rejected(&mut gba, state, "sample_accum top byte 0xEE");
    // One frame of audio, not a runaway.
    gba.clear_audio_buffer();
    gba.run_frame();
    assert!(gba.apu_samples().len() < 2000);
}

// --- CORE-3 / CORE-8: timer reload and counter -------------------------------

/// edge.log: reload 0x10000 divided by zero in `counter_after` on every step;
/// a counter of 0xFFFFFFFF overflowed `counters[i] += 1` in a debug build.
/// The game can only ever write 16 bits to either.
#[test]
fn a_timer_reload_or_counter_past_16_bits_is_rejected() {
    for (counter, reload) in [
        (0xFFFFu32, 0x1_0000u32),
        (0x1_0000, 0x1_0000),
        (0xFFFF, 0x2_0000),
        (0x1_0000, 0xFFFF),
        (0xFFFF_FFFF, 0),
    ] {
        let mut gba = idle();
        let mut state = gba.save_state().data;
        state[TIMER0_AT..TIMER0_AT + 4].copy_from_slice(&counter.to_le_bytes());
        state[TIMER0_AT + 4..TIMER0_AT + 8].copy_from_slice(&reload.to_le_bytes());
        assert_rejected(
            &mut gba,
            state,
            &format!("counter {counter:#X} reload {reload:#X}"),
        );
    }
}

#[test]
fn a_timer_at_its_16_bit_limits_still_loads() {
    let mut gba = idle();
    let mut state = gba.save_state().data;
    state[TIMER0_AT..TIMER0_AT + 4].copy_from_slice(&0xFFFFu32.to_le_bytes());
    state[TIMER0_AT + 4..TIMER0_AT + 8].copy_from_slice(&0xFFFFu32.to_le_bytes());
    assert!(gba.load_state(&SaveState { data: state }).is_ok());
    gba.run_frame();
}

// --- CORE-4: cycles ---------------------------------------------------------

/// edge.log: `cycles = u64::MAX - 10` wrapped the frame target below `cycles`,
/// so every later frame returned without stepping - a silent freeze.
#[test]
fn a_cycle_count_near_u64_max_is_rejected() {
    for cycles in [u64::MAX - 10, u64::MAX - 300_000, 1 << 63] {
        let mut gba = idle();
        let mut state = gba.save_state().data;
        let at = state.len() - CYCLES_FROM_END;
        assert_eq!(
            u64::from_le_bytes(state[at..at + 8].try_into().unwrap()),
            gba.cycles,
            "cycles field moved"
        );
        state[at..at + 8].copy_from_slice(&cycles.to_le_bytes());
        assert_rejected(&mut gba, state, &format!("cycles {cycles:#X}"));
    }
}

// --- EMU-CORE-13: EEPROM serial state ---------------------------------------

/// emu-eeprom.log: a count at or past the value that ends its phase never
/// matches again - every later bit only increments it - so the chip read FF
/// and dropped writes until the ROM was reloaded, and the wedge was saved
/// into every later state. Chip block offsets 5 (phase) and 6 (count).
#[test]
fn an_eeprom_count_past_the_end_of_its_phase_is_rejected() {
    let rom = cart(&[0xEAFF_FFFE], b"EEPROMTEST", b"EEPROM_V");
    // Opcode ends at 2 bits, a 6-bit address at 6, write data at 64; the stop
    // and read phases never count.
    for (phase, count) in [
        (0u8, 2u8),
        (0, 64),
        (1, 6),
        (1, 64),
        (2, 64),
        (3, 64),
        (4, 1),
        (5, 1),
        (6, 1),
    ] {
        let mut gba = running(&rom);
        gba.run_frame();
        let mut state = gba.save_state().data;
        let chip = state.len() - CHIP_FROM_END;
        state[chip + 5] = phase;
        state[chip + 6] = count;
        assert_rejected(&mut gba, state, &format!("phase {phase} count {count}"));
    }
}

/// A state taken mid-command is legitimate and must keep loading.
#[test]
fn an_eeprom_state_mid_command_still_loads() {
    let rom = cart(&[0xEAFF_FFFE], b"EEPROMTEST", b"EEPROM_V");
    for (phase, count) in [(0u8, 1u8), (1, 5), (2, 5), (3, 63), (4, 0), (6, 0)] {
        let mut gba = running(&rom);
        let mut state = gba.save_state().data;
        let chip = state.len() - CHIP_FROM_END;
        state[chip + 5] = phase;
        state[chip + 6] = count;
        assert!(
            gba.load_state(&SaveState { data: state }).is_ok(),
            "phase {phase} count {count}"
        );
    }
}
