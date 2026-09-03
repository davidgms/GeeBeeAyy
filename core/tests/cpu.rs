//! ARM7TDMI regression tests. Instructions are placed in IWRAM and stepped
//! directly, so no ROM or BIOS image is needed.

use geebeeayy_core::cpu::Cpu;
use geebeeayy_core::memory::MemoryBus;

const BASE: u32 = 0x0300_0000;

/// Load ARM words at BASE and point the CPU at them.
fn setup_arm(instrs: &[u32]) -> (Cpu, MemoryBus) {
    let mut bus = MemoryBus::new();
    for (i, &word) in instrs.iter().enumerate() {
        bus.write32(BASE + (i as u32) * 4, word);
    }
    let mut cpu = Cpu::new();
    cpu.registers[15] = BASE;
    cpu.registers[13] = 0x0300_7F00;
    (cpu, bus)
}

/// Load THUMB halfwords at BASE and point the CPU at them in THUMB state.
fn setup_thumb(instrs: &[u16]) -> (Cpu, MemoryBus) {
    let mut bus = MemoryBus::new();
    for (i, &half) in instrs.iter().enumerate() {
        bus.write16(BASE + (i as u32) * 2, half);
    }
    let mut cpu = Cpu::new();
    cpu.registers[15] = BASE;
    cpu.registers[13] = 0x0300_7F00;
    cpu.cpsr |= 0x20; // T bit
    (cpu, bus)
}

fn steps(cpu: &mut Cpu, bus: &mut MemoryBus, n: usize) {
    for _ in 0..n {
        cpu.step(bus);
    }
}

#[test]
fn arm_executes_consecutive_instructions() {
    // mov r0, #1 ; mov r1, #2 ; mov r2, #3
    let (mut cpu, mut bus) = setup_arm(&[0xE3A0_0001, 0xE3A0_1002, 0xE3A0_2003]);
    steps(&mut cpu, &mut bus, 3);
    assert_eq!(cpu.registers[0], 1);
    assert_eq!(cpu.registers[1], 2, "second instruction was skipped");
    assert_eq!(cpu.registers[2], 3, "third instruction was skipped");
}

#[test]
fn thumb_executes_consecutive_instructions() {
    // mov r0, #1 ; mov r1, #2 ; mov r2, #3
    let (mut cpu, mut bus) = setup_thumb(&[0x2001, 0x2102, 0x2203]);
    steps(&mut cpu, &mut bus, 3);
    assert_eq!(cpu.registers[0], 1);
    assert_eq!(cpu.registers[1], 2, "second instruction was skipped");
    assert_eq!(cpu.registers[2], 3, "third instruction was skipped");
}

#[test]
fn arm_r15_reads_as_pc_plus_8() {
    // mov r0, pc  -> r0 must be BASE + 8
    let (mut cpu, mut bus) = setup_arm(&[0xE1A0_000F]);
    cpu.step(&mut bus);
    assert_eq!(cpu.registers[0], BASE + 8);
}

#[test]
fn arm_branch_and_link() {
    // b +2 instructions ; (skipped) ; mov r0, #7
    // B offset is relative to PC (instr + 8), so offset 0 lands on instr+8.
    let (mut cpu, mut bus) = setup_arm(&[0xEA00_0000, 0xE3A0_00FF, 0xE3A0_0007]);
    steps(&mut cpu, &mut bus, 2);
    assert_eq!(
        cpu.registers[0], 7,
        "branch did not land on instruction at +8"
    );
}

#[test]
fn arm_adds_sets_carry_and_overflow() {
    // mov r0, #0x80000000 (0x2 ror 2) ; adds r0, r0, r0
    let (mut cpu, mut bus) = setup_arm(&[0xE3A0_0102, 0xE090_0000]);
    steps(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.registers[0], 0);
    assert!(cpu.flag_z(), "Z must be set");
    assert!(cpu.flag_c(), "C must be set on unsigned overflow");
    assert!(cpu.flag_v(), "V must be set on signed overflow");
}

