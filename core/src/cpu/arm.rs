use super::Cpu;
use crate::memory::MemoryBus;

/// Execute a single ARM instruction.
///
/// The condition check is already done by the caller (mod.rs).
/// This function dispatches based on bits [27:4] and [7:4].
pub fn execute(instruction: u32, cpu: &mut Cpu, bus: &mut MemoryBus) -> u32 {

    // Software Interrupt.
    //
    // GBATEK, ARM CPU Exceptions: an ARM SWI carries a 24bit comment field
    // while a THUMB SWI carries 8 bits, and the BIOS handler reads the byte at
    // [lr-2] in both cases. For an ARM opcode that byte is bits 23-16, so the
    // function number lives there - "you could use only the most significant
    // 8bits of the 24bit ARM comment". Passing the raw 24bit field instead
    // makes every ARM-mode `swi n` miss its handler.
    if (instruction >> 24) & 0xF == 0xF {
        let comment = (instruction >> 16) & 0xFF;
        cpu.swi(comment, bus);
        return 3;
    }

    // Branch and Exchange (BX) / Branch with Link and Exchange (BLX Rm)
    // bits [27:20]=0001_0010, bits [7:4]=0001 (BX) or 0011 (BLX)
    if (instruction >> 20) & 0xFF == 0x12 && (instruction & 0xF0) == 0x10 {
        return branch_exchange(instruction, cpu);
    }
    if (instruction >> 20) & 0xFF == 0x12 && (instruction & 0xF0) == 0x30 {
        return branch_link_exchange_rm(instruction, cpu);
    }

    // Software Breakpoint (BLX_imm) — bits [27:24]=1001, bits [7:4]=0011
    // Treat as NOP for now

    // Multiply Long / Multiply — bits [27:21] = 000_01xx to 000_00xx, bits [7:4] = 1001
    if (instruction >> 4) & 0xF == 0x9 {
        let opcode_bits = (instruction >> 20) & 0xFF;
        match opcode_bits {
            0b0000_0000..=0b0000_0011 => return multiply(instruction, cpu),
            0b0000_1000..=0b0000_1111 => return multiply_long(instruction, cpu),
            // SWP is 00010 B 00, so bits[27:20] are 0x10 for the word form and
                // 0x14 for the byte form. The old range stopped at 0x13, so
                // every SWPB fell through and was decoded as an MSR.
                0b0001_0000 | 0b0001_0100 => return swap(instruction, cpu, bus),
            _ => {}
        }
    }

    // Halfword / signed Data Transfer — bits [27:25]=000, bit [7]=1, bit [4]=1,
    // SH != 00 (SH == 00 is SWP, handled above).
    // Must be tested before PSR Transfer: a pre-indexed, down-counting STRH has
    // the same bits [24:23]=10 / bit [20]=0 pattern that MSR matches on.
    if (instruction >> 25) & 0x7 == 0b000
        && (instruction >> 7) & 1 == 1
        && (instruction >> 4) & 1 == 1
        && (instruction >> 5) & 0x3 != 0
    {
        return halfword_data_transfer(instruction, cpu, bus);
    }

    // PSR Transfer — bits [27:26]=00, bits [24:23]=10, bit [20]=0
    if (instruction >> 26) & 0x3 == 0b00
        && (instruction >> 23) & 0x3 == 0b10
        && (instruction >> 20) & 1 == 0
    {
        // Bit 21 selects the direction: 0 = MRS (read PSR), 1 = MSR (write PSR).
        if (instruction >> 21) & 1 == 0 {
            return mrs(instruction, cpu);
        } else {
            return msr(instruction, cpu);
        }
    }

    // Data Processing — bits [27:26]=00, and not one of the multiply, swap or
    // halfword encodings, which are the only [27:26]=00 forms with both bit 7
    // and bit 4 set. Those were all matched above.
    //
    // The register-specified shift form (`movs r0, r0, lsl r1`) has bit 4 set
    // and bit 7 clear. An earlier `bit [4] == 0` guard here excluded it, and an
    // immediate-shift-only fallback did not catch it either, so every
    // `<op> rd, rn, rm, lsl rs` in the instruction set silently did nothing.
    // The bit 7 / bit 4 exclusion only applies to the register-operand form.
    // With bit 25 set the operand is an immediate and bits [7:4] are part of
    // its value, so `mov r1, #0xFF00` (0xE3A01CFF) must not be filtered out.
    if (instruction >> 26) & 0x3 == 0b00
        && ((instruction >> 25) & 1 == 1
            || !((instruction >> 4) & 1 == 1 && (instruction >> 7) & 1 == 1))
    {
        return data_processing(instruction, cpu);
    }

    // Single Data Transfer — bits [27:26]=01
    if (instruction >> 26) & 0x3 == 0b01 {
        return single_data_transfer(instruction, cpu, bus);
    }

    // Undefined instruction — bits [27:25]=01, bit [4]=1, bit [7]=1
    if (instruction >> 25) & 0x7 == 0b011 && (instruction >> 4) & 1 == 1 && (instruction >> 7) & 1 == 1 {
        // Undefined instruction - treat as NOP for now
        return 1;
    }

    // Block Data Transfer — bits [27:25]=100
    if (instruction >> 25) & 0x7 == 0b100 {
        return block_data_transfer(instruction, cpu, bus);
    }

    // Branch — bits [27:25]=101
    if (instruction >> 25) & 0x7 == 0b101 {
        return branch(instruction, cpu);
    }

    // Coprocessor data transfer / SWI already handled
    // For unhandled instructions, treat as NOP
    1
}

