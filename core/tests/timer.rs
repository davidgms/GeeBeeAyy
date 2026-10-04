//! Timer overflow and cascade regression tests. No ROM or BIOS image needed.

use geebeeayy_core::memory::MemoryBus;
use geebeeayy_core::timer::Timer;

const IF: u32 = 0x0400_0202;

/// TMxCNT_H: bit 7 enable, bit 6 IRQ enable, bit 2 cascade, bits 0-1 prescaler.
const ENABLE: u16 = 0x0080;
const IRQ: u16 = 0x0040;
const CASCADE: u16 = 0x0004;

/// A cascaded timer is skipped by `tick` and only counted up by the timer
/// below it. That increment used to have no overflow check of its own, so a
/// cascaded timer ran past 0x10000 forever: never reloading, never flagging an
/// overflow for DMA sound, and never raising its IRQ.
#[test]
fn a_cascaded_timer_overflows_reloads_and_raises_its_irq() {
    let mut bus = MemoryBus::new();
    let mut timer = Timer::new();

    // Timer 0: prescaler 1, reloads at 0xFFFF, so it overflows every count.
    timer.set_reload(0, 0xFFFF);
    timer.set_control(0, ENABLE);
    // Timer 1: cascaded off timer 0, also one count from overflowing.
    timer.set_reload(1, 0xFFFF);
    timer.set_control(1, ENABLE | CASCADE | IRQ);

    timer.tick(1, &mut bus);

    assert_eq!(
        timer.counter(1),
        0xFFFF,
        "the cascaded timer should have overflowed and reloaded"
    );
    assert_eq!(
        timer.drain_overflows()[1],
        1,
        "the cascaded timer's overflow was never counted, so DMA sound never pops a sample"
    );
    assert_ne!(
        bus.read16(IF) & 0x0010,
        0,
        "timer 1 overflow must set IF bit 4"
    );
}

/// The cascade chains: timer 0 overflowing can carry all the way to timer 3.
#[test]
fn a_cascade_carries_through_the_whole_chain() {
    let mut bus = MemoryBus::new();
    let mut timer = Timer::new();

    timer.set_reload(0, 0xFFFF);
    timer.set_control(0, ENABLE);
    for t in 1..4 {
        timer.set_reload(t, 0xFFFF);
        timer.set_control(t, ENABLE | CASCADE);
    }

    timer.tick(1, &mut bus);

    let overflows = timer.drain_overflows();
    assert_eq!(
        overflows,
        [1, 1, 1, 1],
        "the carry stopped part way: {overflows:?}"
    );
}

/// A cascade target that is not enabled does not count up.
#[test]
fn a_disabled_cascade_target_is_not_counted() {
    let mut bus = MemoryBus::new();
    let mut timer = Timer::new();

    timer.set_reload(0, 0xFFFF);
    timer.set_control(0, ENABLE);
    timer.set_reload(1, 0x0000);
    timer.set_control(1, CASCADE); // cascaded, but never enabled

    timer.tick(1, &mut bus);

    assert_eq!(timer.counter(1), 0);
    assert_eq!(timer.drain_overflows()[1], 0);
}

/// The plain, non-cascaded path still works: a prescaled timer overflows after
/// the right number of cycles and reloads rather than wrapping to zero.
#[test]
fn a_prescaled_timer_overflows_after_its_full_period() {
    let mut bus = MemoryBus::new();
    let mut timer = Timer::new();

    timer.set_reload(0, 0xFFFE);
    timer.set_control(0, ENABLE | 1); // prescaler 64

    timer.tick(64, &mut bus);
    assert_eq!(timer.counter(0), 0xFFFF);
    assert_eq!(timer.drain_overflows()[0], 0);

    timer.tick(64, &mut bus);
    assert_eq!(
        timer.counter(0),
        0xFFFE,
        "overflow must reload, not wrap to 0"
    );
    assert_eq!(timer.drain_overflows()[0], 1);
}

