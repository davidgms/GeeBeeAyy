//! Freely available homebrew accuracy suites, beyond jsmolka's gba-suite.
//!
//! The ROMs are **not** committed - no ROM ever is. Each developer fetches
//! them into `temp/roms/` under the names below; a missing file prints a skip
//! notice and the test passes.
//!
//! ```text
//! cd temp/roms
//! # mGBA suite (MIT, github.com/mgba-emu/suite), official nightly build:
//! curl -sSLO https://s3.amazonaws.com/mgba/suite-latest.zip
//! unzip suite-latest.zip && mv suite.gba mgba-suite.gba
//! # ARMWrestler, destoer's fixed build of mic's 2004-2006 test ROM. The repo
//! # carries no license file; it is freely distributed homebrew, used locally.
//! curl -sSLo armwrestler.gba \
//!   https://raw.githubusercontent.com/destoer/armwrestler-gba-fixed/master/armwrestler-gba-fixed.gba
//! # FuzzARM (GPL-3.0, github.com/DenSinH/FuzzARM), five pre-generated ROMs:
//! for r in FuzzARM ARM_Any THUMB_Any ARM_DataProcessing THUMB_DataProcessing; do
//!   curl -sSLo fuzzarm-$r.gba https://raw.githubusercontent.com/DenSinH/FuzzARM/master/$r.gba
//! done
//! ```
//!
//! AGS (Nintendo's own "AGB Aging Cartridge") is a commercial cartridge, not
//! freely available, and deliberately has no harness here.
//!
//! How each suite's verdict is read:
//! - **FuzzARM** dumps every failed test to EWRAM (`'AAAA'`/`'TTTT'` marker at
//!   0x02000000) and waits for a key; it ends in a `b .` loop. Exact.
//! - **ARMWrestler** passes each test's failure bitmask to its result-drawing
//!   routine. The harness catches that routine's entry and reads the mask, so
//!   the verdict is the ROM's own, without reading the bitmap screen. Exact.
//! - **mGBA suite** only reports "passes/total" per sub-suite as text on its
//!   BG1 tilemap. The harness drives the menu and reads that text. 100% is
//!   not the bar - several sub-suites measure cycle timing this interpreter
//!   does not model - so the gate is a per-sub-suite floor: no score may
//!   drop below the one recorded in `MGBA_FLOOR`.

use std::collections::BTreeMap;

use geebeeayy_core::Gba;

const KEY_A: u16 = 1 << 0;
const KEY_START: u16 = 1 << 3;
const KEY_DOWN: u16 = 1 << 7;

const CYCLES_PER_FRAME: u64 = 280_896;

fn load(name: &str) -> Option<Gba> {
    let path = format!("{}/../temp/roms/{name}.gba", env!("CARGO_MANIFEST_DIR"));
    let Ok(rom) = std::fs::read(&path) else {
        eprintln!("skipping '{name}': {path} not present");
        return None;
    };
    let mut gba = Gba::new();
    gba.load_rom(&rom).expect("suite ROM should load");
    Some(gba)
}

/// Run `frames` frames one instruction at a time, calling `watch` after each.
fn run_watching(gba: &mut Gba, frames: u32, mut watch: impl FnMut(&Gba)) {
    let target = gba.cycles + CYCLES_PER_FRAME * u64::from(frames);
    while gba.cycles < target {
        gba.step();
        watch(gba);
    }
}

/// Hold `keys` for a few frames, then release for a few: long enough for a
/// once-per-frame poll to see both edges.
fn press(gba: &mut Gba, keys: u16) {
    gba.bus.set_keys(keys);
    gba.run_frames(3);
    gba.bus.set_keys(0);
    gba.run_frames(3);
}

fn c_string(gba: &Gba, mut address: u32) -> String {
    let mut s = String::new();
    while s.len() < 32 {
        let b = gba.bus.read8(address);
        if b == 0 {
            break;
        }
        s.push(b as char);
        address += 1;
    }
    s
}

// --- FuzzARM ---------------------------------------------------------------
//
// All five ROMs are hard gates. The THUMB ones were ignored until 2026-10-02:
// every failure they reported was one bug, THUMB format-4 shifts by a register
// (`lsl/lsr/asr/ror Rd, Rs`) skipping the N/Z update when the low byte of Rs
// is 0 (`cpu.rs` `thumb_register_shift_by_zero_still_sets_n_and_z`).

