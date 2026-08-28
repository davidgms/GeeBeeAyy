//! Input, interrupt-flag and exception-entry regression tests.
//!
//! Nothing here needs a ROM or a BIOS image: the HLE IRQ handler that
//! `MemoryBus::new` installs at 0x18 is exercised directly.

use geebeeayy_core::cpu::{Cpu, Mode};
use geebeeayy_core::memory::MemoryBus;
use geebeeayy_core::timer::Timer;
use geebeeayy_core::dma::Dma;

const BASE: u32 = 0x0300_0000;
const KEYINPUT: u32 = 0x0400_0130;

// ---------------------------------------------------------------------------
// 0.1 - keypad input
// ---------------------------------------------------------------------------

#[test]
fn keyinput_reads_all_released_on_a_fresh_bus() {
    let bus = MemoryBus::new();
    // GBATEK, GBA Keypad Input: "0=Pressed, 1=Released". Nothing held reads 0x03FF.
    assert_eq!(
        bus.read16(KEYINPUT),
        0x03FF,
        "an unpolled pad must read as nothing pressed, not everything pressed"
    );
}

#[test]
fn set_keys_pulls_the_pressed_bits_low() {
    let mut bus = MemoryBus::new();
    // A (bit 0) and Start (bit 3) held; the FFI polarity is 1 = pressed.
    bus.set_keys(0b0000_1001);
    assert_eq!(bus.read16(KEYINPUT), 0x03FF & !0b0000_1001);

    bus.set_keys(0);
    assert_eq!(bus.read16(KEYINPUT), 0x03FF);

    // Bits above 9 do not exist and must not leak into the register.
    bus.set_keys(0xFFFF);
    assert_eq!(bus.read16(KEYINPUT), 0x0000);
}

// ---------------------------------------------------------------------------
// 0.2 - interrupt flags that never fired
// ---------------------------------------------------------------------------

#[test]
fn timer_overflow_sets_its_own_if_bit() {
    // GBATEK, GBA Interrupt Control: IF bits 3,4,5,6 = Timer 0,1,2,3 overflow.
    for i in 0..4usize {
        let mut bus = MemoryBus::new();
        let mut timer = Timer::new();
        timer.set_reload(i, 0xFFFF);
        // Enable + IRQ + prescaler 1.
        timer.set_control(i, 0x00C0);
        timer.tick(2, &mut bus);
        assert_eq!(
            bus.io.if_,
            1 << (3 + i),
            "timer {i} overflow must raise IF bit {}",
            3 + i
        );
    }
}

#[test]
fn timer_overflow_without_irq_enabled_leaves_if_alone() {
    let mut bus = MemoryBus::new();
    let mut timer = Timer::new();
    timer.set_reload(0, 0xFFFF);
    timer.set_control(0, 0x0080); // enable, no IRQ
    timer.tick(2, &mut bus);
    assert_eq!(bus.io.if_, 0);
}

#[test]
fn dma_completion_sets_its_own_if_bit() {
    // GBATEK, GBA Interrupt Control: IF bits 8,9,10,11 = DMA 0,1,2,3.
    for ch in 0..4usize {
        let mut bus = MemoryBus::new();
        let mut dma = Dma::new();
        dma.write_sad(ch, BASE);
        dma.write_dad(ch, BASE + 0x100);
        dma.write_count(ch, 4);
        // Enable | IRQ on end, timing 0 (immediate).
        dma.write_control(ch, 0x8000 | 0x4000, &mut bus);
        assert_eq!(
            bus.io.if_,
            1 << (8 + ch),
            "DMA {ch} completion must raise IF bit {}",
            8 + ch
        );
    }
}

#[test]
fn dma_completion_without_irq_leaves_if_alone() {
    let mut bus = MemoryBus::new();
    let mut dma = Dma::new();
    dma.write_sad(0, BASE);
    dma.write_dad(0, BASE + 0x100);
    dma.write_count(0, 4);
    dma.write_control(0, 0x8000, &mut bus);
    assert_eq!(bus.io.if_, 0);
}

// ---------------------------------------------------------------------------
// 0.3 - banked registers and IRQ entry
// ---------------------------------------------------------------------------