// ---------------------------------------------------------------------------
// Data Processing
// ---------------------------------------------------------------------------

fn data_processing(instruction: u32, cpu: &mut Cpu) -> u32 {
    let opcode = (instruction >> 21) & 0xF;
    let s_flag = (instruction >> 20) & 1 == 1;
    let rn = ((instruction >> 16) & 0xF) as usize;
    let rd = ((instruction >> 12) & 0xF) as usize;
    let immediate = (instruction >> 25) & 1 == 1;

    // TST, TEQ, CMP and CMN with S set and Rd = R15 are the pre-ARMv4 "P"
    // forms: they discard the comparison and copy SPSR into CPSR. Exception
    // handlers use this to return, so missing it strands the CPU in whatever
    // privileged mode it was in - with that mode's stack pointer.
    if s_flag && rd == 15 && (0b1000..=0b1011).contains(&opcode) {
        let spsr = match cpu.mode() {
            super::Mode::Fiq => cpu.spsr_fiq,
            super::Mode::Irq => cpu.spsr_irq,
            super::Mode::Supervisor => cpu.spsr_svc,
            super::Mode::Abort => cpu.spsr_abt,
            super::Mode::Undefined => cpu.spsr_und,
            // User and System have no SPSR; the operation is unpredictable.
            _ => cpu.cpsr,
        };
        cpu.set_cpsr(spsr);
        return 1;
    }

    // A register-specified shift costs an extra cycle, so every R15 read in
    // that form sees PC + 12 rather than the usual PC + 8.
    let register_specified_shift = !immediate && (instruction >> 4) & 1 == 1;
    let rn_val = if rn == 15 && register_specified_shift {
        cpu.reg(15).wrapping_add(4)
    } else {
        cpu.reg(rn)
    };

    // Shift operand2
    let shifted = cpu.shift_operand2(instruction, immediate);
    let op2 = shifted.value;

    // Execute ALU operation
    let (result, write_result, carry, overflow) = match opcode {
        0b0000 => (rn_val & op2, true, shifted.carry_out, cpu.flag_v()), // AND
        0b0001 => (rn_val ^ op2, true, shifted.carry_out, cpu.flag_v()), // EOR
        0b0010 => {
            let result = rn_val.wrapping_sub(op2);
            let carry = rn_val >= op2;
            (result, true, carry, overflow_sub(rn_val, op2, result))
        }
        0b0011 => {
            let result = op2.wrapping_sub(rn_val);
            let carry = op2 >= rn_val;
            (result, true, carry, overflow_sub(op2, rn_val, result))
        }
        0b0100 => {
            let result = rn_val.wrapping_add(op2);
            let carry = (rn_val as u64) + (op2 as u64) > 0xFFFF_FFFF;
            (result, true, carry, overflow_add(rn_val, op2, result))
        }
        0b0101 => {
            let carry = cpu.flag_c() as u32;
            let result = rn_val.wrapping_add(op2).wrapping_add(carry);
            let carry = (rn_val as u64) + (op2 as u64) + (carry as u64) > 0xFFFF_FFFF;
            (result, true, carry, overflow_add(rn_val, op2, result))
        }
        0b0110 => {
            let carry = cpu.flag_c() as u32;
            let result = rn_val.wrapping_sub(op2).wrapping_sub(1 - carry);
            let carry = (rn_val as u64) >= (op2 as u64) + (1 - carry as u64);
            (result, true, carry, overflow_sub(rn_val, op2, result))
        }
        0b0111 => {
            let carry = cpu.flag_c() as u32;
            let result = op2.wrapping_sub(rn_val).wrapping_sub(1 - carry);
            let carry = (op2 as u64) >= (rn_val as u64) + (1 - carry as u64);
            (result, true, carry, overflow_sub(op2, rn_val, result))
        }
        0b1000 => {
            let result = rn_val & op2;
            cpu.set_flag_nz_data(result, shifted.carry_out);
            return 1;
        }
        0b1001 => {
            let result = rn_val ^ op2;
            cpu.set_flag_nz_data(result, shifted.carry_out);
            return 1;
        }
        0b1010 => {
            let result = rn_val.wrapping_sub(op2);
            let carry = rn_val >= op2;
            cpu.set_flags(
                result >> 31 == 1,
                result == 0,
                carry,
                overflow_sub(rn_val, op2, result),
            );
            return 1;
        }
        0b1011 => {
            let result = rn_val.wrapping_add(op2);
            let carry = (rn_val as u64) + (op2 as u64) > 0xFFFF_FFFF;
            cpu.set_flags(
                result >> 31 == 1,
                result == 0,
                carry,
                overflow_add(rn_val, op2, result),
            );
            return 1;
        }
        0b1100 => (rn_val | op2, true, shifted.carry_out, cpu.flag_v()),
        0b1101 => {
            let result = op2;
            (result, true, shifted.carry_out, cpu.flag_v())
        }
        0b1110 => (rn_val & !op2, true, shifted.carry_out, cpu.flag_v()),
        0b1111 => (!op2, true, shifted.carry_out, cpu.flag_v()),
        _ => unreachable!(),
    };

    if write_result {
        cpu.set_reg(rd, result);
        if s_flag {
            // If Rd is PC, CPSR = SPSR_current
            if rd == 15 {
                let spsr = match cpu.mode() {
                    super::Mode::Fiq => cpu.spsr_fiq,
                    super::Mode::Irq => cpu.spsr_irq,
                    super::Mode::Supervisor => cpu.spsr_svc,
                    super::Mode::Abort => cpu.spsr_abt,
                    super::Mode::Undefined => cpu.spsr_und,
                    _ => cpu.cpsr,
                };
                cpu.set_cpsr(spsr);
            } else {
                cpu.set_flags(result >> 31 == 1, result == 0, carry, overflow);
            }
        }
    }

    1
}

