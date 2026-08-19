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
        let instruction = bus.read32(pc);
        self.registers[15] = pc + 8; // Pipeline: PC is always 2 ahead

        let is_thumb = self.cpsr & 0x20 != 0;

        let cycles = if is_thumb {
            self.execute_thumb(instruction as u16, bus)
        } else {
            self.execute_arm(instruction, bus)
        };

        cycles
    }

    fn execute_arm(&mut self, instruction: u32, bus: &mut super::memory::MemoryBus) -> u32 {
        // TODO: Full ARM instruction decoding
        let _ = instruction;
        let _ = bus;
        1
    }

    fn execute_thumb(&mut self, instruction: u16, bus: &mut super::memory::MemoryBus) -> u32 {
        // TODO: Full THUMB instruction decoding
        let _ = instruction;
        let _ = bus;
        1
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
}
