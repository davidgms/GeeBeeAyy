pub mod arm;
mod prefetch;
pub mod thumb;

use crate::memory::MemoryBus;

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
    /// R8-R12 plus R13/R14, banked for FIQ only.
    pub fiq_registers: [u32; 7],
    /// R13, R14 for IRQ mode.
    pub irq_registers: [u32; 2],
    /// R13, R14 for Supervisor mode.
    pub svc_registers: [u32; 2],
    /// R13, R14 for Abort mode.
    pub abt_registers: [u32; 2],
    /// R13, R14 for Undefined mode.
    pub und_registers: [u32; 2],
    /// R13, R14 for User/System, parked here while a privileged mode runs.
    pub usr_registers: [u32; 2],
    /// R8-R12 for User/System, parked here only while FIQ runs.
    pub usr_r8_r12: [u32; 5],
    pub cpsr: u32,
    pub spsr_fiq: u32,
    pub spsr_irq: u32,
    pub spsr_svc: u32,
    pub spsr_abt: u32,
    pub spsr_und: u32,
    pub halted: bool,
    /// Set when an instruction writes R15, so `step` knows not to advance PC itself.
    pub(crate) branched: bool,
    /// Cycles owed by an exception entry, or by HLE BIOS code that ran in no
    /// time, charged to the next `step`.
    pub(crate) entry_cycles: u32,
    /// The next opcode fetch is non-sequential: the instruction before it
    /// made a data access, which takes the bus off the code, or (GBATEK,
    /// prefetch disable bug) ran internal cycles in ROM with the GamePak
    /// prefetch buffer off. Only ROM, where N and S differ, can tell.
    /// Transient, like `branched`, so not part of a save state.
    pub(crate) fetch_n: bool,
    /// The GamePak prefetch buffer. Transient: a save state restores with it
    /// empty, which costs at most a few cycles of one opcode fetch.
    prefetch: prefetch::Prefetch,
}

pub struct BarrelShiftResult {
    pub value: u32,
    pub carry_out: bool,
}

impl Default for Cpu {
    fn default() -> Self {
        Self::new()
    }
}

impl Cpu {
    pub fn new() -> Self {
        Self {
            registers: [0; 16],
            fiq_registers: [0; 7],
            irq_registers: [0; 2],
            svc_registers: [0; 2],
            abt_registers: [0; 2],
            und_registers: [0; 2],
            usr_registers: [0; 2],
            usr_r8_r12: [0; 5],
            cpsr: Mode::System as u32,
            spsr_fiq: 0,
            spsr_irq: 0,
            spsr_svc: 0,
            spsr_abt: 0,
            spsr_und: 0,
            halted: false,
            branched: false,
            entry_cycles: 0,
            fetch_n: false,
            prefetch: prefetch::Prefetch::default(),
        }
    }

    /// Initialize CPU state as if the GBA BIOS boot has completed.
    pub fn boot(&mut self) {
        // Conventional post-boot stacks, as the real BIOS sets them
        // (GBATEK, BIOS RAM Usage). SP itself has no hardware reset value.
        self.registers[13] = 0x0300_7F00; // sp_sys / sp_usr
        self.usr_registers[0] = 0x0300_7F00;
        self.svc_registers[0] = 0x0300_7FE0;
        self.irq_registers[0] = 0x0300_7FA0;
        self.registers[15] = 0x0800_0000; // PC = ROM entry point
        self.cpsr = Mode::System as u32; // System mode, IRQ disabled
        self.halted = false;
    }

    /// Write CPSR, switching the banked registers when the mode field changes.
    ///
    /// Every path that alters CPSR's mode bits must go through here. Writing
    /// `cpsr` directly leaves the wrong bank live, which shows up later as a
    /// corrupted stack rather than as an obvious fault.
    pub fn set_cpsr(&mut self, value: u32) {
        let old = self.mode();
        self.cpsr = value;
        let new = self.mode();
        if old != new {
            self.switch_bank(old, new);
        }
    }

