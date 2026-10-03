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
