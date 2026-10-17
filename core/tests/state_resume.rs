//! A save state has to resume the machine it was taken from, not a hybrid.
//!
//! Each case saves a running machine at a point where some piece of live
//! state matters - the dot within the scanline, the affine accumulators, an
//! HLE BIOS wait, the BIOS read latch, cycles owed to the next instruction -
//! loads it into a freshly booted machine of the same ROM (whose live state
//! differs), and requires both to run on identically. Up to v10 those fields
//! were not in the state, so the restored machine kept its own.

use gbee_core::Gba;

/// A 0x400-byte ROM with `code` at the entry point.
fn rom(code: &[u32]) -> Vec<u8> {
    let mut data = vec![0u8; 0x400];
    for (i, w) in code.iter().enumerate() {
        data[i * 4..i * 4 + 4].copy_from_slice(&w.to_le_bytes());
    }
    data
}

fn machine(rom: &[u8]) -> Gba {
    let mut gba = Gba::new();
    gba.load_rom(rom).expect("ROM should load");
    gba
}

const SPIN: [u32; 1] = [0xEAFF_FFFE]; // b .

fn write_code(gba: &mut Gba, at: u32, code: &[u32]) {
    for (i, w) in code.iter().enumerate() {
        gba.bus.write32(at + 4 * i as u32, *w);
    }
}

/// What two machines running the same program must agree on after a step.
fn snapshot(gba: &Gba) -> (u64, [u32; 16], u32, u16, bool, u16) {
    (
        gba.cycles,
        gba.cpu.registers,
        gba.cpu.cpsr,
        gba.ppu.scanline,
        gba.bus.io.halt,
        gba.bus.io.if_,
    )
}

/// Save `original`, load the state into `fresh`, then step both `steps`
/// times and require the same machine after every step and at the end.
fn assert_resumes_alike(original: &mut Gba, fresh: &mut Gba, steps: usize, what: &str) {
    fresh
        .load_state(&original.save_state())
        .expect("a machine's own state loads");
    for i in 0..steps {
        original.step();
        fresh.step();
        assert_eq!(
            snapshot(fresh),
            snapshot(original),
            "{what}: the restored machine diverged at step {i}"
        );
    }
    assert!(
        fresh.frame_buffer()[..] == original.frame_buffer()[..],
        "{what}: the restored machine drew a different picture"
    );
    assert!(
        fresh.save_state().data == original.save_state().data,
        "{what}: the machines ended in different states"
    );
}

/// The dot within the scanline. Restored at 0, the line boundary, VCOUNT,
/// HBlank and every PPU interrupt moved by however far into the line the
/// state was taken - for good, since nothing resynchronises it.
#[test]
fn the_dot_within_the_scanline_survives_a_save_state() {
    let rom = rom(&SPIN);
    let mut original = machine(&rom);
    original.run_frame();
    while !original.ppu.hblank {
        original.step();
    }
    assert_resumes_alike(&mut original, &mut machine(&rom), 3000, "mid-HBlank");
}

/// The affine reference point is accumulated down the frame (PB/PD per
/// line) and only reloaded at line 0 or when BGxX/BGxY are written. Restored
/// as whatever the target machine held, a rotated background drew the wrong
/// rows until the next frame.
#[test]
fn the_affine_accumulators_survive_a_save_state_mid_frame() {
    let rom = rom(&SPIN);
    let mut original = machine(&rom);
    let bus = &mut original.bus;
    // Mode 2, BG2 on; BG2CNT: 256 colours, char base 0, map at 0x06004000,
    // 128x128 pixels, no wrap. Identity transform: one map row per 8 lines.
    bus.write16(0x0400_0000, 0x0402);
    bus.write16(0x0400_000C, 0x0880);
    bus.write16(0x0400_0020, 0x0100); // PA
    bus.write16(0x0400_0026, 0x0100); // PD
    for tile in 1..=16u32 {
        for b in (0..64).step_by(2) {
            bus.write16(0x0600_0000 + tile * 64 + b, (tile * 0x0101) as u16);
        }
        bus.write16(0x0500_0000 + tile * 2, (tile * 0x0842) as u16);
    }
    // Map row r is all tile r + 1, so every 8 lines look different.
    for r in 0..16u32 {
        for c in (0..16).step_by(2) {
            bus.write16(0x0600_4000 + r * 16 + c, ((r + 1) * 0x0101) as u16);
        }
    }
    original.run_frame();
    while original.ppu.scanline != 80 {
        original.step();
    }
    // To the end of the frame: lines 80-127 show map rows 10-15.
    assert_resumes_alike(&mut original, &mut machine(&rom), 15_000, "line 80");
}