    /// Park the outgoing mode's banked registers and load the incoming one's.
    ///
    /// GBATEK, ARM CPU Register Set: only FIQ banks R8-R12. Supervisor, Abort,
    /// IRQ and Undefined bank R13 and R14 alone, and System shares User's bank.
    fn switch_bank(&mut self, old: Mode, new: Mode) {
        // Save R13/R14 into the outgoing mode's bank.
        let outgoing = [self.registers[13], self.registers[14]];
        match old {
            Mode::Fiq => {
                self.fiq_registers[5] = outgoing[0];
                self.fiq_registers[6] = outgoing[1];
                for i in 0..5 {
                    self.fiq_registers[i] = self.registers[8 + i];
                }
            }
            Mode::Irq => self.irq_registers = outgoing,
            Mode::Supervisor => self.svc_registers = outgoing,
            Mode::Abort => self.abt_registers = outgoing,
            Mode::Undefined => self.und_registers = outgoing,
            Mode::User | Mode::System => self.usr_registers = outgoing,
        }

        // Restore R8-R12 to the User bank when leaving FIQ.
        if old == Mode::Fiq && new != Mode::Fiq {
            for i in 0..5 {
                self.registers[8 + i] = self.usr_r8_r12[i];
            }
        }
        // Park the User R8-R12 when entering FIQ.
        if new == Mode::Fiq && old != Mode::Fiq {
            for i in 0..5 {
                self.usr_r8_r12[i] = self.registers[8 + i];
                self.registers[8 + i] = self.fiq_registers[i];
            }
        }

        let incoming = match new {
            Mode::Fiq => [self.fiq_registers[5], self.fiq_registers[6]],
            Mode::Irq => self.irq_registers,
            Mode::Supervisor => self.svc_registers,
            Mode::Abort => self.abt_registers,
            Mode::Undefined => self.und_registers,
            Mode::User | Mode::System => self.usr_registers,
        };
        self.registers[13] = incoming[0];
        self.registers[14] = incoming[1];
    }

    pub fn step(&mut self, bus: &mut super::memory::MemoryBus) -> u32 {
        if self.halted {
            return 1;
        }

        let pc = self.registers[15];
        let is_thumb = self.cpsr & 0x20 != 0;
        self.branched = false;
        let entry = std::mem::take(&mut self.entry_cycles);
        bus.exec_pc = pc;
        bus.exec_thumb = is_thumb;
        if pc < 0x4000 {
            bus.latch_bios_prefetch();
        }

        // R15 reads as the address of the instruction plus two fetches while it
        // executes: +8 in ARM, +4 in THUMB. Each instruction pays for the
        // prefetch of that address; the decoders return what their data
        // accesses and internal cycles cost. Returned rather than added to a
        // field as they go: a running total in memory measured ~7% slower
        // per frame on Mario Tennis.
        let seq = !std::mem::take(&mut self.fetch_n);
        let mut cycles = if is_thumb {
            let instruction = bus.read16(pc);
            self.registers[15] = pc.wrapping_add(4);
            self.fetch_cycles(bus, pc.wrapping_add(4), false, seq)
                + self.execute_thumb(instruction, bus)
        } else {
            let instruction = bus.read32(pc);
            self.registers[15] = pc.wrapping_add(8);
            self.fetch_cycles(bus, pc.wrapping_add(8), true, seq)
                + self.execute_arm(instruction, bus)
        };

        // Only advance to the next instruction when the instruction itself did
        // not write PC; a branch has already put the target there.
        if !self.branched {
            self.registers[15] = pc.wrapping_add(if is_thumb { 2 } else { 4 });
        }

        // Align against the state we are in *now*: a BX or an `ldm ^` may have
        // flipped the T bit as part of this instruction.
        self.align_pc();

        // Any write to PC refills the pipeline at the target, in the state
        // the instruction left: one place for every branch, load to PC and
        // exception return.
        if self.branched {
            let (target, word) = (self.registers[15], self.cpsr & 0x20 == 0);
            cycles +=
                bus.access_cycles(target, word, false) + bus.access_cycles(target, word, true);
            self.fetch_n = false;
            // The buffer starts over behind the two opcodes just fetched.
            let width = if word { 4 } else { 2 };
            self.restart_prefetch(bus, target.wrapping_add(2 * width));
        }

        cycles + entry
    }

    /// What the next instruction's opcode fetch will cost, for a read that
    /// happens after it (see `Timer::counters`).
    pub(crate) fn next_fetch_cycles(&self, bus: &MemoryBus) -> u32 {
        let thumb = self.cpsr & 0x20 != 0;
        let address = self.registers[15].wrapping_add(if thumb { 4 } else { 8 });
        // On a copy: this only looks.
        let mut prefetch = self.prefetch;
        if prefetch.active && bus.prefetch_enabled() {
            if let Some(cycles) = prefetch.fetch(address, !thumb) {
                return cycles;
            }
        }
        bus.access_cycles(address, !thumb, !self.fetch_n)
    }

