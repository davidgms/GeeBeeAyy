use super::Cpu;
use crate::memory::MemoryBus;

/// Execute a single THUMB instruction.
pub fn execute(instruction: u16, cpu: &mut Cpu, bus: &mut MemoryBus) -> u32 {
    let bits15_12 = (instruction >> 12) & 0xF;
    let bits15_10 = (instruction >> 10) & 0x3F;

    match bits15_12 {
        // Format 1/2: shift by immediate, ADD/SUB
        0b0000 | 0b0001 => {
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
                // Format 2: ADD/SUB with a register or 3-bit immediate operand.
                // bits[15:11] = 00011, so shift_op reads as 0b11 here. This
                // used to fall into `unreachable!()` and panic the emulator.
                _ => {
                    let immediate = (instruction >> 10) & 1 == 1;
                    let subtract = (instruction >> 9) & 1 == 1;
                    let field = (instruction >> 6) & 7;
                    let operand = if immediate {
                        field as u32
                    } else {
                        cpu.reg(field as usize)
                    };

                    let result = if subtract {
                        rs_val.wrapping_sub(operand)
                    } else {
                        rs_val.wrapping_add(operand)
                    };
                    let carry = if subtract {
                        rs_val >= operand
                    } else {
                        result < rs_val
                    };
                    let overflow = if subtract {
                        crate::cpu::arm::overflow_sub(rs_val, operand, result)
                    } else {
                        crate::cpu::arm::overflow_add(rs_val, operand, result)
                    };
                    cpu.set_flags(result >> 31 == 1, result == 0, carry, overflow);
                    cpu.set_reg(rd, result);
                }
            }
            1
        }

        // Format 3: MOV, CMP, ADD, SUB with an 8-bit immediate
        0b0010 | 0b0011 => {
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

        // Format 4/5/6: ALU, hi-register + BX, PC-relative load
        0b0100 => {
            if bits15_10 == 0b0100_00 {
                format4_alu(instruction, cpu);
                1
            } else if bits15_10 == 0b0100_01 {
                format5_hireg(instruction, cpu);
                3
            } else {
                // bits [15:11] = 01001 -> Format 6: LDR Rd, [PC, #imm]
                format6_ldr_pc(instruction, cpu, bus);
                3
            }
        }

        // Format 7/8: load/store with register offset
        0b0101 => {
            let load = (instruction >> 11) & 1 == 1;
            let flag = (instruction >> 10) & 1 == 1;
            if (instruction >> 9) & 1 == 0 {
                // Format 7: word / byte. bit11 = L, bit10 = B
                match (load, flag) {
                    (false, false) => format7_str(instruction, cpu, bus),
                    (false, true) => {
                        let addr = format7_addr(instruction, cpu);
                        bus.write8(addr, cpu.reg((instruction & 7) as usize) as u8);
                    }
                    (true, false) => format7_ldr(instruction, cpu, bus),
                    (true, true) => {
                        let addr = format7_addr(instruction, cpu);
                        let val = bus.read8(addr) as u32;
                        cpu.set_reg((instruction & 7) as usize, val);
                    }
                }
            } else {
                // Format 8: halfword / sign-extended. bit11 = H, bit10 = S
                match (flag, load) {
                    (false, false) => format8_strh(instruction, cpu, bus),
                    (false, true) => format8_ldrh(instruction, cpu, bus),
                    (true, false) => format9_ldrsb(instruction, cpu, bus),
                    (true, true) => format9_ldrsh(instruction, cpu, bus),
                }
            }
            3
        }

        // Format 9: LDR/STR with immediate offset
        0b0110 | 0b0111 => {
            let load = (instruction >> 11) & 1 == 1;
            let byte = (instruction >> 12) & 1 == 1;
            let rn = ((instruction >> 3) & 7) as usize;
            let rd = (instruction & 7) as usize;
            let offset5 = ((instruction >> 6) & 0x1F) as u32;
            let addr = cpu
                .reg(rn)
                .wrapping_add(if byte { offset5 } else { offset5 * 4 });

            match (load, byte) {
                (true, false) => {
                    let val = bus.read32_rotated(addr);
                    cpu.set_reg(rd, val);
                    3
                }
                (true, true) => {
                    cpu.set_reg(rd, bus.read8(addr) as u32);
                    3
                }
                (false, false) => {
                    bus.write32(addr, cpu.reg(rd));
                    2
                }
                (false, true) => {
                    bus.write8(addr, cpu.reg(rd) as u8);
                    2
                }
            }
        }

        // Format 10: LDRH/STRH with immediate offset
        0b1000 => {
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

        // Format 11: SP-relative LDR/STR
        0b1001 => {
            let load = (instruction >> 11) & 1 == 1;
            let rd = ((instruction >> 8) & 7) as usize;
            let imm8 = (instruction & 0xFF) as u32;
            let offset = imm8 * 4;
            let sp = cpu.registers[13];
            let addr = sp.wrapping_add(offset);

            if load {
                // SP-relative loads rotate on a misaligned SP, same as any LDR.
                let val = bus.read32_rotated(addr);
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
        0b1110 => {
            let offset11 = (instruction & 0x7FF) as i16 as i32;
            let pc = cpu.registers[15];
            let target = pc.wrapping_add((offset11 << 1) as u32);
            cpu.set_reg(15, target);
            3
        }

        // Format 19: Long branch with link (both halves; bit 11 picks which)
        0b1111 => {
            let offset11 = (instruction & 0x7FF) as u32;
            if (instruction >> 11) & 1 == 0 {
                // First half: LR = PC + (sign-extended offset << 12)
                let offset = (((offset11 << 12) as i32) << 9) >> 9;
                let lr = cpu.registers[15].wrapping_add(offset as u32);
                cpu.set_reg(14, lr);
            } else {
                // Second half: branch to LR + (offset << 1), LR = address of the
                // instruction after this half, with the THUMB bit set.
                let target = cpu.registers[14].wrapping_add(offset11 << 1);
                let return_addr = cpu.registers[15].wrapping_sub(2) | 1;
                cpu.set_reg(14, return_addr);
                cpu.set_reg(15, target);
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
            // NEG is RSB rd, rs, #0. C is "no borrow", which for 0 - rs_val
            // holds only when rs_val is 0.
            let carry = rs_val == 0;
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

fn format7_addr(instruction: u16, cpu: &Cpu) -> u32 {
    let offset = ((instruction >> 6) & 7) as usize;
    let rb = ((instruction >> 3) & 7) as usize;
    cpu.reg(rb).wrapping_add(cpu.reg(offset))
}

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
    let val = bus.read32_rotated(addr);
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
