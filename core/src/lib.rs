pub mod cpu;
pub mod ppu;
pub mod apu;
pub mod memory;
pub mod timer;
pub mod cart;
pub mod io;
pub mod dma;
pub mod savestate;
pub mod bios;
pub mod ffi;

use cpu::Cpu;
use ppu::Ppu;
use apu::Apu;
use memory::MemoryBus;
use timer::Timer;
use cart::Cartridge;
use dma::Dma;

const CYCLES_PER_FRAME: u64 = 280896;

pub struct Gba {
    pub cpu: Cpu,
    pub ppu: Ppu,
    pub apu: Apu,
    pub bus: MemoryBus,
    pub timer: Timer,
    pub cartridge: Cartridge,
    pub dma: Dma,
    pub cycles: u64,
    pub run_frame_counter: u64,
}

impl Gba {
    pub fn new() -> Self {
        Self {
            cpu: Cpu::new(),
            ppu: Ppu::new(),
            apu: Apu::new(),
            bus: MemoryBus::new(),
            timer: Timer::new(),
            cartridge: Cartridge::empty(),
            dma: Dma::new(),
            cycles: 0,
            run_frame_counter: 0,
        }
    }

    pub fn load_rom(&mut self, data: &[u8]) -> Result<(), cart::CartError> {
        self.cartridge = Cartridge::from_bytes(data)?;
        self.bus.load_rom(data);
        self.cpu.boot();
        Ok(())
    }

    /// Advance the whole machine by one CPU instruction.
    ///
    /// Ticks the timers, PPU, APU and DMA with the cycles that instruction
    /// consumed, and delivers any interrupt it raised. `run_frame` is this in
    /// a loop; a test or debugger that steps `cpu` directly instead will stall
    /// forever on a ROM that polls DISPSTAT for VBlank.
    pub fn step(&mut self) -> u32 {
        let before = self.cycles;
        // If halted, just advance PPU until an interrupt wakes us
        if self.cpu.halted || self.bus.io.halt {
            // Advance one scanline at a time so a HALT cannot outrun the PPU
            // that has to wake it.
            const SCANLINE: u32 = 1232;
            self.ppu.tick(SCANLINE, &mut self.bus, &mut self.dma);
            self.timer.tick(SCANLINE, &mut self.bus);
            self.apu.tick(SCANLINE);
            self.cycles += SCANLINE as u64;

            if self.bus.io.interrupt_pending() {
                self.cpu.halted = false;
                self.bus.io.halt = false;
                self.cpu.handle_irq();
            }
            return (self.cycles - before) as u32;
        }

        let cycles = self.cpu.step(&mut self.bus);
        self.cycles += cycles as u32 as u64;
        self.timer.tick(cycles as u32, &mut self.bus);
        self.ppu.tick(cycles as u32, &mut self.bus, &mut self.dma);
        self.apu.tick(cycles as u32);

        for _ in 0..cycles {
            self.bus.prefetch_tick();
        }

        // DMA Sound
        if self.apu.fifo_a_half_empty() {
            if let Some((dest, data)) = self.dma.do_sound_transfer(1, &mut self.bus) {
                if dest == 0x0400_00A0 {
                    for &byte in &data { self.apu.write_fifo_a(byte as i8); }
                }
            }
        }
        if self.apu.fifo_b_half_empty() {
            if let Some((dest, data)) = self.dma.do_sound_transfer(2, &mut self.bus) {
                if dest == 0x0400_00A4 {
                    for &byte in &data { self.apu.write_fifo_b(byte as i8); }
                }
            }
        }

        let overflows = self.timer.drain_overflows();
        for (i, &overflow) in overflows.iter().enumerate() {
            if overflow { self.apu.on_timer_overflow(i as u8); }
        }

        let writes = self.bus.drain_sound_writes();
        for (offset, value) in writes {
            self.apu_sound_write(offset, value);
        }

        // Route interrupts from PPU -> bus.io
        if self.ppu.vblank_pending() {
            self.bus.io.request_interrupt(0x0001);
        }
        if self.ppu.hblank_pending() {
            self.bus.io.request_interrupt(0x0002);
        }

        // Deliver IRQs to CPU
        if self.bus.io.interrupt_pending() {
            self.cpu.handle_irq();
        }

        (self.cycles - before) as u32
    }

    pub fn run_frame(&mut self) {
        let target = self.cycles + CYCLES_PER_FRAME;
        while self.cycles < target {
            self.step();
        }
    }

    fn apu_sound_write(&mut self, offset: u32, value: u8) {
        match offset {
            0x60 => self.apu.write_sound1_reg(0x60, value),
            0x62 => self.apu.write_sound1_reg(0x62, value),
            0x63 => self.apu.write_sound1_reg(0x63, value),
            0x64 => self.apu.write_sound1_reg(0x64, value),
            0x68 => self.apu.write_sound2_reg(0x68, value),
            0x6C => self.apu.write_sound2_reg(0x6C, value),
            0x6D => self.apu.write_sound2_reg(0x6D, value),
            0x6E => self.apu.write_sound2_reg(0x6E, value),
            0x70 => self.apu.write_sound3_reg(0x70, value),
            0x72 => self.apu.write_sound3_reg(0x72, value),
            0x74 => self.apu.write_sound3_reg(0x74, value),
            0x78 => self.apu.write_sound4_reg(0x78, value),
            0x7A => self.apu.write_sound4_reg(0x7A, value),
            0x7C => self.apu.write_sound4_reg(0x7C, value),
            0x7E => self.apu.write_sound4_reg(0x7E, value),
            0x80 => {
                let hi = self.bus.read8(0x0400_0081);
                self.apu.write_soundcnt_l(((hi as u16) << 8) | value as u16);
            }
            0x81 => {
                let lo = self.bus.read8(0x0400_0080);
                self.apu.write_soundcnt_l(((value as u16) << 8) | lo as u16);
            }
            0x82 => {
                let hi = self.bus.read8(0x0400_0083);
                self.apu.write_soundcnt_h(((hi as u16) << 8) | value as u16);
            }
            0x83 => {
                let lo = self.bus.read8(0x0400_0082);
                self.apu.write_soundcnt_h(((value as u16) << 8) | lo as u16);
            }
            0xA0 => self.apu.write_fifo_a(value as i8),
            0xA1 => self.apu.write_fifo_a(value as i8),
            0xA2 => self.apu.write_fifo_a(value as i8),
            0xA3 => self.apu.write_fifo_a(value as i8),
            0xA4 => self.apu.write_fifo_b(value as i8),
            0xA5 => self.apu.write_fifo_b(value as i8),
            0xA6 => self.apu.write_fifo_b(value as i8),
            0xA7 => self.apu.write_fifo_b(value as i8),
            _ => {}
        }
    }

    pub fn frame_buffer(&self) -> &[u8; 240 * 160 * 3] {
        self.ppu.frame_buffer()
    }

    pub fn apu_samples(&self) -> &[f32] {
        self.apu.samples()
    }

    pub fn clear_audio_buffer(&mut self) {
        self.apu.clear_buffer();
    }

    pub fn save_state(&self) -> savestate::SaveState {
        savestate::SaveState::create(self)
    }

    pub fn load_state(&mut self, state: &savestate::SaveState) -> Result<(), savestate::SaveStateError> {
        state.restore(self)
    }

    pub fn run_frames(&mut self, count: u32) {
        for _ in 0..count {
            self.run_frame();
        }
    }
}
