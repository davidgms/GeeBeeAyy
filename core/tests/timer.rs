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