#[test]
fn arm_subs_borrow_clears_carry() {
    // mov r0, #1 ; subs r0, r0, #2
    let (mut cpu, mut bus) = setup_arm(&[0xE3A0_0001, 0xE250_0002]);
    steps(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.registers[0], 0xFFFF_FFFF);
    assert!(cpu.flag_n());
    assert!(!cpu.flag_c(), "C is clear when a subtraction borrows");
}

#[test]
fn arm_condition_codes_gate_execution() {
    // mov r0, #0 ; cmp r0, #0 ; movne r1, #1 ; moveq r2, #1
    let (mut cpu, mut bus) = setup_arm(&[0xE3A0_0000, 0xE350_0000, 0x13A0_1001, 0x03A0_2001]);
    steps(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.registers[1], 0, "NE must not execute when Z is set");
    assert_eq!(cpu.registers[2], 1, "EQ must execute when Z is set");
}

#[test]
fn arm_stm_ldm_round_trip() {
    // mov r0, #0x02000000 ; mov r1, #0x11 ; mov r2, #0x22
    // stmia r0, {r1, r2} ; mov r1, #0 ; mov r2, #0 ; ldmia r0, {r1, r2}
    let (mut cpu, mut bus) = setup_arm(&[
        0xE3A0_0402, // mov r0, #0x02000000
        0xE3A0_1011, // mov r1, #0x11
        0xE3A0_2022, // mov r2, #0x22
        0xE880_0006, // stmia r0, {r1, r2}
        0xE3A0_1000, // mov r1, #0
        0xE3A0_2000, // mov r2, #0
        0xE890_0006, // ldmia r0, {r1, r2}
    ]);
    steps(&mut cpu, &mut bus, 7);
    assert_eq!(cpu.registers[1], 0x11);
    assert_eq!(cpu.registers[2], 0x22);
}

#[test]
fn arm_ldr_str_word() {
    // mov r0, #0x02000000 ; mov r1, #0xAB ; str r1, [r0] ; ldr r2, [r0]
    let (mut cpu, mut bus) = setup_arm(&[0xE3A0_0402, 0xE3A0_10AB, 0xE580_1000, 0xE590_2000]);
    steps(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.registers[2], 0xAB);
    // Assert the address too. A store and a load that agree on the *wrong*
    // address still round-trip, which is how an inverted offset-mode flag
    // survived here undetected.
    assert_eq!(
        bus.read32(0x0200_0000),
        0xAB,
        "the store went somewhere else"
    );
}

#[test]
fn arm_ldr_str_immediate_offset_is_not_a_register() {
    // Bit 25 clear means the offset is the literal 12-bit field, not Rm.
    // mov r0, #0x02000000 ; mov r4, #0xF0 ; mov r1, #0x5A
    // str r1, [r0, #4]    ; ldr r2, [r0, #4]
    // The offset field is 0x004, and R4 holds 0xF0, so a decoder that reads
    // the low nibble as Rm lands 0xF0 bytes away instead of 4.
    let (mut cpu, mut bus) = setup_arm(&[
        0xE3A0_0402, // mov r0, #0x02000000
        0xE3A0_40F0, // mov r4, #0xF0
        0xE3A0_105A, // mov r1, #0x5A
        0xE580_1004, // str r1, [r0, #4]
        0xE590_2004, // ldr r2, [r0, #4]
    ]);
    steps(&mut cpu, &mut bus, 5);
    assert_eq!(bus.read32(0x0200_0004), 0x5A, "immediate offset was not 4");
    assert_eq!(bus.read32(0x0200_00F0), 0, "R4 was used as the offset");
    assert_eq!(cpu.registers[2], 0x5A);
}