/// Give up counting past this many failures; a broken CPU fails everything.
const FUZZARM_MAX_FAILURES: usize = 100;
const FUZZARM_MAX_FRAMES: u32 = 20_000;

/// `wait_until_key_down: ldr r1,[r0]; and r1,#0xff; cmp r1,#0xff; beq` - the
/// loop a failed test parks in once its EWRAM dump is complete. Waiting for
/// this, not for the dump marker, matters: the marker is written first and
/// the rest of the dump trickles in between slow text drawing.
const FUZZARM_KEY_WAIT: [u8; 16] = [
    0x00, 0x10, 0x90, 0xE5, 0xFF, 0x10, 0x01, 0xE2, 0xFF, 0x00, 0x51, 0xE3, 0xFB, 0xFF, 0xFF, 0x0A,
];

/// Returns `(finished, failures)`; each failure is the opcode text from the
/// EWRAM dump plus the dump's input, got and expected words.
fn run_fuzzarm(gba: &mut Gba) -> (bool, Vec<String>) {
    let wait = find_in_rom(gba, &FUZZARM_KEY_WAIT).expect("FuzzARM key-wait loop not found");
    let mut failures = Vec::new();
    for _ in 0..FUZZARM_MAX_FRAMES {
        let end = gba.cycles + CYCLES_PER_FRAME;
        while gba.cycles < end {
            let pc = gba.cpu.registers[15];
            if pc == wait {
                let word = |i: u32| gba.bus.read32(0x0200_0000 + i * 4);
                let opcode: String = (4..16)
                    .map(|a| gba.bus.read8(0x0200_0000 + a) as char)
                    .collect();
                let state = if word(0) == 0x5454_5454 {
                    "THUMB"
                } else {
                    "ARM"
                };
                failures.push(format!(
                    "{state} {:<12} in r0={:08X} r1={:08X} r2={:08X} cpsr={:08X} \
                     got r3={:08X} r4={:08X} cpsr={:08X} want r3={:08X} r4={:08X} cpsr={:08X}",
                    opcode.trim_end(),
                    word(4),
                    word(5),
                    word(6),
                    word(7),
                    word(8),
                    word(9),
                    word(11),
                    word(12),
                    word(13),
                    word(15),
                ));
                if failures.len() >= FUZZARM_MAX_FAILURES {
                    return (false, failures);
                }
                // Any key down moves on; the ROM first waits for all keys
                // up, which the released state after this satisfies.
                gba.bus.set_keys(KEY_A);
                gba.run_frame();
                gba.bus.set_keys(0);
                break;
            }
            gba.step();
            // "End of testing" parks in `b .`.
            if gba.cpu.registers[15] == pc {
                return (true, failures);
            }
        }
    }
    (false, failures)
}

fn fuzzarm(name: &str) {
    let Some(mut gba) = load(&format!("fuzzarm-{name}")) else {
        return;
    };
    let (finished, failures) = run_fuzzarm(&mut gba);
    for f in &failures {
        eprintln!("FuzzARM {name}: FAIL {f}");
    }
    eprintln!(
        "FuzzARM {name}: {} failure(s), {}",
        failures.len(),
        if finished {
            "reached End of testing"
        } else {
            "did NOT finish"
        }
    );
    assert!(finished, "FuzzARM {name} did not reach its end loop");
    assert!(
        failures.is_empty(),
        "FuzzARM {name}: {} failure(s)",
        failures.len()
    );
}

#[test]
fn fuzzarm_mixed() {
    fuzzarm("FuzzARM");
}

#[test]
fn fuzzarm_arm_any() {
    fuzzarm("ARM_Any");
}

#[test]
fn fuzzarm_thumb_any() {
    fuzzarm("THUMB_Any");
}

#[test]
fn fuzzarm_arm_data_processing() {
    fuzzarm("ARM_DataProcessing");
}

#[test]
fn fuzzarm_thumb_data_processing() {
    fuzzarm("THUMB_DataProcessing");
}

// --- ARMWrestler -------------------------------------------------------------