#[test]
fn boot_sets_the_three_banked_stack_pointers() {
    let mut cpu = Cpu::new();
    cpu.boot();
    assert_eq!(cpu.registers[13], 0x0300_7F00, "sp_sys");
    cpu.set_cpsr((cpu.cpsr & !0x1F) | Mode::Supervisor as u32);
    assert_eq!(cpu.registers[13], 0x0300_7FE0, "sp_svc");
    cpu.set_cpsr((cpu.cpsr & !0x1F) | Mode::Irq as u32);
    assert_eq!(cpu.registers[13], 0x0300_7FA0, "sp_irq");
}

#[test]
fn fiq_banks_r8_to_r12_and_irq_does_not() {
    // GBATEK, ARM CPU Register Set: only FIQ banks R8-R12; SVC/ABT/IRQ/UND
    // bank only R13 and R14.
    let mut cpu = Cpu::new();
    cpu.registers[8] = 0xAAAA_AAAA;
    cpu.registers[12] = 0xCCCC_CCCC;

    cpu.set_cpsr((cpu.cpsr & !0x1F) | Mode::Fiq as u32);
    assert_eq!(cpu.registers[8], 0, "FIQ must see its own R8");
    assert_eq!(cpu.registers[12], 0, "FIQ must see its own R12");
    cpu.registers[8] = 0x1111_1111;

    cpu.set_cpsr((cpu.cpsr & !0x1F) | Mode::System as u32);
    assert_eq!(cpu.registers[8], 0xAAAA_AAAA, "User R8 must survive FIQ");
    assert_eq!(cpu.registers[12], 0xCCCC_CCCC);

    cpu.set_cpsr((cpu.cpsr & !0x1F) | Mode::Irq as u32);
    assert_eq!(cpu.registers[8], 0xAAAA_AAAA, "IRQ must not bank R8");
    assert_eq!(cpu.registers[12], 0xCCCC_CCCC, "IRQ must not bank R12");

    cpu.set_cpsr((cpu.cpsr & !0x1F) | Mode::Fiq as u32);
    assert_eq!(cpu.registers[8], 0x1111_1111, "FIQ R8 must survive the trip");
}

#[test]
fn irq_entry_from_thumb_clears_t_and_switches_to_the_irq_stack() {
    // GBATEK, ARM CPU Exceptions: LR_irq = return address + 4, SPSR_irq = CPSR,
    // T = 0, I = 1, mode = IRQ.
    let mut cpu = Cpu::new();
    cpu.boot();
    cpu.cpsr |= 0x20; // interrupted code was running in Thumb
    cpu.registers[15] = 0x0800_0100;

    cpu.handle_irq();

    assert_eq!(cpu.cpsr & 0x20, 0, "the handler always runs in ARM state");
    assert_eq!(cpu.cpsr & 0x1F, Mode::Irq as u32);
    assert_ne!(cpu.cpsr & 0x80, 0, "I must be set on entry");
    assert_ne!(cpu.spsr_irq & 0x20, 0, "SPSR_irq keeps the interrupted T bit");
    assert_eq!(cpu.registers[14], 0x0800_0104, "LR_irq = return address + 4");
    assert_eq!(cpu.registers[13], 0x0300_7FA0, "the handler runs on sp_irq");
    assert_eq!(cpu.registers[15], 0x0000_0018);
}

#[test]
fn irq_entry_leaves_the_user_stack_pointer_untouched() {
    let mut cpu = Cpu::new();
    cpu.boot();
    cpu.registers[13] = 0x0300_7EE0; // the game moved its own stack

    cpu.handle_irq();
    assert_eq!(cpu.registers[13], 0x0300_7FA0);
    cpu.registers[13] -= 24; // as the handler's stmdb would

    cpu.set_cpsr(cpu.spsr_irq); // as `subs pc, lr, #4` would
    assert_eq!(
        cpu.registers[13], 0x0300_7EE0,
        "IRQ must not run on the game's own stack"
    );
}