    /// An opcode fetch: from the prefetch buffer if it has the opcode,
    /// otherwise an ordinary access, behind which the buffer starts over.
    #[inline]
    fn fetch_cycles(&mut self, bus: &MemoryBus, address: u32, word: bool, seq: bool) -> u32 {
        if self.prefetch.active && bus.prefetch_enabled() {
            if let Some(cycles) = self.prefetch.fetch(address, word) {
                return cycles;
            }
        }
        let cycles = bus.access_cycles(address, word, seq);
        self.restart_prefetch(bus, address.wrapping_add(if word { 4 } else { 2 }));
        cycles
    }

    /// Point the prefetch buffer at `next`, empty, if opcodes there come from
    /// ROM with the buffer enabled; otherwise leave it stopped.
    #[inline]
    fn restart_prefetch(&mut self, bus: &MemoryBus, next: u32) {
        if prefetch::gamepak(next) && bus.prefetch_enabled() {
            // `| 2`: the halfword time itself, never the 128 KiB boundary's N.
            self.prefetch
                .restart(next, bus.access_cycles(next | 2, false, true));
        } else {
            self.prefetch.active = false;
        }
    }

    /// Cycles of a data access, which also leaves the next opcode fetch
    /// non-sequential.
    #[inline]
    pub(crate) fn data_cycles(
        &mut self,
        bus: &MemoryBus,
        address: u32,
        word: bool,
        seq: bool,
    ) -> u32 {
        self.fetch_n = true;
        let mut cycles = bus.access_cycles(address, word, seq);
        if self.prefetch.active {
            if prefetch::gamepak(address) {
                cycles += self.prefetch.stop();
            } else {
                self.prefetch.advance(cycles);
            }
        }
        cycles
    }

    /// `cycles` internal cycles with no data access. GBATEK's prefetch
    /// disable bug: with the GamePak prefetch buffer off, a ROM opcode with
    /// internal cycles that does not write R15 makes the next opcode fetch
    /// non-sequential. Outside ROM N and S cost the same, so the flag is
    /// harmless there.
    #[inline]
    pub(crate) fn idle_cycles(&mut self, bus: &MemoryBus, cycles: u32) -> u32 {
        if !bus.prefetch_enabled() {
            self.fetch_n = true;
        }
        self.prefetch.advance(cycles);
        cycles
    }

    /// Force PC alignment for the current instruction set: halfword in THUMB,
    /// word in ARM.
    fn align_pc(&mut self) {
        if self.cpsr & 0x20 != 0 {
            self.registers[15] &= !1;
        } else {
            self.registers[15] &= !3;
        }
    }

    pub fn execute_arm(&mut self, instruction: u32, bus: &mut super::memory::MemoryBus) -> u32 {
        let cond = (instruction >> 28) & 0xF;
        if self.condition_met(cond) {
            arm::execute(instruction, self, bus)
        } else {
            0
        }
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

    pub fn flag_n(&self) -> bool {
        self.cpsr & (1 << 31) != 0
    }
    pub fn flag_z(&self) -> bool {
        self.cpsr & (1 << 30) != 0
    }
    pub fn flag_c(&self) -> bool {
        self.cpsr & (1 << 29) != 0
    }
    pub fn flag_v(&self) -> bool {
        self.cpsr & (1 << 28) != 0
    }

    pub fn set_flags(&mut self, n: bool, z: bool, c: bool, v: bool) {
        self.cpsr &= 0x0FFF_FFFF;
        if n {
            self.cpsr |= 1 << 31;
        }
        if z {
            self.cpsr |= 1 << 30;
        }
        if c {
            self.cpsr |= 1 << 29;
        }
        if v {
            self.cpsr |= 1 << 28;
        }
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
            0b0000 => self.flag_z(),                                      // EQ
            0b0001 => !self.flag_z(),                                     // NE
            0b0010 => self.flag_c(),                                      // CS/HS
            0b0011 => !self.flag_c(),                                     // CC/LO
            0b0100 => self.flag_n(),                                      // MI
            0b0101 => !self.flag_n(),                                     // PL
            0b0110 => self.flag_v(),                                      // VS
            0b0111 => !self.flag_v(),                                     // VC
            0b1000 => self.flag_c() && !self.flag_z(),                    // HI
            0b1001 => !self.flag_c() || self.flag_z(),                    // LS
            0b1010 => self.flag_n() == self.flag_v(),                     // GE
            0b1011 => self.flag_n() != self.flag_v(),                     // LT
            0b1100 => !self.flag_z() && (self.flag_n() == self.flag_v()), // GT
            0b1101 => self.flag_z() || (self.flag_n() != self.flag_v()),  // LE
            0b1110 => true,                                               // AL (always)
            0b1111 => false,
            _ => false,
        }
    }

