use super::Cpu;
use crate::memory::MemoryBus;

/// Execute a single THUMB instruction.
pub fn execute(instruction: u16, cpu: &mut Cpu, bus: &mut MemoryBus) -> u32 {
    let bits15_13 = (instruction >> 13) & 0x7;
    let bits15_12 = (instruction >> 12) & 0xF;
    let bits15_11 = (instruction >> 11) & 0x1F;
    let bits15_10 = (instruction >> 10) & 0x3F;

    match bits15_13 {
        // Format 1: LSL, LSR, ASR (shift by immediate)
        0b000 => {
            let shift_op = (instruction >> 11) & 3;
            let offset5 = (instruction >> 6) & 0x1F;
            let rs = ((instruction >> 3) & 7) as usize;
            let rd = (instruction & 7) as usize;
            let rs_val = cpu.reg(rs);

            match shift_op {
                0b00 => { // LSL
                    if offset5 == 0 {
                        cpu.set_reg(rd, rs_val);
                    } else {
                        let carry = (rs_val >> (32 - offset5)) & 1 == 1;
                        let val = rs_val.wrapping_shl(offset5 as u32);
                        cpu.set_flag_nz_data(val, carry);
                        cpu.set_reg(rd, val);
                    }
                }
                0b01 => { // LSR
                    if offset5 == 0 {
                        let carry = rs_val >> 31 == 1;
                        cpu.set_flag_nz_data(0, carry);
                        cpu.set_reg(rd, 0);
                    } else {
                        let carry = (rs_val >> (offset5 - 1)) & 1 == 1;
                        let val = rs_val.wrapping_shr(offset5 as u32);
                        cpu.set_flag_nz_data(val, carry);
                        cpu.set_reg(rd, val);
                    }
                }
                0b10 => { // ASR
                    if offset5 == 0 {
                        let carry = rs_val >> 31 == 1;
                        let val = if carry { 0xFFFFFFFF } else { 0 };
                        cpu.set_flag_nz_data(val, carry);
                        cpu.set_reg(rd, val);
                    } else {
                        let carry = (rs_val >> (offset5 - 1)) & 1 == 1;
                        let val = ((rs_val as i32) >> offset5) as u32;
                        cpu.set_flag_nz_data(val, carry);
                        cpu.set_reg(rd, val);
                    }
                }
                _ => unreachable!(),
            }
            1
        }

        // Format 2/3: ADD, SUB (various forms)
        0b001 => {
            let op_type = (instruction >> 11) & 3;
            match op_type {
                // Format 2: ADD, SUB (3-register or immediate 3-bit)
                0b010 | 0b011 => {
                    let imm_flag = (instruction >> 10) & 1;
                    let rn_offset = ((instruction >> 6) & 7) as usize;
                    let rs = ((instruction >> 3) & 7) as usize;
                    let rd = (instruction & 7) as usize;
                    let rs_val = cpu.reg(rs);
                    let operand = if imm_flag == 1 {
                        rn_offset as u32
                    } else {
                        cpu.reg(rn_offset)
                    };
                    let is_sub = (instruction >> 9) & 1 == 1;

                    if is_sub {
                        let result = rs_val.wrapping_sub(operand);
                        let carry = rs_val >= operand;
                        let overflow = crate::cpu::arm::overflow_sub(rs_val, operand, result);
                        cpu.set_flags(result >> 31 == 1, result == 0, carry, overflow);
                        cpu.set_reg(rd, result);
                    } else {
                        let result = rs_val.wrapping_add(operand);
                        let carry = (rs_val as u64) + (operand as u64) > 0xFFFF_FFFF;
                        let overflow = crate::cpu::arm::overflow_add(rs_val, operand, result);
                        cpu.set_flags(result >> 31 == 1, result == 0, carry, overflow);
                        cpu.set_reg(rd, result);
                    }
                    1
                }
                // Format 3: ADD, SUB, MOV, CMP (immediate 8-bit)
                _ => {
                    let op = (instruction >> 11) & 3;
                    let rd = ((instruction >> 8) & 7) as usize;
                    let imm8 = (instruction & 0xFF) as u32;
                    let rd_val = cpu.reg(rd);

                    match op {
                        0b00 => { // MOV
                            cpu.set_flag_nz(imm8);
                            cpu.set_reg(rd, imm8);
                        }
                        0b01 => { // CMP
                            let result = rd_val.wrapping_sub(imm8);
                            let carry = rd_val >= imm8;
                            let overflow = crate::cpu::arm::overflow_sub(rd_val, imm8, result);
                            cpu.set_flags(result >> 31 == 1, result == 0, carry, overflow);
                        }
                        0b10 => { // ADD
                            let result = rd_val.wrapping_add(imm8);
                            let carry = (rd_val as u64) + (imm8 as u64) > 0xFFFF_FFFF;
                            let overflow = crate::cpu::arm::overflow_add(rd_val, imm8, result);
                            cpu.set_flags(result >> 31 == 1, result == 0, carry, overflow);
                            cpu.set_reg(rd, result);
                        }
                        0b11 => { // SUB
                            let result = rd_val.wrapping_sub(imm8);
                            let carry = rd_val >= imm8;
                            let overflow = crate::cpu::arm::overflow_sub(rd_val, imm8, result);
                            cpu.set_flags(result >> 31 == 1, result == 0, carry, overflow);
                            cpu.set_reg(rd, result);
                        }
                        _ => unreachable!(),
                    }
                    1
                }
            }
        }

        // Format 4-6 and Format 9 (immediate): all start with 0b011
        0b011 => {
            if bits15_10 == 0b0100_00 {
                // Format 4: ALU operations
                format4_alu(instruction, cpu);
                1
            } else if bits15_10 == 0b0100_01 {
                // Format 5: Hi register operations / BX
                format5_hireg(instruction, cpu);
                3
            } else if bits15_11 == 0b0100_1 {
                // Format 6: LDR (PC-relative)
                format6_ldr_pc(instruction, cpu, bus);
                3
            } else {
                // Format 9: LDR/STR (immediate offset) — bits [15:13] = 011, bit [12] = varies
                let load = (instruction >> 11) & 1 == 1;
                let offset5 = ((instruction >> 6) & 0x1F) * 4;
                let rn = ((instruction >> 3) & 7) as usize;
                let rd = (instruction & 7) as usize;
                let addr = cpu.reg(rn).wrapping_add(offset5 as u32);

                if load {
                    let val = bus.read32(addr);
                    let rotate = (addr & 3) * 8;
                    let val = val.rotate_right(rotate);
                    cpu.set_reg(rd, val);
                    3
                } else {
                    bus.write32(addr, cpu.reg(rd));
                    2
                }
            }
        }

        // Format 7-9: Load/Store register offset
        0b100 => {
            if bits15_12 == 0b0101 {
                // Format 7/8/9 register offset
                let bit9 = (instruction >> 9) & 1;
                let bit5 = (instruction >> 5) & 1;
                let bit10 = (instruction >> 10) & 1;

                if bit9 == 0 {
                    // Format 7: STR or LDR (register offset)
                    if bit5 == 0 {
                        format7_str(instruction, cpu, bus);
                    } else {
                        format7_ldr(instruction, cpu, bus);
                    }
                } else {
                    // Format 8/9: signed/unsigned halfword
                    if bit5 == 0 {
                        if bit10 == 0 {
                            format8_strh(instruction, cpu, bus);
                        } else {
                            format9_ldrsb(instruction, cpu, bus);
                        }
                    } else {
                        if bit10 == 0 {
                            format8_ldrh(instruction, cpu, bus);
                        } else {
                            format9_ldrsh(instruction, cpu, bus);
                        }
                    }
                }
                3
            } else {
                // Format 10: LDRH/STRH (immediate offset) — bits [15:11] = 10000 or 10001
                let load = (instruction >> 11) & 1 == 1;
                let offset5 = ((instruction >> 6) & 0x1F) * 2;
                let rn = ((instruction >> 3) & 7) as usize;
                let rd = (instruction & 7) as usize;
                let addr = cpu.reg(rn).wrapping_add(offset5 as u32);

                if load {
                    let val = bus.read16(addr);
                    cpu.set_reg(rd, val as u32);
                    3
                } else {
                    bus.write16(addr, cpu.reg(rd) as u16);
                    2
                }
            }
        }

        // Format 11: SP-relative LDR/STR
        0b1001 => {
            let load = (instruction >> 11) & 1 == 1;
            let rd = ((instruction >> 8) & 7) as usize;
            let imm8 = (instruction & 0xFF) as u32;
            let offset = imm8 * 4;
            let sp = cpu.registers[13];
            let addr = sp.wrapping_add(offset);

            if load {
                let val = bus.read32(addr);
                cpu.set_reg(rd, val);
                3
            } else {
                bus.write32(addr, cpu.reg(rd));
                2
            }
        }

        // Format 12: ADD PC/SP
        0b1010 => {
            let sp_flag = (instruction >> 11) & 1;
            let rd = ((instruction >> 8) & 7) as usize;
            let imm8 = (instruction & 0xFF) as u32;

            let result = if sp_flag == 1 {
                cpu.registers[13].wrapping_add(imm8 * 4)
            } else {
                (cpu.registers[15] & !2).wrapping_add(imm8 * 4)
            };

            cpu.set_reg(rd, result);
            1
        }

        // Format 13/14: ADD SP / PUSH / POP
        0b1011 => {
            let bits11_8 = (instruction >> 8) & 0xF;
            if bits11_8 == 0b0000 {
                // Format 13: ADD SP
                let op = (instruction >> 7) & 1;
                let imm7 = (instruction & 0x7F) as u32;
                if op == 0 {
                    cpu.registers[13] = cpu.registers[13].wrapping_add(imm7 * 4);
                } else {
                    cpu.registers[13] = cpu.registers[13].wrapping_sub(imm7 * 4);
                }
                1
            } else {
                // Format 14: PUSH/POP
                let l = (instruction >> 11) & 1;
                let r = (instruction >> 8) & 1;
                let reg_list = instruction & 0xFF;

                if l == 0 {
                    // PUSH
                    let reg_count = reg_list.count_ones() + r as u32;
                    let mut sp = cpu.registers[13].wrapping_sub(reg_count * 4);
                    sp &= !3;
                    for i in 0..8u16 {
                        if reg_list & (1 << i) != 0 {
                            bus.write32(sp, cpu.reg(i as usize));
                            sp = sp.wrapping_add(4);
                        }
                    }
                    if r == 1 {
                        bus.write32(sp, cpu.registers[14]);
                    }
                    cpu.registers[13] = cpu.registers[13].wrapping_sub(reg_count * 4);
                    2 + reg_count
                } else {
                    // POP
                    let reg_count = reg_list.count_ones() + r as u32;
                    let mut sp = cpu.registers[13];
                    for i in 0..8u16 {
                        if reg_list & (1 << i) != 0 {
                            let val = bus.read32(sp);
                            cpu.set_reg(i as usize, val);
                            sp = sp.wrapping_add(4);
                        }
                    }
                    if r == 1 {
                        let val = bus.read32(sp);
                        if val & 1 == 1 {
                            cpu.cpsr |= 0x20;
                            cpu.set_reg(15, val & !1);
                        } else {
                            cpu.cpsr &= !0x20;
                            cpu.set_reg(15, val & !3);
                        }
                        sp = sp.wrapping_add(4);
                    }
                    cpu.registers[13] = sp;
                    2 + reg_count
                }
            }
        }

        // Format 15: STMIA/LDMIA
        0b1100 => {
            let load = (instruction >> 11) & 1 == 1;
            let rn = ((instruction >> 8) & 7) as usize;
            let reg_list = instruction & 0xFF;
            let reg_count = reg_list.count_ones();
            let mut addr = cpu.reg(rn);

            if load {
                for i in 0..8u16 {
                    if reg_list & (1 << i) != 0 {
                        let val = bus.read32(addr);
                        cpu.set_reg(i as usize, val);
                        addr = addr.wrapping_add(4);
                    }
                }
                cpu.set_reg(rn, addr);
                2 + reg_count
            } else {
                for i in 0..8u16 {
                    if reg_list & (1 << i) != 0 {
                        bus.write32(addr, cpu.reg(i as usize));
                        addr = addr.wrapping_add(4);
                    }
                }
                cpu.set_reg(rn, addr);
                1 + reg_count
            }
        }

        // Format 16/17/18/19: Conditional branch, SWI, Unconditional branch, Long branch
        0b1101 => {
            if bits15_12 == 0b1101 {
                let cond = (instruction >> 8) & 0xF;
                if cond == 0b1111 {
                    // Format 17: SWI
                    let comment = instruction & 0xFF;
                    cpu.swi(comment as u32, bus);
                    3
                } else if cond == 0b1110 {
                    // Undefined, treat as NOP
                    1
                } else if cpu.condition_met(cond as u32) {
                    // Format 16: Conditional branch
                    let offset = (instruction & 0xFF) as i8 as i32;
                    let pc = cpu.registers[15];
                    let target = pc.wrapping_add((offset << 1) as u32);
                    cpu.set_reg(15, target);
                    3
                } else {
                    1
                }
            } else {
                // Format 19: Long branch with link (first part: 11110 or 11101)
                let h = (instruction >> 11) & 1;
                if h == 0 {
                    let offset11 = (instruction & 0x7FF) as u32;
                    let pc = cpu.registers[15];
                    let offset = (offset11 << 12) as i32;
                    let lr = pc.wrapping_add(offset as u32);
                    cpu.set_reg(14, lr);
                } else {
                    let offset11 = (instruction & 0x7FF) as u32;
                    let lr = cpu.registers[14];
                    let old_pc = cpu.registers[15];
                    let pc = lr.wrapping_add(offset11 << 1);
                    cpu.set_reg(14, old_pc | 1);
                    cpu.set_reg(15, pc);
                }
                3
            }
        }

        // Format 18: Unconditional branch
        0b11100 => {
            let offset11 = (instruction & 0x7FF) as i16 as i32;
            let pc = cpu.registers[15];
            let target = pc.wrapping_add((offset11 << 1) as u32);
            cpu.set_reg(15, target);
            3
        }

        // Format 19: Long branch with link (second part)
        0b11110 | 0b11101 => {
            let h = (instruction >> 11) & 1;
            if h == 0 {
                let offset11 = (instruction & 0x7FF) as u32;
                let pc = cpu.registers[15];
                let offset = (offset11 << 12) as i32;
                let lr = pc.wrapping_add(offset as u32);
                cpu.set_reg(14, lr);
            } else {
                let offset11 = (instruction & 0x7FF) as u32;
                let lr = cpu.registers[14];
                let old_pc = cpu.registers[15];
                let pc = lr.wrapping_add(offset11 << 1);
                cpu.set_reg(14, old_pc | 1);
                cpu.set_reg(15, pc);
            }
            3
        }

        _ => 1,
    }
}