pub fn overflow_add(op1: u32, op2: u32, result: u32) -> bool {
    let sign1 = op1 >> 31;
    let sign2 = op2 >> 31;
    let signr = result >> 31;
    sign1 == sign2 && sign1 != signr
}

pub fn overflow_sub(op1: u32, op2: u32, result: u32) -> bool {
    let sign1 = op1 >> 31;
    let sign2 = op2 >> 31;
    let signr = result >> 31;
    sign1 != sign2 && signr != sign1
}

// ---------------------------------------------------------------------------
// Multiply
// ---------------------------------------------------------------------------

fn multiply(instruction: u32, cpu: &mut Cpu) -> u32 {
    let accumulate = (instruction >> 21) & 1 == 1;
    let set_flags = (instruction >> 20) & 1 == 1;
    let rd = ((instruction >> 16) & 0xF) as usize;
    let rn = ((instruction >> 12) & 0xF) as usize;
    let rs = ((instruction >> 8) & 0xF) as usize;
    let rm = (instruction & 0xF) as usize;

    let rm_val = cpu.reg(rm);
    let rs_val = cpu.reg(rs);
    let result = rm_val.wrapping_mul(rs_val);

    let result = if accumulate {
        result.wrapping_add(cpu.reg(rn))
    } else {
        result
    };

    cpu.set_reg(rd, result);

    if set_flags {
        let n = result >> 31 == 1;
        let z = result == 0;
        cpu.set_flags(n, z, cpu.flag_c(), cpu.flag_v());
    }

    // MUL takes 1S + mI cycles, approximate as 4
    4
}