/// `stmfd sp!,{r0-r5,lr}; mov r4,r1; mov r5,r2` - the ARM `DrawResult` entry.
/// r0 = test name, r1 = failure bitmask (low byte), r8 = screen row.
const AW_ARM_RESULT: [u8; 12] = [
    0x3F, 0x40, 0x2D, 0xE9, 0x01, 0x40, 0xA0, 0xE1, 0x02, 0x50, 0xA0, 0xE1,
];
/// `push {r4,r5,lr}; movs r1,#16; adds r2,r7,#0` - the THUMB `_drawresult`.
/// r0 = test name, r6 = failure bitmask, r7 = screen row.
const AW_THUMB_RESULT: [u8; 6] = [0x30, 0xB5, 0x10, 0x21, 0x3A, 0x1C];

/// Locate a byte pattern in the ROM, as a bus address.
fn find_in_rom(gba: &Gba, pattern: &[u8]) -> Option<u32> {
    let rom: Vec<u8> = (0..0x8_0000u32)
        .map(|a| gba.bus.read8(0x0800_0000 + a))
        .collect();
    rom.windows(pattern.len())
        .position(|w| w == pattern)
        .map(|p| 0x0800_0000 + p as u32)
}

#[test]
fn armwrestler() {
    let Some(mut gba) = load("armwrestler") else {
        return;
    };
    let arm_entry = find_in_rom(&gba, &AW_ARM_RESULT).expect("ARM DrawResult not found");
    let thumb_entry = find_in_rom(&gba, &AW_THUMB_RESULT).expect("THUMB drawresult not found");

    // (thumb, page, row) -> (name, failure mask). Pages redraw every frame,
    // so the map dedupes.
    let mut results: BTreeMap<(bool, u8, u32), (String, u32)> = BTreeMap::new();
    let mut page = |gba: &mut Gba| {
        run_watching(gba, 4, |gba| {
            let pc = gba.cpu.registers[15];
            let thumb = gba.cpu.cpsr & 0x20 != 0;
            let page = gba.bus.read8(0x0300_0008);
            let r = &gba.cpu.registers;
            if !thumb && pc == arm_entry {
                let name = c_string(gba, r[0]);
                results.insert((false, page, r[8]), (name, r[1] & 0xFF));
            } else if thumb && pc == thumb_entry {
                let name = c_string(gba, r[0]);
                results.insert((true, page, r[7]), (name, r[6]));
            }
        });
    };

    gba.run_frames(10);
    // Menu item 0 is the ARM ALU test; Start then walks pages 0..=4 and back
    // to the menu.
    press(&mut gba, KEY_START);
    for _ in 0..5 {
        page(&mut gba);
        press(&mut gba, KEY_START);
    }
    // Menu items 3..5 are THUMB; item 3 starts at THUMB page 0 of 3.
    for _ in 0..3 {
        press(&mut gba, KEY_DOWN);
    }
    press(&mut gba, KEY_START);
    for _ in 0..3 {
        page(&mut gba);
        press(&mut gba, KEY_START);
    }

    let bad: Vec<_> = results.iter().filter(|(_, (_, mask))| *mask != 0).collect();
    for ((thumb, page, row), (name, mask)) in &bad {
        eprintln!(
            "ARMWrestler: BAD {} page {page} row {row}: {name} mask {mask:#X}",
            if *thumb { "THUMB" } else { "ARM" }
        );
    }
    eprintln!(
        "ARMWrestler: {}/{} OK",
        results.len() - bad.len(),
        results.len()
    );
    assert!(
        results.len() > 50,
        "only {} results seen - menu navigation went wrong",
        results.len()
    );
    assert!(bad.is_empty(), "ARMWrestler: {} BAD result(s)", bad.len());
}

// --- mGBA suite --------------------------------------------------------------

const MGBA_SUITES: usize = 14;
/// Frames allowed for one sub-suite to finish and show its score.
const MGBA_SUITE_FRAMES: u32 = 3_000;

/// Text row `row` of the suite's BG1 tilemap (screen block 1, tile = char -
/// ' ').
fn mgba_text_row(gba: &Gba, row: u32) -> String {
    (0..30)
        .map(|col| {
            let entry = gba.bus.read16(0x0600_0800 + (row * 32 + col) * 2) & 0x3FF;
            char::from_u32(u32::from(entry) + 0x20).unwrap_or('?')
        })
        .collect()
}