    /// Barrel shifter: LSL
    pub fn lsl(&mut self, value: u32, shift: u32) -> BarrelShiftResult {
        if shift == 0 {
            BarrelShiftResult {
                value,
                carry_out: self.flag_c(),
            }
        } else if shift < 32 {
            let carry_out = (value >> (32 - shift)) & 1 == 1;
            BarrelShiftResult {
                value: value << shift,
                carry_out,
            }
        } else if shift == 32 {
            BarrelShiftResult {
                value: 0,
                carry_out: value & 1 == 1,
            }
        } else {
            BarrelShiftResult {
                value: 0,
                carry_out: false,
            }
        }
    }

    /// Barrel shifter: LSR
    pub fn lsr(&mut self, value: u32, shift: u32) -> BarrelShiftResult {
        if shift == 0 {
            BarrelShiftResult {
                value,
                carry_out: self.flag_c(),
            }
        } else if shift < 32 {
            let carry_out = (value >> (shift - 1)) & 1 == 1;
            BarrelShiftResult {
                value: value >> shift,
                carry_out,
            }
        } else if shift == 32 {
            BarrelShiftResult {
                value: 0,
                carry_out: value >> 31 == 1,
            }
        } else {
            BarrelShiftResult {
                value: 0,
                carry_out: false,
            }
        }
    }

    /// Barrel shifter: ASR
    pub fn asr(&mut self, value: u32, shift: u32) -> BarrelShiftResult {
        if shift == 0 {
            BarrelShiftResult {
                value,
                carry_out: self.flag_c(),
            }
        } else if shift < 32 {
            let carry_out = (value >> (shift - 1)) & 1 == 1;
            let result = ((value as i32) >> shift) as u32;
            BarrelShiftResult {
                value: result,
                carry_out,
            }
        } else {
            let carry_out = value >> 31 == 1;
            let result = if carry_out { 0xFFFFFFFF } else { 0 };
            BarrelShiftResult {
                value: result,
                carry_out,
            }
        }
    }

    /// Barrel shifter: ROR
    pub fn ror(&mut self, value: u32, shift: u32) -> BarrelShiftResult {
        if shift == 0 {
            BarrelShiftResult {
                value,
                carry_out: self.flag_c(),
            }
        } else {
            let shift = shift % 32;
            if shift == 0 {
                BarrelShiftResult {
                    value,
                    carry_out: value >> 31 == 1,
                }
            } else {
                let result = value.rotate_right(shift);
                let carry_out = result >> 31 == 1;
                BarrelShiftResult {
                    value: result,
                    carry_out,
                }
            }
        }
    }

    /// Barrel shifter: RRX (rotate right extended)
    pub fn rrx(&mut self, value: u32) -> BarrelShiftResult {
        let old_carry = self.flag_c() as u32;
        let result = (old_carry << 31) | (value >> 1);
        let carry_out = value & 1 == 1;
        BarrelShiftResult {
            value: result,
            carry_out,
        }
    }