/// TMxCNT_H's enable bit lives in its low byte, at 0x4000102 for timer 0.
/// The bus only handed a timer write to the timer unit on the register's
/// high byte, so `strb` to the control byte never started the timer.
#[test]
fn a_byte_write_to_the_control_register_starts_the_timer() {
    let mut gba = geebeeayy_core::Gba::new();
    gba.load_rom(&vec![0u8; 0x200]).expect("ROM should load");
    gba.bus.write16(0x0400_0100, 0xFFF0); // TM0CNT_L reload
    gba.bus.write8(0x0400_0102, (ENABLE | IRQ) as u8);
    gba.run_frame();
    assert_ne!(
        gba.bus.io.if_ & 0x0008,
        0,
        "timer 0 never overflowed: the byte write did not start it"
    );
}

/// Run the mGBA suite's timer-IRQ probe (`src/timer-irq.c`) from IWRAM: start
/// timer 0 at `start` with IRQ enabled via `str r3, [r1]` (TM0CNT_L = start,
/// TM0CNT_H = 0xC0), set the reload to 0 with `strh r2, [r1]`, run `nops`
/// NOPs, then `ldrh` the counter. The IRQ handler stops the timer, so a read
/// the IRQ got in front of sees a frozen, much later value.
fn timer_irq_probe(start: u16, nops: usize) -> u16 {
    const CODE: u32 = 0x0300_0000;
    const HANDLER: u32 = 0x0300_0400;
    const RESULT: u32 = 0x0300_0800;
    let mut gba = geebeeayy_core::Gba::new();
    gba.load_rom(&vec![0u8; 0x200]).expect("ROM should load");

    let mut code = vec![
        0xE581_3000, // str r3, [r1]
        0xE1C1_20B0, // strh r2, [r1]
    ];
    code.extend(std::iter::repeat_n(0xE1A0_0000, nops)); // mov r0, r0
    code.extend([
        0xE1D1_00B0, // ldrh r0, [r1]
        0xE1C7_00B0, // strh r0, [r7]
        0xEAFF_FFFE, // b .
    ]);
    let handler = [
        0xE3A0_C301, // mov r12, #0x04000000
        0xE28C_CC01, // add r12, r12, #0x100
        0xE3A0_0000, // mov r0, #0
        0xE1CC_00B2, // strh r0, [r12, #2]   ; TM0CNT_H = 0, timer stops
        0xE28C_CC01, // add r12, r12, #0x100
        0xE3A0_0008, // mov r0, #8
        0xE1CC_00B2, // strh r0, [r12, #2]   ; acknowledge IF bit 3
        0xE12F_FF1E, // bx lr
    ];
    for (i, &w) in code.iter().enumerate() {
        gba.bus.write32(CODE + 4 * i as u32, w);
    }
    for (i, &w) in handler.iter().enumerate() {
        gba.bus.write32(HANDLER + 4 * i as u32, w);
    }
    gba.bus.write32(0x0300_7FFC, HANDLER);
    gba.bus.write16(0x0400_0200, 0x0008); // IE: timer 0
    gba.bus.write16(0x0400_0208, 1); // IME
    gba.bus.write16(RESULT, 0xBEEF);

    gba.cpu.registers[1] = 0x0400_0100;
    gba.cpu.registers[2] = 0x00C0_0000;
    gba.cpu.registers[3] = 0x00C0_0000 | u32::from(start);
    gba.cpu.registers[7] = RESULT;
    gba.cpu.registers[15] = CODE;
    for _ in 0..200 {
        gba.step();
    }
    gba.bus.read16(RESULT)
}