#[test]
fn irq_returns_to_the_interrupted_instruction() {
    let mut bus = MemoryBus::new();

    // Game IRQ handler: mov r5, #0x42 ; bx lr
    bus.write32(0x0300_1000, 0xE3A0_5042);
    bus.write32(0x0300_1004, 0xE12F_FF1E);
    bus.write32(0x0300_7FFC, 0x0300_1000);

    // Interrupted code: mov r0, #1 ; mov r1, #2
    bus.write32(BASE, 0xE3A0_0001);
    bus.write32(BASE + 4, 0xE3A0_1002);

    let mut cpu = Cpu::new();
    cpu.boot();
    cpu.registers[15] = BASE;
    cpu.step(&mut bus);
    assert_eq!(cpu.registers[15], BASE + 4);
    let cpsr_before = cpu.cpsr;

    bus.io.ie = 0x0001;
    bus.io.ime = 1;
    bus.io.request_interrupt(0x0001);
    cpu.handle_irq();

    for _ in 0..64 {
        if cpu.mode() == Mode::System {
            break;
        }
        cpu.step(&mut bus);
    }

    assert_eq!(cpu.registers[5], 0x42, "the game handler never ran");
    assert_eq!(
        cpu.registers[15],
        BASE + 4,
        "IRQ return landed on the wrong instruction"
    );
    assert_eq!(cpu.cpsr, cpsr_before, "CPSR was not restored from SPSR_irq");
    assert_eq!(cpu.registers[13], 0x0300_7F00, "sp_sys was clobbered");
    assert_eq!(bus.io.if_, 0, "the HLE handler must acknowledge IF");

    // And the interrupted instruction stream resumes correctly.
    cpu.step(&mut bus);
    assert_eq!(cpu.registers[1], 2);
}

// ---------------------------------------------------------------------------
// Memory map: mirroring and the 8-bit video write rules
// ---------------------------------------------------------------------------

#[test]
fn ram_regions_mirror_through_their_blocks() {
    let mut bus = MemoryBus::new();
    bus.write32(0x0200_0000, 0xDEAD_BEEF);
    assert_eq!(bus.read32(0x0204_0000), 0xDEAD_BEEF, "EWRAM mirrors every 256 KB");

    bus.write32(0x0300_0000, 0xCAFE_BABE);
    assert_eq!(bus.read32(0x0300_8000), 0xCAFE_BABE, "IWRAM mirrors every 32 KB");

    bus.write16(0x0500_0010, 0x1234);
    assert_eq!(bus.read16(0x0500_0410), 0x1234, "palette mirrors every 1 KB");

    bus.write16(0x0700_0010, 0x5678);
    assert_eq!(bus.read16(0x0700_0410), 0x5678, "OAM mirrors every 1 KB");
}

#[test]
fn vram_upper_block_folds_back() {
    // VRAM mirrors in 128 KB steps but holds 96 KB: 0x18000..0x20000 is a
    // second view of 0x10000..0x18000.
    let mut bus = MemoryBus::new();
    bus.write16(0x0601_0000, 0xABCD);
    assert_eq!(bus.read16(0x0601_8000), 0xABCD);
}

#[test]
fn byte_writes_to_video_memory_follow_the_gba_rules() {
    let mut bus = MemoryBus::new();

    // OAM ignores byte writes entirely.
    bus.write16(0x0700_0010, 0x0000);
    bus.write8(0x0700_0010, 0xFF);
    assert_eq!(bus.read16(0x0700_0010), 0x0000, "OAM must ignore byte stores");

    // Palette duplicates the byte across the halfword.
    bus.write8(0x0500_0020, 0xAB);
    assert_eq!(bus.read16(0x0500_0020), 0xABAB);

    // A halfword store must NOT be treated as two byte stores.
    bus.write16(0x0500_0020, 0x1234);
    assert_eq!(bus.read16(0x0500_0020), 0x1234, "write16 leaked into the byte path");

    // VRAM: BG half duplicates, OBJ half ignores. Mode 0 puts OBJ at 0x10000.
    bus.write16(0x0400_0000, 0x0000);
    bus.write8(0x0600_0040, 0xCD);
    assert_eq!(bus.read16(0x0600_0040), 0xCDCD, "BG VRAM duplicates the byte");
    bus.write16(0x0601_0040, 0x0000);
    bus.write8(0x0601_0040, 0xEE);
    assert_eq!(bus.read16(0x0601_0040), 0x0000, "OBJ VRAM must ignore byte stores");
}