    /// Decode immediate operand for ARM data processing (8-bit value + 4-bit rotation)
    pub fn decode_imm(&mut self, instruction: u32) -> BarrelShiftResult {
        let rotate = ((instruction >> 8) & 0xF) * 2;
        let imm8 = instruction & 0xFF;
        if rotate == 0 {
            BarrelShiftResult {
                value: imm8,
                carry_out: self.flag_c(),
            }
        } else {
            let result = imm8.rotate_right(rotate);
            let carry_out = result >> 31 == 1;
            BarrelShiftResult {
                value: result,
                carry_out,
            }
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

            // Same PC + 12 rule as above: with a register-specified shift the
            // instruction takes an extra cycle and R15 reads four bytes further on.
            let rm_val = if shift_by_reg && rm == 15 {
                self.registers[15].wrapping_add(4)
            } else {
                self.registers[rm]
            };

            if shift_by_reg {
                // Register shift: shift amount comes from bits [11:8] (Rs)
                let rs = ((instruction >> 8) & 0xF) as usize;
                let shift_amount = self.registers[rs] & 0xFF;
                match shift_type {
                    0b00 => self.lsl(rm_val, shift_amount),
                    0b01 => self.lsr(rm_val, shift_amount),
                    0b10 => self.asr(rm_val, shift_amount),
                    0b11 => {
                        if shift_amount == 0 {
                            BarrelShiftResult {
                                value: rm_val,
                                carry_out: self.flag_c(),
                            }
                        } else if shift_amount.is_multiple_of(32) {
                            BarrelShiftResult {
                                value: rm_val,
                                carry_out: rm_val >> 31 == 1,
                            }
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
            self.branched = true;
        }
    }

    /// Read a register from the User bank regardless of the current mode.
    ///
    /// `LDM`/`STM` with the S bit set and R15 absent from the list transfer the
    /// User registers, which is how a privileged handler saves the interrupted
    /// task's context.
    pub fn user_reg(&self, reg: usize) -> u32 {
        match reg {
            8..=12 if self.mode() == Mode::Fiq => self.usr_r8_r12[reg - 8],
            13 | 14 if !matches!(self.mode(), Mode::User | Mode::System) => {
                self.usr_registers[reg - 13]
            }
            _ => self.registers[reg],
        }
    }

    /// Write a register in the User bank regardless of the current mode.
    pub fn set_user_reg(&mut self, reg: usize, value: u32) {
        match reg {
            8..=12 if self.mode() == Mode::Fiq => self.usr_r8_r12[reg - 8] = value,
            13 | 14 if !matches!(self.mode(), Mode::User | Mode::System) => {
                self.usr_registers[reg - 13] = value
            }
            _ => self.set_reg(reg, value),
        }
    }

    /// SWI handler - calls BIOS HLE
    pub fn swi(&mut self, comment: u32, bus: &mut super::memory::MemoryBus) {
        let back = if self.cpsr & 0x20 != 0 { 2 } else { 4 };
        // Software interrupt: call BIOS HLE
        super::bios::handle_swi(comment, self, bus);
        // The BIOS returns with `movs pc, lr`, a refill at the caller's next
        // instruction. Writing PC here charges it there, in the caller's
        // region, unless the HLE already sent PC somewhere else (SoftReset,
        // or IntrWait re-running its own SWI).
        if !self.branched {
            self.set_reg(15, self.registers[15].wrapping_sub(back));
        }
        // What the real BIOS's SWI epilogue (or SoftReset) leaves in the
        // BIOS read latch; the HLE runs no BIOS code to fetch it.
        bus.bios_latch = if comment == 0 {
            super::memory::BIOS_AFTER_BOOT
        } else {
            super::memory::BIOS_AFTER_SWI
        };
    }

    /// Handle IRQ exception.
    ///
    /// GBATEK, ARM CPU Exceptions: `LR_irq = return address + 4` (a pipeline
    /// artefact, applied whether the interrupted code was ARM or Thumb),
    /// `SPSR_irq = CPSR`, `T = 0` so the handler always runs in ARM state,
    /// `I = 1`, `F` unchanged, mode = IRQ. The canonical return is
    /// `SUBS PC, LR, #4`, which undoes the +4 and restores CPSR in one go.
    pub fn handle_irq(&mut self) {
        if self.cpsr & 0x80 != 0 {
            return;
        }
        let return_addr = self.registers[15];
        self.halted = false;

        let old_cpsr = self.cpsr;
        let mut new_cpsr = (old_cpsr & !0x3F) | Mode::Irq as u32;
        new_cpsr |= 0x80; // mask further IRQs
        new_cpsr &= !0x20; // the vector at 0x18 is ARM code
        self.set_cpsr(new_cpsr);

        // set_cpsr has switched to the IRQ bank, so this writes LR_irq.
        self.spsr_irq = old_cpsr;
        self.registers[14] = return_addr.wrapping_add(4);
        self.registers[15] = 0x0000_0018;
        self.fetch_n = false;
        // ARM7TDMI TRM: exception entry is 2S+1N, the pipeline refill at the
        // vector. Charged to the vector's first instruction.
        self.entry_cycles = 3;
    }
}
