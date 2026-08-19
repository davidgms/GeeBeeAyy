pub mod arm;
pub mod thumb;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Mode {
    User = 0b10000,
    Fiq = 0b10001,
    Irq = 0b10010,
    Supervisor = 0b10011,
    Abort = 0b10111,
    Undefined = 0b11011,
    System = 0b11111,
}

pub struct Cpu {
    pub registers: [u32; 16],
    pub fiq_registers: [u32; 7],
    pub irq_registers: [u32; 2],
    pub cpsr: u32,
    pub spsr_fiq: u32,
    pub spsr_irq: u32,
    pub spsr_svc: u32,
    pub spsr_abt: u32,
    pub spsr_und: u32,
    pub halted: bool,
}

pub struct BarrelShiftResult {
    pub value: u32,
    pub carry_out: bool,
}

impl Cpu {
    pub fn new() -> Self {
        Self {
            registers: [0; 16],
            fiq_registers: [0; 7],
            irq_registers: [0; 2],
            cpsr: Mode::System as u32,
            spsr_fiq: 0,
            spsr_irq: 0,
            spsr_svc: 0,
            spsr_abt: 0,
            spsr_und: 0,
            halted: false,
        }
    }

    pub fn step(&mut self, bus: &mut super::memory::MemoryBus) -> u32 {
        if self.halted {
            return 1;
        }

        let pc = self.registers[15];

        let is_thumb = self.cpsr & 0x20 != 0;

        let cycles = if is_thumb {
            let instruction = bus.read16(pc) as u16;
            self.registers[15] = pc.wrapping_add(4);
            self.execute_thumb(instruction, bus)
        } else {
            let instruction = bus.read32(pc);
            self.registers[15] = pc.wrapping_add(8);
            self.execute_arm(instruction, bus)
        };

        // Ensure PC bit 0 and bit 1 are always zero (ARM/THUMB alignment)
        self.registers[15] &= !0x3;

        cycles
    }

    pub fn execute_arm(&mut self, instruction: u32, bus: &mut super::memory::MemoryBus) -> u32 {
        let cond = (instruction >> 28) & 0xF;
        if !self.condition_met(cond) {
            return 1;
        }
        arm::execute(instruction, self, bus)
    }

    pub fn execute_thumb(&mut self, instruction: u16, bus: &mut super::memory::MemoryBus) -> u32 {
        thumb::execute(instruction, self, bus)
    }

    pub fn mode(&self) -> Mode {
        match self.cpsr & 0x1F {
            0b10000 => Mode::User,
            0b10001 => Mode::Fiq,
            0b10010 => Mode::Irq,
            0b10011 => Mode::Supervisor,
            0b10111 => Mode::Abort,
            0b11011 => Mode::Undefined,
            0b11111 => Mode::System,
            _ => Mode::System,
        }
    }

    pub fn flag_n(&self) -> bool { self.cpsr & (1 << 31) != 0 }
    pub fn flag_z(&self) -> bool { self.cpsr & (1 << 30) != 0 }
    pub fn flag_c(&self) -> bool { self.cpsr & (1 << 29) != 0 }
    pub fn flag_v(&self) -> bool { self.cpsr & (1 << 28) != 0 }

    pub fn set_flags(&mut self, n: bool, z: bool, c: bool, v: bool) {
        self.cpsr &= 0x0FFF_FFFF;
        if n { self.cpsr |= 1 << 31; }
        if z { self.cpsr |= 1 << 30; }
        if c { self.cpsr |= 1 << 29; }
        if v { self.cpsr |= 1 << 28; }
    }

    pub fn set_flag_nz(&mut self, result: u32) {
        let n = result >> 31 == 1;
        let z = result == 0;
        let c = self.flag_c();
        let v = self.flag_v();
        self.set_flags(n, z, c, v);
    }

    pub fn set_flag_nz_data(&mut self, result: u32, carry: bool) {
        let n = result >> 31 == 1;
        let z = result == 0;
        let v = self.flag_v();
        self.set_flags(n, z, carry, v);
    }

    /// Evaluate ARM condition code
    fn condition_met(&self, cond: u32) -> bool {
        match cond {
            0b0000 => self.flag_z(),                     // EQ
            0b0001 => !self.flag_z(),                    // NE
            0b0010 => self.flag_c(),                     // CS/HS
            0b0011 => !self.flag_c(),                    // CC/LO
            0b0100 => self.flag_n(),                     // MI
            0b0101 => !self.flag_n(),                    // PL
            0b0110 => self.flag_v(),                     // VS
            0b0111 => !self.flag_v(),                    // VC
            0b1000 => self.flag_c() && !self.flag_z(),   // HI
            0b1001 => !self.flag_c() || self.flag_z(),   // LS
            0b1010 => self.flag_n() == self.flag_v(),    // GE
            0b1011 => self.flag_n() != self.flag_v(),    // LT
            0b1100 => !self.flag_z() && (self.flag_n() == self.flag_v()), // GT
            0b1101 => self.flag_z() || (self.flag_n() != self.flag_v()),  // LE
            0b1110 => true,                              // AL (always)
            0b1111 => false,
            _ => false,
        }
    }

    /// Barrel shifter: LSL
    pub fn lsl(&mut self, value: u32, shift: u32) -> BarrelShiftResult {
        if shift == 0 {
            BarrelShiftResult { value, carry_out: self.flag_c() }
        } else if shift < 32 {
            let carry_out = (value >> (32 - shift)) & 1 == 1;
            BarrelShiftResult { value: value << shift, carry_out }
        } else if shift == 32 {
            BarrelShiftResult { value: 0, carry_out: value & 1 == 1 }
        } else {
            BarrelShiftResult { value: 0, carry_out: false }
        }
    }