#[test]
fn arm_ldr_register_offset_still_works() {
    // Bit 25 set means Rm is the offset, shifted by the [11:7] field.
    // mov r0, #0x02000000 ; mov r3, #2 ; mov r1, #0x77
    // str r1, [r0, r3, lsl #2]  -> 0x02000008
    let (mut cpu, mut bus) = setup_arm(&[
        0xE3A0_0402, // mov r0, #0x02000000
        0xE3A0_3002, // mov r3, #2
        0xE3A0_1077, // mov r1, #0x77
        0xE780_1103, // str r1, [r0, r3, lsl #2]
    ]);
    steps(&mut cpu, &mut bus, 4);
    assert_eq!(
        bus.read32(0x0200_0008),
        0x77,
        "register offset or its shift is wrong"
    );
}

#[test]
fn thumb_bl_returns_to_next_instruction() {
    // bl +4 ; mov r0, #1 ; (target) mov r1, #1 ; bx lr
    let (mut cpu, mut bus) = setup_thumb(&[0xF000, 0xF802, 0x2001, 0x2101, 0x4770]);
    steps(&mut cpu, &mut bus, 2); // BL is two halfwords, one logical call
    assert_eq!(
        cpu.registers[14] & !1,
        BASE + 4,
        "LR must point past the BL pair"
    );
}

#[test]
fn barrel_shifter_edge_cases() {
    let mut cpu = Cpu::new();
    cpu.set_flags(false, false, false, false);

    let r = cpu.lsl(0x8000_0000, 1);
    assert_eq!(r.value, 0);
    assert!(r.carry_out, "LSL #1 of 0x80000000 carries out the top bit");

    let r = cpu.lsr(1, 1);
    assert_eq!(r.value, 0);
    assert!(r.carry_out);

    let r = cpu.asr(0x8000_0000, 31);
    assert_eq!(r.value, 0xFFFF_FFFF, "ASR keeps the sign bit");

    let r = cpu.asr(0x8000_0000, 32);
    assert_eq!(r.value, 0xFFFF_FFFF);
    assert!(r.carry_out);

    let r = cpu.ror(1, 1);
    assert_eq!(r.value, 0x8000_0000);
    assert!(r.carry_out);

    cpu.set_flags(false, false, true, false); // C = 1
    let r = cpu.rrx(0);
    assert_eq!(r.value, 0x8000_0000, "RRX shifts the old carry into bit 31");
    assert!(!r.carry_out);
}

#[test]
fn irq_entry_switches_to_arm_state() {
    let mut cpu = Cpu::new();
    cpu.cpsr |= 0x20; // running in THUMB
    cpu.registers[15] = 0x0800_1234;
    cpu.handle_irq();
    assert_eq!(cpu.registers[15], 0x0000_0018, "IRQ vector");
    assert_eq!(
        cpu.cpsr & 0x20,
        0,
        "the IRQ vector is ARM code, T must be cleared"
    );
    assert_ne!(cpu.cpsr & 0x80, 0, "IRQs must be masked on entry");
}

// --- THUMB formats that the 3-bit dispatch used to drop on the floor --------

#[test]
fn thumb_alu_and() {
    // mov r0, #0xF ; mov r1, #3 ; and r0, r1
    let (mut cpu, mut bus) = setup_thumb(&[0x200F, 0x2103, 0x4008]);
    steps(&mut cpu, &mut bus, 3);
    assert_eq!(cpu.registers[0], 3);
}

#[test]
fn thumb_bx_switches_to_arm() {
    // mov r1, #0x40 ; bx r1   (bit 0 clear -> ARM state)
    let (mut cpu, mut bus) = setup_thumb(&[0x2140, 0x4708]);
    steps(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.registers[15], 0x40);
    assert_eq!(
        cpu.cpsr & 0x20,
        0,
        "BX to an even address must clear the T bit"
    );
}

#[test]
fn thumb_pc_relative_load() {
    // ldr r0, [pc, #0] ; <pad> ; .word 0xCAFEBABE
    let (mut cpu, mut bus) = setup_thumb(&[0x4800, 0x0000]);
    bus.write32(BASE + 4, 0xCAFE_BABE);
    cpu.step(&mut bus);
    assert_eq!(cpu.registers[0], 0xCAFE_BABE);
}