fn multiply_long(instruction: u32, cpu: &mut Cpu) -> u32 {
    // cond 00001 U A S RdHi RdLo Rs 1001 Rm
    // U = 1 is the *signed* form (SMULL/SMLAL); U = 0 is unsigned.
    let signed = (instruction >> 22) & 1 == 1;
    let accumulate = (instruction >> 21) & 1 == 1;
    let set_flags = (instruction >> 20) & 1 == 1;
    let rd_hi = ((instruction >> 16) & 0xF) as usize;
    let rd_lo = ((instruction >> 12) & 0xF) as usize;
    let rs = ((instruction >> 8) & 0xF) as usize;
    let rm = (instruction & 0xF) as usize;

    let product = if signed {
        let rm_val = cpu.reg(rm) as i32 as i64;
        let rs_val = cpu.reg(rs) as i32 as i64;
        rm_val.wrapping_mul(rs_val) as u64
    } else {
        (cpu.reg(rm) as u64).wrapping_mul(cpu.reg(rs) as u64)
    };

    // The accumulate form adds the existing RdHi:RdLo pair to the product.
    // It used to discard the product and write the pair straight back, which
    // made every UMLAL and SMLAL a no-op.
    let result = if accumulate {
        let existing = ((cpu.reg(rd_hi) as u64) << 32) | cpu.reg(rd_lo) as u64;
        product.wrapping_add(existing)
    } else {
        product
    };

    cpu.set_reg(rd_lo, result as u32);
    cpu.set_reg(rd_hi, (result >> 32) as u32);

    if set_flags {
        let n = (result >> 63) & 1 == 1;
        let z = result == 0;
        cpu.set_flags(n, z, cpu.flag_c(), cpu.flag_v());
    }

    // Long multiply takes 1S + mI, approximate as 5
    5
}

// ---------------------------------------------------------------------------
// Swap (SWP, SWPB)
// ---------------------------------------------------------------------------

fn swap(instruction: u32, cpu: &mut Cpu, bus: &mut MemoryBus) -> u32 {
    let byte_swap = (instruction >> 22) & 1 == 1;
    let rn = ((instruction >> 16) & 0xF) as usize;
    let rd = ((instruction >> 12) & 0xF) as usize;
    let rm = (instruction & 0xF) as usize;

    let addr = cpu.reg(rn);
    let rm_val = cpu.reg(rm);

    if byte_swap {
        let mem_val = bus.read8(addr);
        bus.write8(addr, rm_val as u8);
        cpu.set_reg(rd, mem_val as u32);
    } else {
        // SWP rotates the loaded word the same way LDR does.
        let mem_val = bus.read32_rotated(addr);
        bus.write32(addr & !0x3, rm_val);
        cpu.set_reg(rd, mem_val);
    }

    // SWP takes 1S + 2N + 1I, approximate as 4
    4
}

// ---------------------------------------------------------------------------
// PSR Transfer
// ---------------------------------------------------------------------------

fn mrs(instruction: u32, cpu: &mut Cpu) -> u32 {
    let rd = ((instruction >> 12) & 0xF) as usize;
    let spsr = (instruction >> 22) & 1 == 1;

    let psr_val = if spsr {
        match cpu.mode() {
            super::Mode::Fiq => cpu.spsr_fiq,
            super::Mode::Irq => cpu.spsr_irq,
            super::Mode::Supervisor => cpu.spsr_svc,
            super::Mode::Abort => cpu.spsr_abt,
            super::Mode::Undefined => cpu.spsr_und,
            _ => cpu.cpsr,
        }
    } else {
        cpu.cpsr
    };

    cpu.set_reg(rd, psr_val);
    1
}

