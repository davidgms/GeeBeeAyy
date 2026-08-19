pub mod cpu;
pub mod ppu;
pub mod apu;
pub mod memory;
pub mod timer;
pub mod cart;
pub mod io;
pub mod dma;

use cpu::Cpu;
use ppu::Ppu;
use apu::Apu;
use memory::MemoryBus;
use timer::Timer;
use cart::Cartridge;
use io::IoHandler;
use dma::Dma;

const CYCLES_PER_FRAME: u64 = 280896; // ~59.73 Hz

pub struct Gba {
    pub cpu: Cpu,
    pub ppu: Ppu,
    pub apu: Apu,
    pub bus: MemoryBus,
    pub timer: Timer,
    pub cartridge: Cartridge,
    pub io: IoHandler,
    pub dma: Dma,
    pub cycles: u64,
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
            io: IoHandler::new(),
            dma: Dma::new(),
            cycles: 0,
        }
    }

    pub fn load_rom(&mut self, data: &[u8]) -> Result<(), cart::CartError> {
        self.cartridge = Cartridge::from_bytes(data)?;
        self.bus.load_rom(data);
        Ok(())
    }

    pub fn run_frame(&mut self) {
        let target = self.cycles + CYCLES_PER_FRAME;
        while self.cycles < target {
            let cycles = self.cpu.step(&mut self.bus);
            self.cycles += cycles as u32 as u64;
            self.timer.tick(cycles as u32, &mut self.bus);
            self.ppu.tick(cycles as u32, &mut self.bus, &mut self.dma);
            self.apu.tick(cycles as u32);

            // Tick prefetch buffer
            for _ in 0..cycles {
                self.bus.prefetch_tick();
            }

            // Process sound register writes from memory bus
            let writes = self.bus.drain_sound_writes();
            for (offset, value) in writes {
                self.apu_sound_write(offset, value);
            }

            // Check for interrupts
            if self.ppu.vblank_pending() {
                self.io.request_interrupt(0x0001); // VBlank IRQ
            }
            if self.ppu.hblank_pending() {
                self.io.request_interrupt(0x0002); // HBlank IRQ
            }

            if self.io.interrupt_pending() {
                self.cpu.handle_irq();
            }
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
            // FIFO A (0x040000A0 - 0x040000A3)
            0xA0 => self.apu.write_fifo_a(value as i8),
            0xA1 => self.apu.write_fifo_a(value as i8),
            0xA2 => self.apu.write_fifo_a(value as i8),
            0xA3 => self.apu.write_fifo_a(value as i8),
            // FIFO B (0x040000A4 - 0x040000A7)
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
}