#[test]
fn thumb_register_offset_load_store() {
    // r0 = 0x02000000 base, r1 = 0 offset
    let (mut cpu, mut bus) = setup_thumb(&[
        0x2002, // mov r0, #2
        0x0600, // lsl r0, r0, #24 -> 0x02000000
        0x2100, // mov r1, #0
        0x22AB, // mov r2, #0xAB
        0x5042, // str  r2, [r0, r1]
        0x5843, // ldr  r3, [r0, r1]
        0x5442, // strb r2, [r0, r1]
        0x5C44, // ldrb r4, [r0, r1]
        0x5242, // strh r2, [r0, r1]
        0x5A45, // ldrh r5, [r0, r1]
    ]);
    steps(&mut cpu, &mut bus, 10);
    assert_eq!(cpu.registers[3], 0xAB, "LDR register offset");
    assert_eq!(cpu.registers[4], 0xAB, "LDRB register offset");
    assert_eq!(cpu.registers[5], 0xAB, "LDRH register offset");
}

#[test]
fn thumb_halfword_immediate_is_not_sp_relative() {
    // The two used to share a dispatch arm, so SP-relative accesses were
    // decoded as halfword accesses against r0.
    let (mut cpu, mut bus) = setup_thumb(&[
        0x2002, // mov r0, #2
        0x0600, // lsl r0, r0, #24 -> 0x02000000
        0x21FF, // mov r1, #0xFF
        0x8001, // strh r1, [r0, #0]
        0x8802, // ldrh r2, [r0, #0]
        0x9100, // str  r1, [sp, #0]
        0x9B00, // ldr  r3, [sp, #0]
    ]);
    steps(&mut cpu, &mut bus, 7);
    assert_eq!(cpu.registers[2], 0xFF, "halfword immediate load");
    assert_eq!(cpu.registers[3], 0xFF, "SP-relative load");
    assert_eq!(
        bus.read32(cpu.registers[13]),
        0xFF,
        "SP-relative store hit the stack"
    );
}

#[test]
fn thumb_push_pop() {
    // mov r0, #0x11 ; mov r1, #0x22 ; push {r0,r1} ; mov r0,#0 ; mov r1,#0 ; pop {r0,r1}
    let (mut cpu, mut bus) = setup_thumb(&[0x2011, 0x2122, 0xB403, 0x2000, 0x2100, 0xBC03]);
    let sp_before = cpu.registers[13];
    steps(&mut cpu, &mut bus, 6);
    assert_eq!(cpu.registers[0], 0x11);
    assert_eq!(cpu.registers[1], 0x22);
    assert_eq!(
        cpu.registers[13], sp_before,
        "PUSH/POP must balance the stack"
    );
}

#[test]
fn thumb_stmia_ldmia() {
    let (mut cpu, mut bus) = setup_thumb(&[
        0x2002, // mov r0, #2
        0x0600, // lsl r0, r0, #24 -> 0x02000000
        0x2111, // mov r1, #0x11
        0x2222, // mov r2, #0x22
        0xC006, // stmia r0!, {r1, r2}
        0x2002, // mov r0, #2
        0x0600, // lsl r0, r0, #24 (reset base)
        0xC818, // ldmia r0!, {r3, r4}
    ]);
    steps(&mut cpu, &mut bus, 8);
    assert_eq!(cpu.registers[3], 0x11);
    assert_eq!(cpu.registers[4], 0x22);
}

#[test]
fn thumb_conditional_branch() {
    // mov r0, #0 ; cmp r0, #0 ; beq +0 ; mov r1, #1 (skipped) ; mov r2, #1
    let (mut cpu, mut bus) = setup_thumb(&[0x2000, 0x2800, 0xD000, 0x2101, 0x2201]);
    steps(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.registers[1], 0, "BEQ should have skipped this");
    assert_eq!(cpu.registers[2], 1, "BEQ landed on the wrong instruction");
}

