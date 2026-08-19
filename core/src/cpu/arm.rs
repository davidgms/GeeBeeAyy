use super::Cpu;
use crate::memory::MemoryBus;

/// Execute a single ARM instruction.
///
/// The condition check is already done by the caller (mod.rs).
/// This function dispatches based on bits [27:4] and [7:4].
pub fn execute(instruction: u32, cpu: &mut Cpu, bus: &mut MemoryBus) -> u32 {

    // Software Interrupt
    if (instruction >> 24) & 0xF == 0xF {
        let comment = instruction & 0x00FF_FFFF;
        cpu.swi(comment);
        return 3;
    }

    // Branch and Exchange (BX) — bits [27:20]=0001_0010, bits [7:4]=0001
    if (instruction >> 24) & 0xFF == 0x12 && (instruction & 0xF0) == 0x10 {
        return branch_exchange(instruction, cpu);
    }

    // Software Breakpoint (BLX_imm) — bits [27:24]=1001, bits [7:4]=0011
    // Treat as NOP for now

    // Multiply Long / Multiply — bits [27:21] = 000_01xx to 000_00xx, bits [7:4] = 1001
    if (instruction >> 4) & 0xF == 0x9 {
        let opcode_bits = (instruction >> 20) & 0xFF;
        match opcode_bits {
            0b0000_0000..=0b0000_0011 => return multiply(instruction, cpu),
            0b0000_1000..=0b0000_1011 => return multiply_long(instruction, cpu),
            0b0001_0000..=0b0001_0011 => return swap(instruction, cpu, bus),
            _ => {}
        }
    }

    // PSR Transfer — bits [27:26]=00, bits [24:23]=10, bit [20]=0
    if (instruction >> 26) & 0x3 == 0b00
        && (instruction >> 23) & 0x3 == 0b10
        && (instruction >> 20) & 1 == 0
    {
        let bit4 = (instruction >> 4) & 1;
        if bit4 == 0 {
            return mrs(instruction, cpu);
        } else {
            return msr(instruction, cpu);
        }
    }

    // Data Processing / Register Shift — bits [27:26]=00, bit [4]=0
    if (instruction >> 26) & 0x3 == 0b00 && (instruction >> 4) & 1 == 0 {
        return data_processing(instruction, cpu);
    }

    // Data Processing / Immediate Shift — bits [27:26]=00, bit [25]=1
    if (instruction >> 26) & 0x3 == 0b00 && (instruction >> 25) & 1 == 1 {
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

    let rn_val = cpu.reg(rn);

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
        0b1100 => {
            let result = rn_val | op2;
            cpu.set_flag_nz_data(result, shifted.carry_out);
            (result, true, shifted.carry_out, cpu.flag_v())
        }
        0b1101 => {
            let result = op2;
            (result, true, shifted.carry_out, cpu.flag_v())
        }
        0b1110 => {
            let result = rn_val & !op2;
            cpu.set_flag_nz_data(result, shifted.carry_out);
            (result, true, shifted.carry_out, cpu.flag_v())
        }
        0b1111 => {
            let result = !op2;
            cpu.set_flag_nz_data(result, shifted.carry_out);
            (result, true, shifted.carry_out, cpu.flag_v())
        }
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
                cpu.cpsr = spsr;
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
    let accumulate = (instruction >> 21) & 1 == 1;
    let set_flags = (instruction >> 20) & 1 == 1;
    let rd_hi = ((instruction >> 16) & 0xF) as usize;
    let rd_lo = ((instruction >> 12) & 0xF) as usize;
    let rs = ((instruction >> 8) & 0xF) as usize;
    let rm = (instruction & 0xF) as usize;

    let rm_val = cpu.reg(rm) as i32 as i64;
    let rs_val = cpu.reg(rs) as i32 as i64;

    let unsigned_mul = (instruction >> 22) & 1 == 1;

    let result = if unsigned_mul {
        let rm_u = cpu.reg(rm) as u64;
        let rs_u = cpu.reg(rs) as u64;
        rm_u.wrapping_mul(rs_u)
    } else {
        (rm_val.wrapping_mul(rs_val)) as u64
    };

    let result = if accumulate {
        let hi = cpu.reg(rd_hi) as u64;
        let lo = cpu.reg(rd_lo) as u64;
        (hi << 32) | lo
    } else {
        result
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
        let mem_val = bus.read32(addr & !0x3);
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
        cpu.cpsr = (cpu.cpsr & !mask) | (new_psr & mask);
    }

    1
}

// ---------------------------------------------------------------------------
// Single Data Transfer (LDR, STR, LDRB, STRB)
// ---------------------------------------------------------------------------

fn single_data_transfer(instruction: u32, cpu: &mut Cpu, bus: &mut MemoryBus) -> u32 {
    let immediate_offset = (instruction >> 25) & 1 == 0;
    let up_down = (instruction >> 23) & 1 == 1;
    let byte_transfer = (instruction >> 22) & 1 == 1;
    let write_back = (instruction >> 21) & 1 == 1;
    let load = (instruction >> 20) & 1 == 1;
    let rn = ((instruction >> 16) & 0xF) as usize;
    let rd = ((instruction >> 12) & 0xF) as usize;

    let base = cpu.reg(rn);
    let offset = if immediate_offset {
        let shift_imm = ((instruction >> 4) & 0xF) * 2;
        let rm = (instruction & 0xF) as usize;
        let rm_val = cpu.reg(rm);
        match (instruction >> 5) & 3 {
            0b00 => rm_val.wrapping_shl(shift_imm),
            0b01 => rm_val.wrapping_shr(if shift_imm == 0 { 32 } else { shift_imm }),
            0b10 => ((rm_val as i32) >> if shift_imm == 0 { 32 } else { shift_imm }) as u32,
            0b11 => rm_val.rotate_right(shift_imm),
            _ => unreachable!(),
        }
    } else {
        instruction & 0xFFF
    };

    let addr = if up_down {
        base.wrapping_add(offset)
    } else {
        base.wrapping_sub(offset)
    };

    if load {
        if byte_transfer {
            let val = bus.read8(addr);
            cpu.set_reg(rd, val as u32);
        } else {
            let val = bus.read32(addr);
            // Handle unaligned LDR
            let rotate_amount = (addr & 3) * 8;
            let val = val.rotate_right(rotate_amount);
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
            bus.write32(addr & !3, val);
        }
    }

    if write_back {
        cpu.set_reg(rn, addr);
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
    let rn = ((instruction >> 16) & 0xF) as usize;
    let reg_list = instruction & 0xFFFF;

    let up_down = (instruction >> 23) & 1 == 1;
    let base = cpu.reg(rn);

    let reg_count = reg_list.count_ones() as u32;

    let mut addr = if up_down {
        if pre_index {
            base.wrapping_add(reg_count * 4)
        } else {
            base
        }
    } else {
        if pre_index {
            base.wrapping_sub(reg_count * 4)
        } else {
            base.wrapping_sub(reg_count * 4)
        }
    };

    // Force word alignment
    addr &= !3;

    if load {
        for i in 0..16u32 {
            if reg_list & (1 << i) != 0 {
                let val = bus.read32(addr);
                cpu.set_reg(i as usize, val);
                addr = addr.wrapping_add(4);
            }
        }
    } else {
        for i in 0..16u32 {
            if reg_list & (1 << i) != 0 {
                let val = if i as usize == rn {
                    base
                } else if i == 15 {
                    cpu.registers[15].wrapping_add(4)
                } else {
                    cpu.reg(i as usize)
                };
                bus.write32(addr, val);
                addr = addr.wrapping_add(4);
            }
        }
    }

    if write_back {
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