/// What hardware reads in the mGBA suite's timer-IRQ test, for the reads the
/// IRQ does not get in front of. A timer enabled by a store starts counting
/// one cycle after that store ends, a reload written in the same cycle as an
/// overflow is not yet the one reloaded, and `ldrh` samples the counter in
/// its second cycle. The `None`s are where hardware takes the IRQ first:
/// four cycles after the overflow, the IRQ wins the next instruction boundary.
#[test]
fn timer_reads_and_irq_latency_match_the_mgba_timer_irq_suite() {
    let cases: [(u16, [Option<u16>; 7]); 3] = [
        (
            0xFFFF,
            [Some(0), Some(1), Some(2), Some(3), None, None, None],
        ),
        (
            0xFFFE,
            [Some(0), Some(1), Some(2), Some(3), Some(4), None, None],
        ),
        (
            0xFFFD,
            [
                Some(0xFFFF),
                Some(0),
                Some(1),
                Some(2),
                Some(3),
                Some(4),
                None,
            ],
        ),
    ];
    for (start, expected) in cases {
        for (nops, want) in expected.into_iter().enumerate() {
            let got = timer_irq_probe(start, nops);
            match want {
                Some(value) => assert_eq!(
                    got, value,
                    "start {start:04X}, {nops} nops: counter read wrong"
                ),
                // Frozen by the handler well after the overflow, and nowhere
                // near the 4 or 5 the uninterrupted read would see.
                None => assert!(
                    (0x10..0x100).contains(&got),
                    "start {start:04X}, {nops} nops: the IRQ should have stopped the \
                     timer before the read, got {got:04X}"
                ),
            }
        }
    }
}

/// Lowering the prescaler of a running timer leaves its cycle accumulator
/// past the new period. The next count is then simply due, never an
/// underflow.
#[test]
fn lowering_the_prescaler_of_a_running_timer_keeps_counting() {
    let mut bus = MemoryBus::new();
    let mut timer = Timer::new();
    timer.set_control(0, ENABLE | 3); // prescaler 1024
    timer.tick(500, &mut bus);
    timer.set_control(0, ENABLE); // prescaler 1, still running
    timer.tick(3, &mut bus);
    assert_eq!(timer.counter(0), 3);
}

/// `Timer::tick` skips straight to the end of a slice when no overflow falls
/// in it. That shortcut must be invisible: ticking a slice in one call has to
/// leave every counter, overflow count and IF bit exactly where ticking it one
/// cycle at a time does, and report the first IRQ at the same cycle. Random
/// reloads near 0xFFFF, every prescaler, cascades, start delays and
/// prescalers changed on running timers, from a fixed seed.
#[test]
fn ticking_a_slice_at_once_matches_ticking_it_cycle_by_cycle() {
    let mut seed = 0x2545_F491_4F6C_DD1Du64;
    let mut rand = move |n: u32| {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed % u64::from(n)) as u32
    };
    for round in 0..200 {
        let (mut bus_a, mut bus_b) = (MemoryBus::new(), MemoryBus::new());
        let (mut bulk, mut stepped) = (Timer::new(), Timer::new());
        let configure =
            |bulk: &mut Timer, stepped: &mut Timer, rand: &mut dyn FnMut(u32) -> u32| {
                let i = rand(4) as usize;
                let reload = 0xFFFF - rand(300) as u16;
                let mut control = ENABLE | rand(4) as u16;
                if rand(2) == 0 {
                    control |= IRQ;
                }
                if i > 0 && rand(3) == 0 {
                    control |= CASCADE;
                }
                for t in [&mut *bulk, &mut *stepped] {
                    t.set_reload(i, reload);
                    t.set_control(i, control);
                }
                if rand(2) == 0 {
                    let delay = rand(3);
                    bulk.delay_start(i, delay);
                    stepped.delay_start(i, delay);
                }
            };
        for _ in 0..4 {
            configure(&mut bulk, &mut stepped, &mut rand);
        }
        for slice in 0..300 {
            if rand(40) == 0 {
                configure(&mut bulk, &mut stepped, &mut rand);
            }
            let long = rand(10) == 0;
            let cycles = 1 + rand(if long { 2000 } else { 12 });
            let want_irq = bulk.tick(cycles, &mut bus_a);
            let mut got_irq = None;
            for c in 1..=cycles {
                if stepped.tick(1, &mut bus_b).is_some() && got_irq.is_none() {
                    got_irq = Some(c);
                }
            }
            let at = format!("round {round}, slice {slice} ({cycles} cycles)");
            // `tick` reports the IRQ of the lowest-numbered timer that raised
            // one, which is not always the earliest when two timers fire in
            // one slice, so only the stepped run's earliest is a lower bound.
            assert_eq!(want_irq.is_some(), got_irq.is_some(), "{at}: IRQ raised");
            assert!(want_irq >= got_irq, "{at}: IRQ before the first one");
            assert_eq!(bulk.counters(), stepped.counters(), "{at}: counters");
            let raw = |t: &Timer| [0, 1, 2, 3].map(|i| t.counter(i));
            assert_eq!(raw(&bulk), raw(&stepped), "{at}: raw counters");
            assert_eq!(
                bulk.drain_overflows(),
                stepped.drain_overflows(),
                "{at}: overflows"
            );
            assert_eq!(bus_a.read16(IF), bus_b.read16(IF), "{at}: IF");
        }
    }
}