#[test]
fn thumb_add_sp_and_load_address() {
    // add r0, sp, #0 ; add sp, #4
    let (mut cpu, mut bus) = setup_thumb(&[0xA800, 0xB001]);
    let sp = cpu.registers[13];
    steps(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.registers[0], sp, "ADD Rd, SP, #imm");
    assert_eq!(cpu.registers[13], sp + 4, "ADD SP, #imm");
}

#[test]
fn thumb_backward_bl_reaches_a_lower_address() {
    // The first half of BL sign-extends its offset; without that, backward
    // calls land far above the caller instead of below it.
    let (mut cpu, mut bus) = setup_thumb(&[0xF7FF, 0xFFFC]); // bl -8
    steps(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.registers[15], BASE.wrapping_sub(4));
}

#[test]
fn arm_halfword_and_signed_transfers() {
    // Build 0x02000000 in r0, store 0xFF80, then read it back four ways.
    let (mut cpu, mut bus) = setup_arm(&[
        0xE3A0_0402, // mov   r0, #0x02000000
        0xE3A0_1CFF, // mov   r1, #0xFF00
        0xE281_1080, // add   r1, r1, #0x80      -> 0xFF80
        0xE1C0_10B0, // strh  r1, [r0]
        0xE1D0_20B0, // ldrh  r2, [r0]
        0xE1D0_30D0, // ldrsb r3, [r0]
        0xE1D0_40F0, // ldrsh r4, [r0]
    ]);
    steps(&mut cpu, &mut bus, 7);
    assert_eq!(cpu.registers[2], 0xFF80, "LDRH is zero-extended");
    assert_eq!(
        cpu.registers[3], 0xFFFF_FF80,
        "LDRSB sign-extends the low byte"
    );
    assert_eq!(
        cpu.registers[4], 0xFFFF_FF80,
        "LDRSH sign-extends the halfword"
    );
}

#[test]
fn arm_halfword_post_index_writes_back() {
    // mov r0, #0x02000000 ; mov r1, #1 ; strh r1, [r0], #2
    let (mut cpu, mut bus) = setup_arm(&[0xE3A0_0402, 0xE3A0_1001, 0xE0C0_10B2]);
    steps(&mut cpu, &mut bus, 3);
    assert_eq!(bus.read16(0x0200_0000), 1);
    assert_eq!(
        cpu.registers[0], 0x0200_0002,
        "post-indexed transfer must write back"
    );
}

#[test]
fn arm_msr_still_decodes_after_the_halfword_arm() {
    // msr cpsr_f, #0xF0000000 must not be swallowed by the halfword decoder.
    let (mut cpu, mut bus) = setup_arm(&[0xE328_F20F]);
    cpu.step(&mut bus);
    assert!(cpu.flag_n() && cpu.flag_z() && cpu.flag_c() && cpu.flag_v());
}

#[test]
fn arm_register_specified_lsl_at_and_past_32() {
    // gba-suite arm tests 152/153. Only the low byte of Rs is used.
    // LSL #32 -> result 0, carry = bit 0 of Rm. LSL #33+ -> result 0, carry 0.
    // movs r0, r0, lsl r1  ==  lsls r0, r1
    let by_32 = &[0xE3A0_0001, 0xE3A0_1020, 0xE1B0_0110]; // r0=1, r1=32
    let (mut cpu, mut bus) = setup_arm(by_32);
    steps(&mut cpu, &mut bus, 3);
    assert_eq!(cpu.registers[0], 0, "LSL by 32 must clear the register");
    assert!(cpu.flag_z());
    assert!(cpu.flag_c(), "LSL by 32 carries out bit 0 of Rm");

    let by_33 = &[0xE3A0_0001, 0xE3A0_1021, 0xE1B0_0110]; // r0=1, r1=33
    let (mut cpu, mut bus) = setup_arm(by_33);
    steps(&mut cpu, &mut bus, 3);
    assert_eq!(cpu.registers[0], 0, "LSL past 32 must clear the register");
    assert!(cpu.flag_z());
    assert!(!cpu.flag_c(), "LSL past 32 must clear the carry");
}