fn msr(instruction: u32, cpu: &mut Cpu) -> u32 {
    let spsr = (instruction >> 22) & 1 == 1;
    let immediate = (instruction >> 25) & 1 == 1;

    let value = if immediate {
        let rotate = ((instruction >> 8) & 0xF) * 2;
        let imm8 = instruction & 0xFF;
        (imm8 as u32).rotate_right(rotate)
    } else {
        cpu.reg((instruction & 0xF) as usize)
    };

    // Determine which flags to write (bits 8-24 of MSR mask)
    let mask = {
        let mut m: u32 = 0;
        if instruction & 0x0001_0000 != 0 { m |= 0x0000_00FF; } // Control
        if instruction & 0x0002_0000 != 0 { m |= 0x0000_FF00; } // Extension
        if instruction & 0x0004_0000 != 0 { m |= 0x00FF_0000; } // Status
        if instruction & 0x0008_0000 != 0 { m |= 0xFF00_0000; } // Flags
        m
    };

    let new_psr = (value & mask) | (!mask & if spsr {
        match cpu.mode() {
            super::Mode::Fiq => cpu.spsr_fiq,
            super::Mode::Irq => cpu.spsr_irq,
            super::Mode::Supervisor => cpu.spsr_svc,
            super::Mode::Abort => cpu.spsr_abt,
            super::Mode::Undefined => cpu.spsr_und,
            _ => cpu.cpsr,
        }
    } else {
        cpu.cpsr
    });

    if spsr {
        match cpu.mode() {
            super::Mode::Fiq => cpu.spsr_fiq = new_psr,
            super::Mode::Irq => cpu.spsr_irq = new_psr,
            super::Mode::Supervisor => cpu.spsr_svc = new_psr,
            super::Mode::Abort => cpu.spsr_abt = new_psr,
            super::Mode::Undefined => cpu.spsr_und = new_psr,
            _ => {}
        }
    } else {
        cpu.set_cpsr((cpu.cpsr & !mask) | (new_psr & mask));
    }

    1
}

// ---------------------------------------------------------------------------
// Single Data Transfer (LDR, STR, LDRB, STRB)
// ---------------------------------------------------------------------------

fn single_data_transfer(instruction: u32, cpu: &mut Cpu, bus: &mut MemoryBus) -> u32 {
    // Bit 25 is the I flag: 0 = 12-bit immediate offset, 1 = shifted register.
    // These two were swapped, so every `ldr rd, [rn, #imm]` took Rm as its
    // offset instead. It hid for a long time because the common
    // `ldr/str rd, [rn]` form has Rm == 0 encoded in the low nibble, which
    // makes both operands the same register and sends load and store to the
    // same wrong address - a round-trip test still passes.
    let register_offset = (instruction >> 25) & 1 == 1;
    let up_down = (instruction >> 23) & 1 == 1;
    let byte_transfer = (instruction >> 22) & 1 == 1;
    let write_back = (instruction >> 21) & 1 == 1;
    let load = (instruction >> 20) & 1 == 1;
    let rn = ((instruction >> 16) & 0xF) as usize;
    let rd = ((instruction >> 12) & 0xF) as usize;

    let base = cpu.reg(rn);
    let offset = if register_offset {
        // Immediate-shift form: amount in bits [11:7], type in [6:5], Rm in [3:0].
        let shift_imm = (instruction >> 7) & 0x1F;
        let rm_val = cpu.reg((instruction & 0xF) as usize);
        match (instruction >> 5) & 3 {
            0b00 => cpu.lsl(rm_val, shift_imm).value,
            0b01 => cpu.lsr(rm_val, if shift_imm == 0 { 32 } else { shift_imm }).value,
            0b10 => cpu.asr(rm_val, if shift_imm == 0 { 32 } else { shift_imm }).value,
            0b11 => {
                if shift_imm == 0 {
                    cpu.rrx(rm_val).value
                } else {
                    cpu.ror(rm_val, shift_imm).value
                }
            }
            _ => unreachable!(),
        }
    } else {
        instruction & 0xFFF
    };

    let offset_addr = if up_down {
        base.wrapping_add(offset)
    } else {
        base.wrapping_sub(offset)
    };

    // Bit 24 is P: 1 = pre-indexed (transfer at base +/- offset), 0 = post-indexed
    // (transfer at base, then update it). This was missing entirely, so every
    // post-indexed `ldr rd, [rn], #off` read from the wrong address.
    let pre_index = (instruction >> 24) & 1 == 1;
    let addr = if pre_index { offset_addr } else { base };

    if load {
        if byte_transfer {
            let val = bus.read8(addr);
            cpu.set_reg(rd, val as u32);
        } else {
            let val = bus.read32_rotated(addr);
            cpu.set_reg(rd, val);
        }
    } else {
        let val = if rd == 15 {
            cpu.registers[15].wrapping_add(4)
        } else {
            cpu.reg(rd)
        };
        if byte_transfer {
            bus.write8(addr, val as u8);
        } else {
            bus.write32(addr, val);
        }
    }

    // A post-indexed transfer always writes the new base back; a pre-indexed
    // one only when W is set. Never clobber Rn when it was also the load
    // destination - the loaded value wins.
    if (!pre_index || write_back) && !(load && rd == rn) {
        cpu.set_reg(rn, offset_addr);
    }

    if load { 3 } else { 2 }
}