    /// Barrel shifter: LSR
    pub fn lsr(&mut self, value: u32, shift: u32) -> BarrelShiftResult {
        if shift == 0 {
            BarrelShiftResult { value, carry_out: self.flag_c() }
        } else if shift < 32 {
            let carry_out = (value >> (shift - 1)) & 1 == 1;
            BarrelShiftResult { value: value >> shift, carry_out }
        } else if shift == 32 {
            BarrelShiftResult { value: 0, carry_out: value >> 31 == 1 }
        } else {
            BarrelShiftResult { value: 0, carry_out: false }
        }
    }

    /// Barrel shifter: ASR
    pub fn asr(&mut self, value: u32, shift: u32) -> BarrelShiftResult {
        if shift == 0 {
            BarrelShiftResult { value, carry_out: self.flag_c() }
        } else if shift < 32 {
            let carry_out = (value >> (shift - 1)) & 1 == 1;
            let result = ((value as i32) >> shift) as u32;
            BarrelShiftResult { value: result, carry_out }
        } else {
            let carry_out = value >> 31 == 1;
            let result = if carry_out { 0xFFFFFFFF } else { 0 };
            BarrelShiftResult { value: result, carry_out }
        }
    }

    /// Barrel shifter: ROR
    pub fn ror(&mut self, value: u32, shift: u32) -> BarrelShiftResult {
        if shift == 0 {
            BarrelShiftResult { value, carry_out: self.flag_c() }
        } else {
            let shift = shift % 32;
            if shift == 0 {
                BarrelShiftResult { value, carry_out: value >> 31 == 1 }
            } else {
                let result = value.rotate_right(shift);
                let carry_out = result >> 31 == 1;
                BarrelShiftResult { value: result, carry_out }
            }
        }
    }

    /// Barrel shifter: RRX (rotate right extended)
    pub fn rrx(&mut self, value: u32) -> BarrelShiftResult {
        let old_carry = self.flag_c() as u32;
        let result = (old_carry << 31) | (value >> 1);
        let carry_out = value & 1 == 1;
        BarrelShiftResult { value: result, carry_out }
    }

    /// Decode immediate operand for ARM data processing (8-bit value + 4-bit rotation)
    pub fn decode_imm(&mut self, instruction: u32) -> BarrelShiftResult {
        let rotate = ((instruction >> 8) & 0xF) * 2;
        let imm8 = instruction & 0xFF;
        if rotate == 0 {
            BarrelShiftResult { value: imm8, carry_out: self.flag_c() }
        } else {
            let result = imm8.rotate_right(rotate);
            let carry_out = result >> 31 == 1;
            BarrelShiftResult { value: result, carry_out }
        }
    }

    /// Barrel shifter for ARM operand 2 (register or immediate)
    pub fn shift_operand2(&mut self, instruction: u32, immediate: bool) -> BarrelShiftResult {
        if immediate {
            self.decode_imm(instruction)
        } else {
            let rm = (instruction & 0xF) as usize;
            let shift_type = (instruction >> 5) & 0x3;
            let shift_imm = (instruction >> 7) & 0x1F;
            let shift_by_reg = (instruction >> 4) & 1 == 1;

            let rm_val = self.registers[rm];

            if shift_by_reg {
                // Register shift: shift amount comes from bits [11:8] (Rs)
                let rs = ((instruction >> 8) & 0xF) as usize;
                let shift_amount = (self.registers[rs] & 0xFF) as u32;
                match shift_type {
                    0b00 => self.lsl(rm_val, shift_amount),
                    0b01 => self.lsr(rm_val, shift_amount),
                    0b10 => self.asr(rm_val, shift_amount),
                    0b11 => {
                        if shift_amount == 0 {
                            BarrelShiftResult { value: rm_val, carry_out: self.flag_c() }
                        } else if shift_amount % 32 == 0 {
                            BarrelShiftResult { value: rm_val, carry_out: rm_val >> 31 == 1 }
                        } else {
                            self.ror(rm_val, shift_amount)
                        }
                    }
                    _ => unreachable!(),
                }
            } else {
                // Immediate shift
                match shift_type {
                    0b00 => self.lsl(rm_val, shift_imm),
                    0b01 => self.lsr(rm_val, if shift_imm == 0 { 32 } else { shift_imm }),
                    0b10 => self.asr(rm_val, if shift_imm == 0 { 32 } else { shift_imm }),
                    0b11 => {
                        if shift_imm == 0 {
                            self.rrx(rm_val)
                        } else {
                            self.ror(rm_val, shift_imm)
                        }
                    }
                    _ => unreachable!(),
                }
            }
        }
    }

    /// Get register with PC-aware read (R15 reads PC+8 during ARM, PC+4 during THUMB)
    pub fn reg(&self, reg: usize) -> u32 {
        self.registers[reg]
    }

    /// Write register, special handling for PC
    pub fn set_reg(&mut self, reg: usize, value: u32) {
        self.registers[reg] = value;
        if reg == 15 {
            self.registers[15] &= !0x3;
        }
    }

    /// SWI handler placeholder
    pub fn swi(&mut self, _comment: u32) {
        // Placeholder for software interrupt handling
        // In a full implementation, this would:
        // 1. Save CPSR to SPSR_svc
        // 2. Set mode to Supervisor
        // 3. Disable IRQ
        // 4. Set LR to return address
        // 5. Jump to SWI vector (0x00000008)
    }

    /// IRQ handler placeholder
    pub fn handle_irq(&mut self) {
        // Placeholder for IRQ handling
        // In a full implementation:
        // 1. Save CPSR to SPSR_irq
        // 2. Set mode to IRQ
        // 3. Disable IRQ
        // 4. Set LR_irq to PC+4
        // 5. Jump to IRQ vector (0x00000018)
    }
}