#[test]
fn arm_test_opcode_with_rd_r15_restores_cpsr_from_spsr() {
    // gba-suite arm test 234. `cmp pc, pc, r0` (0xE15FF000) has S set and
    // Rd = R15. CMP writes no result, so the ARM7TDMI copies SPSR into CPSR
    // instead - the way exception handlers return.
    use geebeeayy_core::cpu::Mode;
    let (mut cpu, mut bus) = setup_arm(&[0xE15F_F000]);
    cpu.set_cpsr((cpu.cpsr & !0x1F) | Mode::Fiq as u32);
    cpu.spsr_fiq = (cpu.cpsr & !0x1F) | Mode::System as u32;
    cpu.registers[15] = BASE;

    cpu.step(&mut bus);

    assert_eq!(
        cpu.mode(),
        Mode::System,
        "S + Rd=R15 on a test opcode must load CPSR from SPSR"
    );
}

#[test]
fn arm_long_multiplies() {
    // gba-suite arm test 306 and neighbours. Bit 22 selects signed (SMULL),
    // not unsigned, and the accumulate forms add to RdHi:RdLo.
    // umull r2, r3, r0, r1 with both operands 0xFFFFFFFF -> 0xFFFFFFFE_00000001
    let (mut cpu, mut bus) = setup_arm(&[
        0xE3E0_0000, // mvn r0, #0   -> 0xFFFFFFFF
        0xE3E0_1000, // mvn r1, #0
        0xE083_2190, // umull r2, r3, r0, r1
    ]);
    steps(&mut cpu, &mut bus, 3);
    assert_eq!(cpu.registers[2], 0x0000_0001, "UMULL low word");
    assert_eq!(cpu.registers[3], 0xFFFF_FFFE, "UMULL high word");

    // smull r2, r3, r0, r1 with -1 * -1 = 1
    let (mut cpu, mut bus) = setup_arm(&[
        0xE3E0_0000, // mvn r0, #0
        0xE3E0_1000, // mvn r1, #0
        0xE0C3_2190, // smull r2, r3, r0, r1
    ]);
    steps(&mut cpu, &mut bus, 3);
    assert_eq!(cpu.registers[2], 1, "SMULL low word");
    assert_eq!(cpu.registers[3], 0, "SMULL high word");

    // umlal must add to the existing pair, not replace it.
    let (mut cpu, mut bus) = setup_arm(&[
        0xE3A0_0002, // mov r0, #2
        0xE3A0_1003, // mov r1, #3
        0xE3A0_2005, // mov r2, #5
        0xE3A0_3000, // mov r3, #0
        0xE0A3_2190, // umlal r2, r3, r0, r1
    ]);
    steps(&mut cpu, &mut bus, 5);
    assert_eq!(cpu.registers[2], 11, "UMLAL must accumulate: 2*3 + 5");
    assert_eq!(cpu.registers[3], 0);
}

#[test]
fn arm_pre_and_post_indexed_transfers() {
    // gba-suite arm test 353. Post-index transfers at the base then updates it;
    // pre-index with writeback transfers at base+offset and keeps it.
    let (mut cpu, mut bus) = setup_arm(&[
        0xE3A0_0020, // mov r0, #32
        0xE3A0_1001, // mov r1, #1
        0xE3A0_2402, // mov r2, #0x02000000
        0xE482_0004, // str r0, [r2], #4          -> stores at 0x02000000, r2 += 4
        0xE732_3101, // ldr r3, [r2, -r1, lsl #2]! -> loads 0x02000000, r2 -= 4
    ]);
    steps(&mut cpu, &mut bus, 5);
    assert_eq!(
        bus.read32(0x0200_0000),
        32,
        "post-indexed store used base+offset"
    );
    assert_eq!(
        cpu.registers[3], 32,
        "pre-indexed load read the wrong address"
    );
    assert_eq!(cpu.registers[2], 0x0200_0000, "writeback did not land");
}

