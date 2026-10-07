//! Instruction timing in every memory region and wait-state setting, against
//! the hardware table of the mGBA suite's timing tests.
//!
//! The suite (github.com/mgba-emu/suite, `src/timing.c` and `src/tests/*.s`,
//! MIT licence, see THIRD_PARTY_NOTICES.md) times a few instructions with
//! timer 0: `str` to TM0CNT starts it, the code runs, `ldrh` from TM0CNT_L
//! reads it. The result is that reading minus the same reading with no code
//! in between, so the fixed cost of the harness cancels. This file rebuilds
//! that harness by hand - no ROM or BIOS image - and runs the same code in
//! ROM under four WAITCNT settings, each with the prefetch buffer off and on,
//! then in EWRAM and IWRAM, ARM and THUMB: the suite's twenty columns.

use geebeeayy_core::Gba;

const TM0CNT: u32 = 0x0400_0100;
const SP: u32 = 0x0300_7F00;

/// Where each column runs, in the table's order: ROM at WAITCNT 0x0000,
/// 0x4000, 0x0004, 0x4004, 0x0010, 0x4010, 0x0014, 0x4014 (bit 14 is the
/// prefetch buffer, bits 2-4 WS0's waits), then EWRAM and IWRAM; ARM first,
/// THUMB second.
const COLUMNS: [(u32, u16); 10] = [
    (0x0800_0100, 0x0000),
    (0x0800_0100, 0x4000),
    (0x0800_0100, 0x0004),
    (0x0800_0100, 0x4004),
    (0x0800_0100, 0x0010),
    (0x0800_0100, 0x4010),
    (0x0800_0100, 0x0014),
    (0x0800_0100, 0x4014),
    (0x0200_0100, 0x0000),
    (0x0300_0100, 0x0000),
];

/// Columns with the prefetch buffer enabled.
const PREFETCH: [usize; 4] = [1, 3, 5, 7];

/// The suite's harness around `code`: START (`str r1, [r0]`), the code, END
/// (`ldrh r2, [r0]` then `strh r1, [r0, #2]`), and a `b .` to stop on.
fn program(thumb: bool, code: &[u32]) -> Vec<u8> {
    let mut out = Vec::new();
    if thumb {
        let mut push = |h: u16| out.extend_from_slice(&h.to_le_bytes());
        push(0x6001);
        code.iter().for_each(|&h| push(h as u16));
        [0x8802, 0x8041, 0xE7FE].into_iter().for_each(push);
    } else {
        let mut push = |w: u32| out.extend_from_slice(&w.to_le_bytes());
        push(0xE580_1000);
        code.iter().for_each(|&w| push(w));
        [0xE1D0_20B0, 0xE1C0_10B2, 0xEAFF_FFFE]
            .into_iter()
            .for_each(push);
    }
    out
}

/// Run the harness around `code` at `base` and return the timer reading.
fn run(thumb: bool, (base, waitcnt): (u32, u16), regs: &[(usize, u32)], code: &[u32]) -> u32 {
    let program = program(thumb, code);
    let mut rom = vec![0u8; 0x1000];
    if base >> 24 == 0x08 {
        rom[0x100..0x100 + program.len()].copy_from_slice(&program);
    }
    let mut gba = Gba::new();
    gba.load_rom(&rom).expect("ROM should load");
    if base >> 24 != 0x08 {
        for (i, &b) in program.iter().enumerate() {
            gba.bus.write8(base + i as u32, b);
        }
    }
    gba.bus.write16(0x0400_0204, waitcnt);
    // The suite's stack holds code addresses; any word will do here.
    gba.bus.write32(SP, 0x0800_0000);
    gba.cpu.registers[0] = TM0CNT;
    gba.cpu.registers[1] = 0x0080_0000; // TM0CNT: reload 0, enabled
    gba.cpu.registers[13] = SP;
    for &(r, v) in regs {
        gba.cpu.registers[r] = v;
    }
    gba.cpu.registers[15] = base;
    if thumb {
        gba.cpu.cpsr |= 0x20;
    }
    let stop = base + program.len() as u32 - if thumb { 2 } else { 4 };
    for _ in 0..10_000 {
        if gba.cpu.registers[15] == stop {
            return gba.cpu.registers[2];
        }
        gba.step();
    }
    panic!("never reached the end of the program");
}

