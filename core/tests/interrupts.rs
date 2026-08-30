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

    // Game IRQ handler: mov r5, #0x42 ; acknowledge IF ; bx lr. The
    // acknowledge is the handler's job on hardware - the BIOS never touches
    // IF - so the test handler does what a real one does.
    bus.write32(0x0300_1000, 0xE3A0_5042); // mov r5, #0x42
    bus.write32(0x0300_1004, 0xE3A0_0301); // mov r0, #0x04000000
    bus.write32(0x0300_1008, 0xE2800C02); // add r0, r0, #0x200
    bus.write32(0x0300_100C, 0xE1D010B2); // ldrh r1, [r0, #2]
    bus.write32(0x0300_1010, 0xE1C010B2); // strh r1, [r0, #2]
    bus.write32(0x0300_1014, 0xE12F_FF1E); // bx lr
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
    assert_eq!(bus.io.if_, 0, "the game handler's IF acknowledge was lost");

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

#[test]
fn a_halted_cpu_is_woken_by_vblank() {
    // The halted path used to tick the PPU without routing its pending flags
    // into IF, so the wake condition could never become true and every game
    // that halts waiting for VBlank - the standard main loop - deadlocked.
    //
    // GBATEK, BIOS Halt Functions: "Halt mode is terminated when any enabled
    // interrupts are requested, that is when (IE AND IF) is not zero... the
    // state of CPUs IRQ disable bit in CPSR register, and the IME register are
    // don't care".
    use geebeeayy_core::Gba;
    let mut gba = Gba::new();
    let mut data = vec![0u8; 0x200];
    data[0..4].copy_from_slice(&0xEAFF_FFFEu32.to_le_bytes()); // b .
    gba.load_rom(&data).unwrap();

    // DISPSTAT bit 3 is what makes the PPU raise a VBlank IRQ at all; IE then
    // decides whether it counts as "enabled" for the halt wake-up.
    gba.bus.write16(0x0400_0004, 0x0008);
    gba.bus.io.ie = 0x0001;
    gba.bus.io.ime = 0; // deliberately masked: it must not matter
    gba.bus.io.halt = true;

    // A halted step advances to the next PPU event, not a whole scanline, so
    // reaching line 160 takes rather more than 160 of them.
    for _ in 0..1000 {
        gba.step();
        if !gba.bus.io.halt {
            break;
        }
    }
    assert!(!gba.bus.io.halt, "VBlank never woke the halted CPU");
    assert_ne!(gba.bus.io.if_ & 1, 0, "the VBlank flag never reached IF");
}

#[test]
fn intr_wait_forces_ime() {
    // GBATEK, SWI 04h: "The function forcefully sets IME=1."
    use geebeeayy_core::Gba;
    let mut gba = Gba::new();
    let mut data = vec![0u8; 0x200];
    data[0..4].copy_from_slice(&0xEF05_0000u32.to_le_bytes()); // swi 5
    gba.load_rom(&data).unwrap();
    gba.bus.io.ime = 0;
    gba.cpu.step(&mut gba.bus);
    assert_eq!(gba.bus.io.ime, 1, "VBlankIntrWait must force IME=1");
}

#[test]
fn a_halted_cpu_still_sees_hblank() {
    // A halted step used to advance a whole scanline in one go, which walks
    // straight over the HBlank boundary: a game that halts with the HBlank
    // IRQ enabled got one interrupt a frame instead of one per line.
    use geebeeayy_core::Gba;
    let mut gba = Gba::new();
    let mut data = vec![0u8; 0x200];
    data[0..4].copy_from_slice(&0xEAFF_FFFEu32.to_le_bytes()); // b .
    gba.load_rom(&data).unwrap();

    gba.bus.write16(0x0400_0004, 0x0010); // HBlank IRQ enable
    gba.bus.io.ie = 0x0002;
    gba.bus.io.ime = 0;

    let mut hblanks = 0;
    let target = gba.cycles + 280_896; // one frame
    while gba.cycles < target {
        gba.bus.io.halt = true;
        gba.bus.io.if_ = 0;
        gba.step();
        if gba.bus.io.if_ & 0x0002 != 0 {
            hblanks += 1;
        }
    }
    assert!(
        hblanks > 200,
        "expected an HBlank a scanline while halted, got {hblanks}"
    );
}