/// A machine with `code` (ARM words) in IWRAM and the CPU about to run it.
fn iwram_machine(code: &[u32]) -> geebeeayy_core::Gba {
    const CODE: u32 = 0x0300_0000;
    let mut gba = geebeeayy_core::Gba::new();
    gba.load_rom(&vec![0u8; 0x200]).expect("ROM should load");
    for (i, &w) in code.iter().enumerate() {
        gba.bus.write32(CODE + 4 * i as u32, w);
    }
    gba.cpu.registers[1] = 0x0400_0100;
    gba.cpu.registers[15] = CODE;
    gba
}

/// A timer IRQ ends HALT on the cycle the timer overflows. The halted path
/// used to advance straight to the next PPU event, so the CPU woke up to a
/// thousand cycles late, at a point that depended on where in the scanline
/// the overflow fell. The mGBA suite's count-up test syncs on exactly such a
/// wake (`IntrWait` on a timer IRQ), and that jitter alone made its prescaled
/// cases impossible to match.
#[test]
fn a_timer_irq_ends_halt_on_the_overflow_cycle() {
    let mut gba = iwram_machine(&[
        0xE581_3000, // str r3, [r1]   ; TM0CNT = 0xF000, prescaler 1, IRQ, enable
        0xEF02_0000, // swi 0x02       ; Halt
        0xEAFF_FFFE, // b .
    ]);
    gba.cpu.registers[3] = u32::from(ENABLE | IRQ) << 16 | 0xF000;
    gba.bus.write16(0x0400_0200, 0x0008); // IE: timer 0. IME stays 0.

    gba.step(); // str
                // The timer counts from one cycle after the store: 0x1000 counts later
                // it overflows. Not on a PPU event: those fall at 960 and 1232 cycles
                // into each 1232-cycle scanline.
    let overflow = gba.cycles + 1 + 0x1000;
    assert_ne!(overflow % 1232, 960);
    assert_ne!(overflow % 1232, 0);
    gba.step(); // swi
    assert!(gba.bus.io.halt, "SWI 2 should halt");
    while gba.bus.io.halt || gba.cpu.halted {
        gba.step();
    }
    assert_eq!(gba.cycles, overflow, "HALT ended off the overflow cycle");
}

/// A prescaled timer counts on the edges of a free-running prescaler, not on
/// a divider restarted by the enable. Starting it a few cycles later moves its
/// first count only when that crosses a prescaler edge. mGBA models it this
/// way (`src/gba/timer.c`, `currentTime &= ~tickMask`), and the mGBA suite's
/// count-up results recorded on hardware need it: 15 extra cycles before the
/// start lose six loop passes at prescaler 64 but gain two at 256 and 1024.
#[test]
fn a_prescaled_timer_counts_on_the_global_prescaler_edges() {
    let mut overflows = Vec::new();
    for lead in 0..70 {
        let mut code = vec![0xE1A0_0000; lead]; // mov r0, r0: one cycle each
        code.push(0xE581_3000); // str r3, [r1]   ; TM0CNT = 0xFFFF, prescaler 64, IRQ (IE stays 0)
        code.extend([0xE1A0_0000; 200]);
        code.push(0xEAFF_FFFE); // b .
        let mut gba = iwram_machine(&code);
        gba.cpu.registers[3] = u32::from(ENABLE | IRQ | 1) << 16 | 0xFFFF;
        while gba.bus.io.if_ & 0x0008 == 0 {
            gba.step();
        }
        overflows.push(gba.cycles);
    }
    let phase = overflows[0] % 64;
    for (lead, at) in overflows.iter().enumerate() {
        assert_eq!(
            at % 64,
            phase,
            "{lead} cycles of lead: overflow at {at} is off the 64-cycle grid"
        );
    }
}