// ---------------------------------------------------------------------------
// Halfword and Signed Data Transfer (LDRH, STRH, LDRSB, LDRSH)
// ---------------------------------------------------------------------------

fn halfword_data_transfer(instruction: u32, cpu: &mut Cpu, bus: &mut MemoryBus) -> u32 {
    let pre_index = (instruction >> 24) & 1 == 1;
    let up = (instruction >> 23) & 1 == 1;
    let imm_offset = (instruction >> 22) & 1 == 1;
    let write_back = (instruction >> 21) & 1 == 1;
    let load = (instruction >> 20) & 1 == 1;
    let rn = ((instruction >> 16) & 0xF) as usize;
    let rd = ((instruction >> 12) & 0xF) as usize;
    let sh = (instruction >> 5) & 0x3;

    // Immediate offset is split across bits [11:8] and [3:0].
    let offset = if imm_offset {
        ((instruction >> 4) & 0xF0) | (instruction & 0xF)
    } else {
        cpu.reg((instruction & 0xF) as usize)
    };

    let base = cpu.reg(rn);
    let offset_addr = if up {
        base.wrapping_add(offset)
    } else {
        base.wrapping_sub(offset)
    };
    let addr = if pre_index { offset_addr } else { base };

    if load {
        let val = match sh {
            // LDRH from an odd address reads the aligned halfword and rotates
            // the result right by 8, the same way a misaligned LDR rotates.
            0b01 => (bus.read16(addr) as u32).rotate_right((addr & 1) * 8),
            0b10 => bus.read8(addr) as i8 as i32 as u32,
            // LDRSH from an odd address degrades to LDRSB on that byte.
            _ => {
                if addr & 1 != 0 {
                    bus.read8(addr) as i8 as i32 as u32
                } else {
                    bus.read16(addr) as i16 as i32 as u32
                }
            }
        };
        cpu.set_reg(rd, val);
    } else {
        bus.write16(addr, cpu.reg(rd) as u16);
    }

    // Post-indexed transfers always write back, but never clobber Rn when it
    // was also the load destination - the loaded value wins.
    if (!pre_index || write_back) && !(load && rd == rn) {
        cpu.set_reg(rn, offset_addr);
    }

    if load { 3 } else { 2 }
}

// ---------------------------------------------------------------------------
// Block Data Transfer (LDM, STM)
// ---------------------------------------------------------------------------