// ---------------------------------------------------------------------------
// Format 4: ALU operations
// ---------------------------------------------------------------------------

fn format4_alu(instruction: u16, cpu: &mut Cpu) {
    let op = (instruction >> 6) & 0xF;
    let rs = ((instruction >> 3) & 7) as usize;
    let rd = (instruction & 7) as usize;

    let rs_val = cpu.reg(rs);
    let rd_val = cpu.reg(rd);
    let result;

    match op {
        0b0000 => { // AND
            result = rd_val & rs_val;
            cpu.set_reg(rd, result);
            cpu.set_flag_nz(result);
        }
        0b0001 => { // EOR
            result = rd_val ^ rs_val;
            cpu.set_reg(rd, result);
            cpu.set_flag_nz(result);
        }
        0b0010 => { // LSL
            let shift = rs_val & 0xFF;
            if shift == 0 {
                result = rd_val;
                cpu.set_reg(rd, result);
            } else if shift < 32 {
                let carry = (rd_val >> (32 - shift)) & 1 == 1;
                result = rd_val.wrapping_shl(shift);
                cpu.set_flag_nz_data(result, carry);
                cpu.set_reg(rd, result);
            } else if shift == 32 {
                cpu.set_flag_nz_data(0, rd_val & 1 == 1);
                cpu.set_reg(rd, 0);
            } else {
                cpu.set_flag_nz_data(0, false);
                cpu.set_reg(rd, 0);
            }
        }
        0b0011 => { // LSR
            let shift = rs_val & 0xFF;
            if shift == 0 {
                result = rd_val;
                cpu.set_reg(rd, result);
            } else if shift < 32 {
                let carry = (rd_val >> (shift - 1)) & 1 == 1;
                result = rd_val.wrapping_shr(shift);
                cpu.set_flag_nz_data(result, carry);
                cpu.set_reg(rd, result);
            } else if shift == 32 {
                cpu.set_flag_nz_data(0, rd_val >> 31 == 1);
                cpu.set_reg(rd, 0);
            } else {
                cpu.set_flag_nz_data(0, false);
                cpu.set_reg(rd, 0);
            }
        }
        0b0100 => { // ASR
            let shift = rs_val & 0xFF;
            if shift == 0 {
                result = rd_val;
                cpu.set_reg(rd, result);
            } else if shift < 32 {
                let carry = (rd_val >> (shift - 1)) & 1 == 1;
                result = ((rd_val as i32) >> shift) as u32;
                cpu.set_flag_nz_data(result, carry);
                cpu.set_reg(rd, result);
            } else {
                let carry = rd_val >> 31 == 1;
                result = if carry { 0xFFFFFFFF } else { 0 };
                cpu.set_flag_nz_data(result, carry);
                cpu.set_reg(rd, result);
            }
        }
        0b0101 => { // ADC
            let carry = cpu.flag_c() as u32;
            result = rd_val.wrapping_add(rs_val).wrapping_add(carry);
            let carry_out = (rd_val as u64) + (rs_val as u64) + (carry as u64) > 0xFFFF_FFFF;
            let overflow = crate::cpu::arm::overflow_add(rd_val, rs_val, result);
            cpu.set_flags(result >> 31 == 1, result == 0, carry_out, overflow);
            cpu.set_reg(rd, result);
        }
        0b0110 => { // SBC
            let carry = cpu.flag_c() as u32;
            result = rd_val.wrapping_sub(rs_val).wrapping_sub(1 - carry);
            let carry_out = (rd_val as u64) >= (rs_val as u64) + (1 - carry as u64);
            let overflow = crate::cpu::arm::overflow_sub(rd_val, rs_val, result);
            cpu.set_flags(result >> 31 == 1, result == 0, carry_out, overflow);
            cpu.set_reg(rd, result);
        }
        0b0111 => { // ROR
            let shift = rs_val & 0xFF;
            if shift == 0 {
                result = rd_val;
                cpu.set_reg(rd, result);
            } else if shift % 32 == 0 {
                cpu.set_flag_nz_data(rd_val, rd_val >> 31 == 1);
                cpu.set_reg(rd, rd_val);
            } else {
                result = rd_val.rotate_right(shift % 32);
                cpu.set_flag_nz_data(result, result >> 31 == 1);
                cpu.set_reg(rd, result);
            }
        }
        0b1000 => { // TST
            result = rd_val & rs_val;
            cpu.set_flag_nz(result);
        }
        0b1001 => { // NEG
            result = 0u32.wrapping_sub(rs_val);
            let carry = 0 >= rs_val;
            let overflow = crate::cpu::arm::overflow_sub(0, rs_val, result);
            cpu.set_flags(result >> 31 == 1, result == 0, carry, overflow);
            cpu.set_reg(rd, result);
        }
        0b1010 => { // CMP
            result = rd_val.wrapping_sub(rs_val);
            let carry = rd_val >= rs_val;
            let overflow = crate::cpu::arm::overflow_sub(rd_val, rs_val, result);
            cpu.set_flags(result >> 31 == 1, result == 0, carry, overflow);
        }
        0b1011 => { // CMN
            result = rd_val.wrapping_add(rs_val);
            let carry = (rd_val as u64) + (rs_val as u64) > 0xFFFF_FFFF;
            let overflow = crate::cpu::arm::overflow_add(rd_val, rs_val, result);
            cpu.set_flags(result >> 31 == 1, result == 0, carry, overflow);
        }
        0b1100 => { // ORR
            result = rd_val | rs_val;
            cpu.set_reg(rd, result);
            cpu.set_flag_nz(result);
        }
        0b1101 => { // MUL
            result = rd_val.wrapping_mul(rs_val);
            cpu.set_reg(rd, result);
            cpu.set_flag_nz(result);
        }
        0b1110 => { // BIC
            result = rd_val & !rs_val;
            cpu.set_reg(rd, result);
            cpu.set_flag_nz(result);
        }
        0b1111 => { // MVN
            result = !rs_val;
            cpu.set_reg(rd, result);
            cpu.set_flag_nz(result);
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Format 5: Hi register operations / BX
// ---------------------------------------------------------------------------

fn format5_hireg(instruction: u16, cpu: &mut Cpu) {
    let op = (instruction >> 8) & 3;
    let h1 = (instruction >> 7) & 1;
    let h2 = (instruction >> 6) & 1;
    let rs = ((instruction >> 3) & 7) | (h2 << 3);
    let rd = (instruction & 7) | (h1 << 3);
    let rs_val = cpu.reg(rs as usize);

    match op {
        0b00 => { // ADD
            let rd_val = cpu.reg(rd as usize);
            let result = rd_val.wrapping_add(rs_val);
            cpu.set_reg(rd as usize, result);
            if rd == 15 {
                cpu.registers[15] &= !1;
            }
        }
        0b01 => { // CMP
            let rd_val = cpu.reg(rd as usize);
            let result = rd_val.wrapping_sub(rs_val);
            let carry = rd_val >= rs_val;
            let overflow = crate::cpu::arm::overflow_sub(rd_val, rs_val, result);
            cpu.set_flags(result >> 31 == 1, result == 0, carry, overflow);
        }
        0b10 => { // MOV
            cpu.set_reg(rd as usize, rs_val);
            if rd == 15 {
                cpu.registers[15] &= !1;
            }
        }
        0b11 => { // BX
            if rs_val & 1 == 1 {
                cpu.cpsr |= 0x20;
                cpu.set_reg(15, rs_val & !1);
            } else {
                cpu.cpsr &= !0x20;
                cpu.set_reg(15, rs_val & !3);
            }
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Format 6: LDR (PC-relative)
// ---------------------------------------------------------------------------

fn format6_ldr_pc(instruction: u16, cpu: &mut Cpu, bus: &mut MemoryBus) {
    let rd = ((instruction >> 8) & 7) as usize;
    let imm8 = (instruction & 0xFF) as u32;
    let addr = (cpu.registers[15] & !2).wrapping_add(imm8 * 4);
    let val = bus.read32(addr);
    cpu.set_reg(rd, val);
}

// ---------------------------------------------------------------------------
// Format 7: STR, LDR (register offset)
// ---------------------------------------------------------------------------

fn format7_str(instruction: u16, cpu: &mut Cpu, bus: &mut MemoryBus) {
    let offset = ((instruction >> 6) & 7) as usize;
    let rb = ((instruction >> 3) & 7) as usize;
    let rd = (instruction & 7) as usize;
    let addr = cpu.reg(rb).wrapping_add(cpu.reg(offset));
    bus.write32(addr, cpu.reg(rd));
}

fn format7_ldr(instruction: u16, cpu: &mut Cpu, bus: &mut MemoryBus) {
    let offset = ((instruction >> 6) & 7) as usize;
    let rb = ((instruction >> 3) & 7) as usize;
    let rd = (instruction & 7) as usize;
    let addr = cpu.reg(rb).wrapping_add(cpu.reg(offset));
    let val = bus.read32(addr);
    let rotate = (addr & 3) * 8;
    let val = val.rotate_right(rotate);
    cpu.set_reg(rd, val);
}

// ---------------------------------------------------------------------------
// Format 8: STRH, LDRH (register offset)
// ---------------------------------------------------------------------------

fn format8_strh(instruction: u16, cpu: &mut Cpu, bus: &mut MemoryBus) {
    let offset = ((instruction >> 6) & 7) as usize;
    let rb = ((instruction >> 3) & 7) as usize;
    let rd = (instruction & 7) as usize;
    let addr = cpu.reg(rb).wrapping_add(cpu.reg(offset));
    bus.write16(addr, cpu.reg(rd) as u16);
}

fn format8_ldrh(instruction: u16, cpu: &mut Cpu, bus: &mut MemoryBus) {
    let offset = ((instruction >> 6) & 7) as usize;
    let rb = ((instruction >> 3) & 7) as usize;
    let rd = (instruction & 7) as usize;
    let addr = cpu.reg(rb).wrapping_add(cpu.reg(offset));
    let val = bus.read16(addr);
    cpu.set_reg(rd, val as u32);
}

// ---------------------------------------------------------------------------
// Format 9: LDRSB, LDRSH
// ---------------------------------------------------------------------------

fn format9_ldrsb(instruction: u16, cpu: &mut Cpu, bus: &mut MemoryBus) {
    let offset = ((instruction >> 6) & 7) as usize;
    let rb = ((instruction >> 3) & 7) as usize;
    let rd = (instruction & 7) as usize;
    let addr = cpu.reg(rb).wrapping_add(cpu.reg(offset));
    let val = bus.read8(addr) as i8 as i32 as u32;
    cpu.set_reg(rd, val);
}

fn format9_ldrsh(instruction: u16, cpu: &mut Cpu, bus: &mut MemoryBus) {
    let offset = ((instruction >> 6) & 7) as usize;
    let rb = ((instruction >> 3) & 7) as usize;
    let rd = (instruction & 7) as usize;
    let addr = cpu.reg(rb).wrapping_add(cpu.reg(offset));
    let val = bus.read16(addr) as i16 as i32 as u32;
    cpu.set_reg(rd, val);
}