/// One case of the mGBA suite's timer count-up test (`src/timers.c`, `runTest`,
/// the tight-loop half), instruction for instruction, with the ROM's own
/// libgba IRQ dispatcher and `testIrq` handler.
///
/// Sync on a prescaler-1024 timer IRQ through `IntrWait`, spin `delay` passes
/// of a delay loop, start timer 0 with `control_reload` and count passes of
/// `add; ldr; tst; bne` until the IRQ handler stops it. Returns the pass count
/// and the counter it read last.
fn count_up_case(control_reload: u32, delay: u32) -> (u32, u16) {
    const IRQ_COUNTER: u32 = 0x0300_0200;
    const RESULT: u32 = 0x0300_0300;
    const DISPATCHER: u32 = 0x0300_2D88;
    const IRQ_TABLE: u32 = 0x0300_3358;
    const TEST_IRQ: u32 = 0x0300_0100;

    let mut gba = iwram_machine(&[
        0xE3A0_0000, // mov r0, #0
        0xE1C3_00B0, // strh r0, [r3]
        0xE584_5000, // str r5, [r4]       ; sync timer: 0xFFFE, prescaler 1024, IRQ
        0xE3A0_0001, // mov r0, #1
        0xE586_0000, // str r0, [r6]       ; irqCounter = 1
        0xE3A0_1008, // mov r1, #8
        0xEF04_0000, // swi 0x04           ; IntrWait(1, timer 0)
        0xE586_9000, // str r9, [r6]       ; irqCounter = irqs
        0xE3A0_0001, // mov r0, #1
        0xE150_000B, // cmp r0, r11        ; delay loop
        0x1280_0001, // addne r0, r0, #1
        0x1AFF_FFFC, // bne cmp
        0xE3A0_0000, // mov r0, #0
        0xE584_8000, // str r8, [r4]       ; start the timer under test
        0xE280_0001, // add r0, r0, #1
        0xE594_2000, // ldr r2, [r4]
        0xE312_0502, // tst r2, #0x800000  ; still enabled?
        0x1AFF_FFFB, // bne add
        0xE58C_0000, // str r0, [r12]
        0xE1CC_20B8, // strh r2, [r12, #8]
        0xEAFF_FFFE, // b .
    ]);
    // libgba's IntrMain as the suite ROM installs it.
    let dispatcher: [u32; 46] = [
        0xE3A0_3301,
        0xE593_2200,
        0xE593_1208,
        0xE583_3208,
        0xE14F_0000,
        0xE92D_400B,
        0xE002_1822,
        0xE153_20B8,
        0xE182_2001,
        0xE143_20B8,
        0xE59F_2084,
        0xE283_3C02,
        0xE592_0004,
        0xE350_0000,
        0x0A00_0003,
        0xE010_0001,
        0x1A00_0005,
        0xE282_2008,
        0xEAFF_FFF8,
        0xE1C3_10B2,
        0xE8BD_400B,
        0xE583_1208,
        0xE1A0_F00E,
        0xE592_2000,
        0xE352_0000,
        0x0AFF_FFF8,
        0xE10F_1000,
        0xE3C1_10DF,
        0xE381_101F,
        0xE129_F001,
        0xE1C3_00B2,
        0xE52D_E004,
        0xE28F_E000,
        0xE12F_FF12,
        0xE49D_E004,
        0xE3A0_3301,
        0xE583_3208,
        0xE10F_3000,
        0xE3C3_30DF,
        0xE383_3092,
        0xE129_F003,
        0xE8BD_400B,
        0xE583_1208,
        0xE169_F000,
        0xE1A0_F00E,
        IRQ_TABLE,
    ];
    for (i, &w) in dispatcher.iter().enumerate() {
        gba.bus.write32(DISPATCHER + 4 * i as u32, w);
    }
    // testIrq, THUMB: if (!--irqCounter) TM0CNT_H = 0;
    let test_irq: [u16; 10] = [
        0x4A04, 0x6813, 0x3B01, 0x6013, 0x2B00, 0xD101, 0x4A02, 0x8013, 0x4770, 0x46C0,
    ];
    for (i, &h) in test_irq.iter().enumerate() {
        gba.bus.write16(TEST_IRQ + 2 * i as u32, h);
    }
    gba.bus.write32(TEST_IRQ + 20, IRQ_COUNTER);
    gba.bus.write32(TEST_IRQ + 24, 0x0400_0102);
    // IntrTable: { handler, mask }, zero-terminated.
    gba.bus.write32(IRQ_TABLE, TEST_IRQ | 1);
    gba.bus.write32(IRQ_TABLE + 4, 0x0008);
    gba.bus.write32(IRQ_TABLE + 8, 0);
    gba.bus.write32(IRQ_TABLE + 12, 0);
    gba.bus.write32(0x0300_7FFC, DISPATCHER);
    gba.bus.write16(0x0400_0200, 0x0008); // IE: timer 0
    gba.bus.write16(0x0400_0208, 1); // IME

    gba.cpu.set_cpsr(0x1F); // System mode, IRQs on, as the suite runs
    gba.cpu.registers[13] = 0x0300_7F00;
    gba.cpu.registers[3] = 0x0300_0400; // activeTestInfo, a scratch halfword
    gba.cpu.registers[4] = 0x0400_0100;
    gba.cpu.registers[5] = 0x00C3_FFFE;
    gba.cpu.registers[6] = IRQ_COUNTER;
    gba.cpu.registers[8] = control_reload;
    gba.cpu.registers[9] = 1; // one IRQ
    gba.cpu.registers[11] = delay;
    gba.cpu.registers[12] = RESULT;
    gba.bus.write32(RESULT, 0xDEAD_BEEF);
    for _ in 0..200_000 {
        gba.step();
        if gba.bus.read32(RESULT) != 0xDEAD_BEEF && gba.cpu.registers[15] == 0x0300_0050 {
            break;
        }
    }
    (gba.bus.read32(RESULT), gba.bus.read16(RESULT + 8))
}