/// The IRQ handler a game installs at [0x03007FFC]: acknowledge IF and OR
/// the handled bits into the BIOS IntrWait flags, as hardware requires.
const HANDLER: [u32; 12] = [
    0xE3A0_0301, // mov r0, #0x04000000
    0xE280_0C02, // add r0, r0, #0x200
    0xE1D0_10B0, // ldrh r1, [r0]        ; IE
    0xE1D0_20B2, // ldrh r2, [r0, #2]    ; IF
    0xE001_1002, // and r1, r1, r2
    0xE1C0_10B2, // strh r1, [r0, #2]    ; acknowledge
    0xE59F_000C, // ldr r0, [pc, #12]    ; 0x03007FF8
    0xE1D0_20B0, // ldrh r2, [r0]
    0xE182_2001, // orr r2, r2, r1
    0xE1C0_20B0, // strh r2, [r0]
    0xE12F_FF1E, // bx lr
    0x0300_7FF8, // literal
];

/// A state taken while the game sits in VBlankIntrWait. The HLE re-runs the
/// SWI after every interrupt, and only its "already waiting" flag stops that
/// re-run from discarding the VBlank flag the handler just set. Restored
/// without it, the first VBlank was thrown away and the game came back a
/// frame late.
#[test]
fn a_state_saved_in_vblank_intr_wait_wakes_at_the_next_vblank() {
    let rom = rom(&[
        0xEF05_0000, // swi 5               ; VBlankIntrWait
        0xE284_4001, // add r4, r4, #1
        0xEAFF_FFFC, // b 0x08000000
    ]);
    let mut original = machine(&rom);
    write_code(&mut original, 0x0300_0000, &HANDLER);
    original.bus.write32(0x0300_7FFC, 0x0300_0000);
    original.bus.write16(0x0400_0004, 0x0008); // VBlank IRQ enable
    original.bus.io.ie = 0x0001;
    original.run_frame();
    original.run_frame();
    while !(original.bus.io.halt && original.bus.io.intr_wait_active && original.ppu.scanline < 100)
    {
        original.step();
    }
    let state = original.save_state();
    let frames_done = original.cpu.registers[4];

    // Resumed in a fresh machine, the wait ends at line 160 of this frame.
    let mut fresh = machine(&rom);
    fresh.load_state(&state).expect("own state loads");
    let mut woke_at = None;
    let frame_end = fresh.cycles + 280_896;
    while fresh.cycles < frame_end {
        fresh.step();
        if fresh.cpu.registers[4] != frames_done {
            woke_at = Some(fresh.ppu.scanline);
            break;
        }
    }
    assert!(
        woke_at.is_some_and(|line| (160..228).contains(&line)),
        "the restored wait did not return at the next VBlank: {woke_at:?}"
    );

    assert_resumes_alike(&mut original, &mut machine(&rom), 40_000, "VBlankIntrWait");
}

/// The Halt SWI owes its BIOS return path (`HALT_RETURN_CYCLES`) once the
/// wake comes. Restored without the flag that says so, the wake was 30
/// cycles short.
#[test]
fn a_state_saved_in_the_halt_swi_keeps_its_return_cost() {
    let rom = rom(&[
        0xEF02_0000, // swi 2               ; Halt
        0xE284_4001, // add r4, r4, #1
        0xEAFF_FFFE, // b .
    ]);
    let mut original = machine(&rom);
    original.bus.write16(0x0400_0004, 0x0008); // VBlank IRQ enable, IME off
    original.bus.io.ie = 0x0001;
    while !original.bus.io.halt_swi {
        original.step();
    }
    assert_resumes_alike(&mut original, &mut machine(&rom), 1000, "Halt SWI");
}

/// A BIOS read from outside the BIOS returns the last opcode the BIOS
/// fetched - after an HLE SWI, the one its epilogue leaves. Restored as the
/// target machine's latch, the read returned the post-boot opcode.
#[test]
fn the_bios_read_latch_survives_a_save_state() {
    let rom = rom(&[
        0xE3A0_0010, // mov r0, #16
        0xEF08_0000, // swi 8               ; Sqrt
        0xE3A0_1000, // mov r1, #0
        0xE591_2000, // ldr r2, [r1]        ; BIOS, from ROM
        0xEAFF_FFFE, // b .
    ]);
    let mut original = machine(&rom);
    original.step();
    original.step();
    assert_resumes_alike(&mut original, &mut machine(&rom), 3, "after a SWI");
    assert_eq!(
        original.cpu.registers[2], 0xE3A0_2004,
        "the latch a SWI leaves"
    );
}

/// An immediate DMA enabled from IWRAM, where the next opcode fetch takes
/// one cycle, stops the CPU after the *next* instruction: the stall is owed
/// across the step boundary. A state taken there lost it.
#[test]
fn a_dma_stall_owed_to_the_next_instruction_survives_a_save_state() {
    let rom = rom(&SPIN);
    let mut original = machine(&rom);
    write_code(
        &mut original,
        0x0300_0000,
        &[0xE1C2_10B0, 0xE1A0_0000, 0xE1A0_0000, 0xEAFF_FFFE], // strh r1, [r2]; nop; nop; b .
    );
    // DMA3: 16 halfwords EWRAM -> EWRAM; the strh writes CNT_H's enable.
    original.bus.write32(0x0400_00D4, 0x0200_0000);
    original.bus.write32(0x0400_00D8, 0x0200_1000);
    original.bus.write16(0x0400_00DC, 16);
    original.cpu.registers[1] = 0x8000;
    original.cpu.registers[2] = 0x0400_00DE;
    original.cpu.registers[15] = 0x0300_0000;
    original.step();
    assert_resumes_alike(&mut original, &mut machine(&rom), 4, "DMA enabled");
}