/// One row of the table: its ARM and THUMB code, registers set beforehand,
/// and the twenty hardware values. `columns` picks which are asserted.
struct Row<'a> {
    name: &'a str,
    arm: &'a [u32],
    thumb: &'a [u32],
    regs: &'a [(usize, u32)],
    expected: [u32; 20],
}

fn check(rows: &[Row], columns: impl Fn(usize) -> bool) {
    let mut failures = Vec::new();
    for row in rows {
        for (mode, code) in [(false, row.arm), (true, row.thumb)] {
            if code.is_empty() {
                continue;
            }
            for (c, &column) in COLUMNS.iter().enumerate() {
                if !columns(c) {
                    continue;
                }
                let got = run(mode, column, row.regs, code).wrapping_sub(run(
                    mode,
                    column,
                    row.regs,
                    &[],
                ));
                let want = row.expected[c + if mode { 10 } else { 0 }];
                if got != want {
                    let mode = if mode { "THUMB" } else { "ARM" };
                    failures.push(format!(
                        "{}: {mode} at {:#010x} WAITCNT {:#06x}: got {} want {want}",
                        row.name, column.0, column.1, got as i32
                    ));
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} cells wrong:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

const NOP_ARM: u32 = 0xE1A0_0000; // mov r0, r0
const NOP_THUMB: u32 = 0x46C0; // mov r8, r8
const ROM_DATA: (usize, u32) = (3, 0x0800_0000);

fn rows() -> Vec<Row<'static>> {
    vec![
        Row {
            name: "nop",
            arm: &[NOP_ARM],
            thumb: &[NOP_THUMB],
            regs: &[],
            expected: [6, 6, 6, 6, 4, 4, 4, 4, 6, 1, 3, 3, 3, 3, 2, 2, 2, 2, 3, 1],
        },
        Row {
            name: "ldr r2, [sp]",
            arm: &[0xE59D_2000],
            thumb: &[0x9A00],
            regs: &[],
            expected: [10, 6, 9, 6, 9, 4, 8, 4, 8, 3, 7, 3, 6, 3, 7, 3, 6, 3, 5, 3],
        },
        Row {
            name: "ldrh r2, [#0x08000000]",
            arm: &[0xE1D3_20B0],
            thumb: &[0x881A],
            regs: &[ROM_DATA],
            expected: [
                14, 14, 12, 12, 13, 13, 11, 11, 12, 7, 11, 11, 9, 9, 11, 11, 9, 9, 9, 7,
            ],
        },
        Row {
            name: "ldr r2, [#0x08000000]",
            arm: &[0xE593_2000],
            thumb: &[0x681A],
            regs: &[ROM_DATA],
            expected: [
                17, 17, 15, 15, 15, 15, 13, 13, 15, 10, 14, 14, 12, 12, 13, 13, 11, 11, 12, 10,
            ],
        },
        Row {
            name: "str r3, [sp] x2",
            arm: &[0xE58D_3000, 0xE58D_3000],
            thumb: &[0x9300, 0x9300],
            regs: &[],
            expected: [
                18, 12, 16, 12, 16, 8, 14, 8, 14, 4, 12, 6, 10, 6, 12, 4, 10, 4, 8, 4,
            ],
        },
        Row {
            name: "mul #0x12345678, #0xFF",
            arm: &[0xE003_0392], // mul r3, r2, r3
            thumb: &[0x4353],    // mul r3, r2
            regs: &[(2, 0xFF), (3, 0x1234_5678)],
            expected: [
                12, 6, 11, 6, 11, 5, 10, 5, 10, 5, 9, 5, 8, 5, 9, 5, 8, 5, 7, 5,
            ],
        },
        Row {
            name: "b 1f ; nop ; 1: nop",
            arm: &[0xEA00_0000, NOP_ARM, NOP_ARM],
            thumb: &[0xE000, NOP_THUMB, NOP_THUMB],
            regs: &[],
            expected: [
                26, 26, 25, 25, 19, 19, 18, 18, 24, 4, 14, 14, 13, 13, 11, 11, 10, 10, 12, 4,
            ],
        },
        Row {
            name: "Trivial loop",
            // mov r2, #0 ; mov r3, #16 ; 1: add r2, #1 ; cmp r2, r3 ; bne 1b
            arm: &[
                0xE3A0_2000,
                0xE3A0_3010,
                0xE282_2001,
                0xE152_0003,
                0x1AFF_FFFC,
            ],
            thumb: &[0x2200, 0x2310, 0x3201, 0x429A, 0xD1FC],
            regs: &[],
            expected: [
                510, 510, 495, 495, 365, 365, 350, 350, 480, 80, 270, 270, 255, 255, 205, 205, 190,
                190, 240, 80,
            ],
        },
        Row {
            name: "ldr r2, [sp] / ldr r2, [#0x08000000]",
            arm: &[0xE59D_2000, 0xE593_2000],
            thumb: &[0x9A00, 0x681A],
            regs: &[ROM_DATA],
            expected: [
                27, 23, 24, 21, 24, 19, 21, 17, 23, 13, 21, 17, 18, 15, 20, 17, 17, 15, 17, 13,
            ],
        },
        // An LDM running off the end of OAM into ROM: how many OAM words
        // come first decides where the prefetch buffer is in a halfword when
        // the ROM access stops it.
        Row {
            name: "ldmia [#0x07FFFFFC]!, {r3-r7}",
            arm: &[0xE8B2_00F8],
            thumb: &[0xCAF8],
            regs: &[(2, 0x07FF_FFFC)],
            expected: [
                36, 36, 34, 34, 28, 29, 26, 27, 34, 29, 33, 33, 31, 31, 26, 27, 24, 25, 31, 29,
            ],
        },
        Row {
            name: "ldmia [#0x07FFFFF8]!, {r3-r7}",
            arm: &[0xE8B2_00F8],
            thumb: &[0xCAF8],
            regs: &[(2, 0x07FF_FFF8)],
            expected: [
                31, 32, 29, 30, 25, 25, 23, 23, 29, 24, 28, 29, 26, 27, 23, 23, 21, 21, 26, 24,
            ],
        },
        Row {
            name: "ldmia sp, {r2-r7}",
            arm: &[0xE89D_00FC],
            thumb: &[],
            regs: &[],
            expected: [
                15, 8, 14, 8, 14, 8, 13, 8, 13, 8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            ],
        },
        Row {
            name: "stmia sp, {r2-r7}",
            arm: &[0xE88D_00FC],
            thumb: &[],
            regs: &[],
            expected: [
                14, 7, 13, 7, 13, 7, 12, 7, 12, 7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            ],
        },
    ]
}

/// ROM with the prefetch buffer off, EWRAM and IWRAM: per-region wait
/// states, WAITCNT, and the opcode fetch after a data access (or after an
/// internal cycle in ROM, GBATEK's prefetch disable bug) being
/// non-sequential.
#[test]
fn timing_without_prefetch_matches_hardware() {
    check(&rows(), |c| !PREFETCH.contains(&c));
}

/// ROM with the GamePak prefetch buffer on: opcodes come out of the buffer
/// in one cycle when the buffer had idle bus cycles to fill it.
#[test]
fn timing_with_prefetch_matches_hardware() {
    check(&rows(), |c| PREFETCH.contains(&c));
}