/// Hardware's results for the mGBA suite's count-up test, "1d 1i" and "4d 1i"
/// of the prescaled rows, plus one prescaler-1 row that always passed. Fifteen
/// cycles of extra delay cost six loop passes at prescaler 64 but gain two at
/// 256 and 1024: the start moved across a global prescaler edge in the first
/// case only. Matching all of them needs three things at once - prescaler
/// edges on the global clock, HALT ending on the overflow cycle, and
/// `IntrWait` taking as long to return as the BIOS code it stands for.
#[test]
fn timer_count_up_matches_the_mgba_suite() {
    let cases = [
        (0x00C0_FFC0, 1, 0x00A, 0xFFDC),
        (0x00C1_FFF0, 1, 0x07B, 0xFFF1),
        (0x00C1_FFF0, 4, 0x081, 0xFFF1),
        (0x00C2_FFF0, 1, 0x1EB, 0xFFF0),
        (0x00C2_FFF0, 4, 0x1E9, 0xFFF0),
        (0x00C3_FFF0, 1, 0x7EB, 0xFFF0),
        (0x00C3_FFF0, 4, 0x7E9, 0xFFF0),
    ];
    for (timer, delay, passes, value) in cases {
        assert_eq!(
            count_up_case(timer, delay),
            (passes, value),
            "timer {timer:06X}, delay {delay}: (loop passes, counter)"
        );
    }
}