/// Every step boundary of a loop interrupted by a fast timer IRQ. Some
/// boundaries fall right after an IRQ entry (its cycles are charged to the
/// next step) and some inside the IRQ delay after an overflow; both were
/// dropped by a save state, moving the interrupt by a few cycles.
#[test]
fn every_step_boundary_of_a_timer_irq_loop_resumes_alike() {
    let rom = rom(&SPIN);
    let mut gba = machine(&rom);
    write_code(
        &mut gba,
        0x0300_0000,
        &[
            0xE3A0_0301, // mov r0, #0x04000000
            0xE280_0C02, // add r0, r0, #0x200
            0xE1D0_10B2, // ldrh r1, [r0, #2]    ; IF
            0xE1C0_10B2, // strh r1, [r0, #2]    ; acknowledge
            0xE12F_FF1E, // bx lr
        ],
    );
    write_code(
        &mut gba,
        0x0300_0100,
        &[
            0xE284_4001, // add r4, r4, #1
            0xE085_5004, // add r5, r5, r4
            0xEAFF_FFFC, // b 0x03000100
        ],
    );
    gba.bus.write32(0x0300_7FFC, 0x0300_0000);
    gba.bus.write16(0x0400_0100, 0xFFC0); // timer 0: 64 counts
    gba.bus.write16(0x0400_0102, 0x00C0); // enable, IRQ, prescaler 1
    gba.bus.io.ie = 0x0008;
    gba.bus.io.ime = 1;
    gba.cpu.registers[15] = 0x0300_0100;
    for _ in 0..50 {
        gba.step();
    }

    const BOUNDARIES: usize = 600;
    const AFTER: usize = 40;
    let mut states = Vec::with_capacity(BOUNDARIES);
    let mut trace = Vec::with_capacity(BOUNDARIES + AFTER);
    for _ in 0..BOUNDARIES + AFTER {
        if states.len() < BOUNDARIES {
            states.push(gba.save_state());
        }
        gba.step();
        trace.push(snapshot(&gba));
    }
    for (k, state) in states.iter().enumerate() {
        let mut fresh = machine(&rom);
        fresh.load_state(state).expect("own state loads");
        for (i, want) in trace[k..k + AFTER].iter().enumerate() {
            fresh.step();
            assert_eq!(
                &snapshot(&fresh),
                want,
                "restored at boundary {k}, diverged {i} steps later"
            );
        }
    }
}

/// Whether the PPU has drawn the current line. A state taken in HBlank, where
/// the line is already drawn, restored into a machine that had not drawn it
/// drew it a second time at the line's end - with whatever the HBlank code
/// had changed by then.
#[test]
fn a_line_drawn_before_the_save_is_not_drawn_again() {
    let rom = rom(&SPIN);
    let mut original = machine(&rom);
    original.bus.write16(0x0400_0000, 0x0000); // mode 0, no layers: backdrop
    original.bus.write16(0x0500_0000, 0x7C00);
    original.run_frame();
    while !(original.ppu.scanline == 50 && original.ppu.hblank) {
        original.step();
    }
    let mut fresh = machine(&rom);
    fresh
        .load_state(&original.save_state())
        .expect("own state loads");
    // What an HBlank handler does: change the colour for the next line.
    for gba in [&mut original, &mut fresh] {
        gba.bus.write16(0x0500_0000, 0x001F);
        while gba.ppu.scanline == 50 {
            gba.step();
        }
    }
    let line = 50 * 240 * 3..51 * 240 * 3;
    assert!(
        fresh.frame_buffer()[line.clone()] == original.frame_buffer()[line],
        "line 50 was drawn again after the restore"
    );
}

/// The PPU raises the VCount IRQ on the edge of the line matching LYC. A
/// state taken on that line, IRQ already raised and acknowledged, restored
/// into a machine that had seen no match raised it a second time.
#[test]
fn a_state_on_the_vcount_line_does_not_raise_the_irq_again() {
    let rom = rom(&SPIN);
    let mut original = machine(&rom);
    original.bus.write16(0x0400_0004, 0x2820); // LYC 40, VCount IRQ enable
    original.bus.io.ie = 0x0004;
    while original.ppu.scanline != 40 {
        original.step();
    }
    original.step();
    assert_eq!(original.bus.io.if_ & 0x0004, 0x0004, "VCount IRQ raised");
    original.bus.io.if_ = 0;
    assert_resumes_alike(&mut original, &mut machine(&rom), 50, "on the LYC line");
}