#[test]
fn vblank_intr_wait_is_not_satisfied_by_an_hblank() {
    // VBlankIntrWait used to test IF, which the BIOS handler has already
    // cleared by the time the game's own handler returns. The first interrupt
    // of any kind - an HBlank, 160 times a frame - therefore released a wait
    // that should have run to line 160, and the game came back mid-frame.
    use geebeeayy_core::Gba;
    let mut gba = Gba::new();
    let mut data = vec![0u8; 0x200];
    data[0..4].copy_from_slice(&0xEF05_0000u32.to_le_bytes()); // swi 5
    data[4..8].copy_from_slice(&0xEAFF_FFFEu32.to_le_bytes()); // b .
    gba.load_rom(&data).unwrap();

    // A game IRQ handler doing exactly what hardware requires of one:
    // acknowledge IF and OR the handled bits into the BIOS IntrWait flags.
    for (i, w) in [
        0xE3A0_0301u32, // mov r0, #0x04000000
        0xE280_0C02,    // add r0, r0, #0x200
        0xE1D0_10B0,    // ldrh r1, [r0]        ; IE
        0xE1D0_20B2,    // ldrh r2, [r0, #2]    ; IF
        0xE001_1002,    // and r1, r1, r2
        0xE1C0_10B2,    // strh r1, [r0, #2]    ; acknowledge
        0xE59F_000C,    // ldr r0, [pc, #12]    ; 0x03007FF8
        0xE1D0_20B0,    // ldrh r2, [r0]
        0xE182_2001,    // orr r2, r2, r1
        0xE1C0_20B0,    // strh r2, [r0]
        0xE12F_FF1E,    // bx lr
        0x0300_7FF8,    // literal
    ]
    .iter()
    .enumerate()
    {
        gba.bus.write32(0x0300_0000 + i as u32 * 4, *w);
    }
    gba.bus.write32(0x0300_7FFC, 0x0300_0000);
    gba.bus.write16(0x0400_0004, 0x0018); // VBlank + HBlank IRQ enable
    gba.bus.io.ie = 0x0003;

    // The SWI sits at 0x08000000 and is re-executed for as long as the wait
    // is unsatisfied, so leaving it means the wait returned.
    let mut woke_at = None;
    for _ in 0..40_000 {
        gba.step();
        let pc = gba.cpu.registers[15];
        if !gba.bus.io.halt && (0x0800_0004..0x0800_0010).contains(&pc) {
            woke_at = Some(gba.bus.read16(0x0400_0006));
            break;
        }
    }
    let line = woke_at.expect("VBlankIntrWait never returned");
    assert!(
        (160..228).contains(&line),
        "VBlankIntrWait returned at line {line}, not in VBlank"
    );
}

#[test]
fn the_bios_handler_leaves_if_for_the_game_to_acknowledge() {
    // The real BIOS IRQ handler saves registers, calls [0x03007FFC] and
    // returns - nothing else. A stub that acknowledged IF first handed every
    // game whose own dispatcher branches on `IE & IF` a zero, and those
    // dispatchers then run whatever their fall-through case is.
    let mut bus = MemoryBus::new();
    // Game handler: r6 = IF as the handler sees it, then bx lr.
    bus.write32(0x0300_1000, 0xE3A0_0301); // mov r0, #0x04000000
    bus.write32(0x0300_1004, 0xE280_0C02); // add r0, r0, #0x200
    bus.write32(0x0300_1008, 0xE1D0_60B2); // ldrh r6, [r0, #2]
    bus.write32(0x0300_100C, 0xE12F_FF1E); // bx lr
    bus.write32(0x0300_7FFC, 0x0300_1000);
    bus.write32(BASE, 0xE320_F000); // nop

    let mut cpu = Cpu::new();
    cpu.boot();
    cpu.registers[15] = BASE;
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
    assert_eq!(
        cpu.registers[6], 0x0001,
        "the game handler must still see its own IF bit"
    );
}

#[test]
fn writing_the_timer_registers_through_the_bus_starts_the_timer() {
    // `Timer::set_control` and `set_reload` had no caller outside this file
    // for the project's whole history: the bus stored TMxCNT into `io_regs`
    // and the timer unit never heard about it. No timer a game started ever
    // ran, so no timer interrupt fired and DMA sound - which is driven
    // entirely by timer overflows - was dead.
    use geebeeayy_core::Gba;
    let mut gba = Gba::new();
    let mut data = vec![0u8; 0x200];
    data[0..4].copy_from_slice(&0xEAFF_FFFEu32.to_le_bytes()); // b .
    gba.load_rom(&data).unwrap();

    gba.bus.write16(0x0400_0100, 0xFF00); // TM0CNT_L: reload
    gba.bus.write16(0x0400_0102, 0x00C0); // TM0CNT_H: enable + IRQ, prescaler 1
    gba.bus.io.ie = 0x0008;

    for _ in 0..200 {
        gba.step();
        if gba.bus.io.if_ & 0x0008 != 0 {
            return;
        }
    }
    panic!("timer 0 never overflowed after its registers were written through the bus");
}

#[test]
fn tm_cnt_l_reads_the_running_counter_not_the_reload() {
    // TMxCNT_L is write-reload, read-counter. Reads used to return whatever
    // reload the game last wrote there, so a game timing anything off a timer
    // read the same value forever.
    use geebeeayy_core::Gba;
    let mut gba = Gba::new();
    let mut data = vec![0u8; 0x200];
    data[0..4].copy_from_slice(&0xEAFF_FFFEu32.to_le_bytes()); // b .
    gba.load_rom(&data).unwrap();

    gba.bus.write16(0x0400_0100, 0x0000);
    gba.bus.write16(0x0400_0102, 0x0080); // enable, prescaler 1
    for _ in 0..50 {
        gba.step();
    }
    assert_ne!(
        gba.bus.read16(0x0400_0100),
        0,
        "TM0CNT_L still reads back the reload the game wrote"
    );
}