/// `(passes, total)` from the "%4u/%-4u" at row 1, column 21.
fn mgba_score(gba: &Gba) -> Option<(u32, u32)> {
    let row = mgba_text_row(gba, 1);
    let (pass, total) = row.get(21..)?.split_once('/')?;
    Some((pass.trim().parse().ok()?, total.trim().parse().ok()?))
}

type SubSuite = (String, Option<(u32, u32)>);

/// Each sub-suite's `(name, score)`, in menu order, or `None` without the
/// ROM. A score is `None` when the sub-suite showed none in time (the video
/// suite never does) or panicked in the core.
fn mgba_scores() -> Option<Vec<SubSuite>> {
    load("mgba-suite")?;
    // A fresh machine per sub-suite, each on its own thread, so one that
    // hangs or panics costs only itself. A panic is a core bug worth
    // reporting, not a reason to lose the other scores.
    let scores = std::thread::scope(|s| {
        let runs: Vec<_> = (0..MGBA_SUITES)
            .map(|index| {
                s.spawn(move || {
                    let mut gba = load("mgba-suite").expect("present a moment ago");
                    gba.run_frames(30);
                    for _ in 0..index {
                        press(&mut gba, KEY_DOWN);
                    }
                    press(&mut gba, KEY_A);
                    let mut score = None;
                    for _ in 0..MGBA_SUITE_FRAMES / 10 {
                        gba.run_frames(10);
                        score = mgba_score(&gba);
                        if score.is_some() {
                            break;
                        }
                    }
                    let name = mgba_text_row(&gba, 1)
                        .get(..20)
                        .unwrap_or("")
                        .trim()
                        .to_string();
                    (name, score)
                })
            })
            .collect();
        runs.into_iter()
            .map(|run| {
                run.join()
                    .unwrap_or_else(|_| ("PANICKED in the core (see above)".into(), None))
            })
            .collect()
    });
    Some(scores)
}

/// The full report; read it with
/// `cargo test --release --test homebrew_suites -- --ignored --nocapture`.
/// Ignored because `mgba_suite_does_not_regress` already runs the same thing
/// as a gate.
#[test]
#[ignore = "score report; run with --ignored --nocapture"]
fn mgba_suite_score() {
    let Some(scores) = mgba_scores() else {
        return;
    };
    let (mut passed, mut total) = (0, 0);
    for (index, (name, score)) in scores.iter().enumerate() {
        match score {
            Some((p, t)) => {
                eprintln!("mGBA suite {index:2} {name:<20} {p:4}/{t}");
                passed += p;
                total += t;
            }
            // The video suite has no count at all; it is judged by eye.
            None => eprintln!(
                "mGBA suite {index:2} {name:<20} no score within {MGBA_SUITE_FRAMES} frames \
                 (hung, panicked, or a visual-only suite)"
            ),
        }
    }
    eprintln!("mGBA suite total: {passed}/{total}");
}

/// Passes per sub-suite as of 2026-10-08 (6852/6998), in menu order; the
/// video suite (13) has no score. Raise a number when a fix gains cells,
/// never lower one to make a change pass.
const MGBA_FLOOR: [u32; MGBA_SUITES - 1] = [
    1552, // Memory
    130,  // I/O read
    1880, // Timing
    936,  // Timer count-up
    90,   // Timer IRQ
    140,  // Shifter
    93,   // Carry
    72,   // Multiply long
    615,  // BIOS math
    1244, // DMA
    90,   // SIO register R/W
    4,    // SIO timing
    6,    // Misc. edge cases
];

/// No sub-suite may score below its floor. Skips without the ROM, like the
/// FuzzARM gates. About 4 s in a release build.
#[test]
fn mgba_suite_does_not_regress() {
    let Some(scores) = mgba_scores() else {
        return;
    };
    let regressions: Vec<String> = MGBA_FLOOR
        .iter()
        .zip(&scores)
        .enumerate()
        .filter_map(|(index, (&floor, (name, score)))| {
            let passed = score.map_or(0, |(p, _)| p);
            (passed < floor).then(|| format!("{index:2} {name}: {passed} < {floor}"))
        })
        .collect();
    assert!(
        regressions.is_empty(),
        "mGBA suite regressed:\n{}",
        regressions.join("\n")
    );
}