fn block_data_transfer(instruction: u32, cpu: &mut Cpu, bus: &mut MemoryBus) -> u32 {
    let pre_index = (instruction >> 24) & 1 == 1;
    let write_back = (instruction >> 21) & 1 == 1;
    let load = (instruction >> 20) & 1 == 1;
    let s_bit = (instruction >> 22) & 1 == 1;
    let rn = ((instruction >> 16) & 0xF) as usize;
    let reg_list = instruction & 0xFFFF;

    let up_down = (instruction >> 23) & 1 == 1;
    let base = cpu.reg(rn);

    // An empty register list is not a no-op on ARMv4: R15 is transferred and
    // the base moves by 0x40, as if all sixteen registers had been listed.
    let empty_list = reg_list == 0;
    let reg_count = if empty_list { 16 } else { reg_list.count_ones() };

    // Registers always move in increasing address order, lowest register at the
    // lowest address; P and U only choose where the block starts.
    //   IA (P=0,U=1) base            IB (P=1,U=1) base + 4
    //   DA (P=0,U=0) base - n*4 + 4  DB (P=1,U=0) base - n*4
    // IB and DA were both off by one slot before.
    let mut addr = match (up_down, pre_index) {
        (true, false) => base,
        (true, true) => base.wrapping_add(4),
        (false, false) => base.wrapping_sub(reg_count * 4).wrapping_add(4),
        (false, true) => base.wrapping_sub(reg_count * 4),
    };

    addr &= !3;

    // S with R15 absent selects the User bank. An empty list transfers R15,
    // so it does not qualify.
    let user_bank = s_bit && reg_list & (1 << 15) == 0 && !empty_list;

    if empty_list {
        if load {
            let val = bus.read32(addr);
            cpu.set_reg(15, val);
        } else {
            bus.write32(addr, cpu.registers[15].wrapping_add(4));
        }
        if write_back {
            let new_base = if up_down {
                base.wrapping_add(0x40)
            } else {
                base.wrapping_sub(0x40)
            };
            cpu.set_reg(rn, new_base);
        }
        return if load { 2 + reg_count } else { 1 + reg_count };
    }

    if load {
        for i in 0..16u32 {
            if reg_list & (1 << i) != 0 {
                let val = bus.read32(addr);
                if user_bank {
                    cpu.set_user_reg(i as usize, val);
                } else {
                    cpu.set_reg(i as usize, val);
                }
                addr = addr.wrapping_add(4);
            }
        }
        if s_bit && reg_list & (1 << 15) != 0 {
            let spsr = match cpu.mode() {
                super::Mode::Fiq => cpu.spsr_fiq,
                super::Mode::Irq => cpu.spsr_irq,
                super::Mode::Supervisor => cpu.spsr_svc,
                super::Mode::Abort => cpu.spsr_abt,
                super::Mode::Undefined => cpu.spsr_und,
                _ => cpu.cpsr,
            };
            cpu.set_cpsr(spsr);
        }
    } else {
        for i in 0..16u32 {
            if reg_list & (1 << i) != 0 {
                // STM with the base in the list stores the ORIGINAL base only
                // when it is the lowest register present; otherwise the
                // written-back value is what lands in memory.
                let val = if i as usize == rn {
                    if reg_list.trailing_zeros() == rn as u32 {
                        base
                    } else if up_down {
                        base.wrapping_add(reg_count * 4)
                    } else {
                        base.wrapping_sub(reg_count * 4)
                    }
                } else if i == 15 {
                    cpu.registers[15].wrapping_add(4)
                } else if user_bank {
                    cpu.user_reg(i as usize)
                } else {
                    cpu.reg(i as usize)
                };
                bus.write32(addr, val);
                addr = addr.wrapping_add(4);
            }
        }
    }

    // Same rule for LDM: if the base register was in the transfer list, the
    // value loaded into it wins over the writeback.
    let base_in_list = reg_list & (1 << rn as u32) != 0;
    if write_back && !(load && base_in_list) {
        let new_base = if up_down {
            base.wrapping_add(reg_count * 4)
        } else {
            base.wrapping_sub(reg_count * 4)
        };
        cpu.set_reg(rn, new_base);
    }

    if load { 2 + reg_count } else { 1 + reg_count }
}

// ---------------------------------------------------------------------------
// Branch (B, BL)
// ---------------------------------------------------------------------------

fn branch(instruction: u32, cpu: &mut Cpu) -> u32 {
    let link = (instruction >> 24) & 1 == 1;
    let pc = cpu.registers[15];

    // Sign-extend 24-bit offset to 32 bits, shift left 2
    let offset = ((instruction & 0x00FF_FFFF) << 2) as i32;
    let offset = if offset & 0x0200_0000 != 0 {
        (offset as i32) | 0xFC00_0000u32 as i32
    } else {
        offset
    };

    if link {
        // BL: save return address in LR
        let return_addr = pc.wrapping_sub(4);
        cpu.set_reg(14, return_addr);
    }

    let target = pc.wrapping_add(offset as u32);
    cpu.set_reg(15, target);

    3
}

// ---------------------------------------------------------------------------
// Branch and Exchange (BX)
// ---------------------------------------------------------------------------

fn branch_exchange(instruction: u32, cpu: &mut Cpu) -> u32 {
    let rm = (instruction & 0xF) as usize;
    let addr = cpu.reg(rm);

    // Bit 0 determines ARM/THUMB
    if addr & 1 == 1 {
        cpu.cpsr |= 0x20; // Set THUMB bit
        cpu.set_reg(15, addr & !1);
    } else {
        cpu.cpsr &= !0x20; // Clear THUMB bit
        cpu.set_reg(15, addr & !3);
    }

    3
}

fn branch_link_exchange_rm(instruction: u32, cpu: &mut Cpu) -> u32 {
    let rm = (instruction & 0xF) as usize;
    let addr = cpu.reg(rm);
    let next_pc = cpu.reg(15).wrapping_sub(4);
    cpu.registers[14] = next_pc; // LR = return address

    if addr & 1 == 1 {
        cpu.cpsr |= 0x20;
        cpu.set_reg(15, addr & !1);
    } else {
        cpu.cpsr &= !0x20;
        cpu.set_reg(15, addr & !3);
    }

    3
}