#[test]
fn arm_misaligned_word_load_rotates() {
    // gba-suite arm test 355. The bus fetches the aligned word; the CPU rotates
    // it right by the byte offset. Reading the unaligned bytes gives a
    // different, wrong answer.
    let (mut cpu, mut bus) = setup_arm(&[
        0xE3A0_0020, // mov r0, #32
        0xE3A0_2402, // mov r2, #0x02000000
        0xE582_0000, // str r0, [r2]
        0xE592_1003, // ldr r1, [r2, #3]
    ]);
    steps(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.registers[1], 32u32.rotate_right(24));
}

#[test]
fn arm_data_processing_without_s_preserves_flags() {
    // gba-suite arm test 363 leans on this: a `mov`/`orr`/`bic`/`mvn` between a
    // comparison and its branch must not disturb the condition flags.
    let (mut cpu, mut bus) = setup_arm(&[
        0xE3A0_0000, // mov r0, #0
        0xE350_0000, // cmp r0, #0        -> Z set
        0xE3A0_10FF, // mov r1, #0xFF
        0xE381_20F0, // orr r2, r1, #0xF0
        0xE3C2_3001, // bic r3, r2, #1
        0xE3E0_4000, // mvn r4, #0
        0x03A0_5001, // moveq r5, #1      -> must still execute
    ]);
    steps(&mut cpu, &mut bus, 7);
    assert!(cpu.flag_z(), "a non-S data processing op cleared Z");
    assert_eq!(cpu.registers[5], 1, "the EQ condition was destroyed");
}

#[test]
fn thumb_add_and_sub_immediate_update_the_register() {
    // Format 3: 001 op Rd imm8. Yggdra Union's startup scans a table with
    // `add r5,#12` / `sub r4,#1` and loops while r4 != 0 - if these do not
    // write back, the game never leaves its first loop.
    let (mut cpu, mut bus) = setup_thumb(&[
        0x2500, // mov r5, #0
        0x350C, // add r5, #12
        0x350C, // add r5, #12
        0x2405, // mov r4, #5
        0x3C01, // sub r4, #1
        0x3C01, // sub r4, #1
    ]);
    steps(&mut cpu, &mut bus, 6);
    assert_eq!(cpu.registers[5], 24, "add r5,#12 twice should give 24");
    assert_eq!(cpu.registers[4], 3, "sub r4,#1 twice from 5 should give 3");
}

#[test]
fn thumb_backward_unconditional_branch() {
    // Format 18's 11-bit offset is signed. Masked to 11 bits and widened as if
    // it were positive, a backward `b` lands 0x1000 above the branch instead
    // of below it - which is exactly how Yggdra Union's division routine fell
    // out of its own function and into the C++ throw path.
    //
    //   0: mov r0, #0    2000
    //   2: b   +2        E001  forward, over the traps, to 8
    //   4: mov r0, #0x77 2077  trap
    //   6: mov r2, #9    2209  the backward branch's target
    //   8: mov r1, #5    2105
    //   A: b   -8        E7FC  back to 6
    let (mut cpu, mut bus) = setup_thumb(&[0x2000, 0xE001, 0x2077, 0x2209, 0x2105, 0xE7FC]);
    steps(&mut cpu, &mut bus, 3); // mov r0,#0 ; b +2 ; mov r1,#5
    assert_eq!(
        cpu.registers[0], 0,
        "forward B should have skipped the trap"
    );
    assert_eq!(
        cpu.registers[1], 5,
        "forward B landed on the wrong halfword"
    );
    cpu.step(&mut bus); // b -8
    assert_eq!(
        cpu.registers[15],
        BASE + 6,
        "backward B must sign-extend its 11-bit offset"
    );
    cpu.step(&mut bus);
    assert_eq!(cpu.registers[2], 9, "backward B did not execute its target");
}
