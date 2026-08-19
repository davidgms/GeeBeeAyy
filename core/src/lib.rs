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
            self.cycles += cycles as u64;
            self.timer.tick(cycles as u32, &mut self.bus);
            self.ppu.tick(cycles as u32, &mut self.bus);
            self.apu.tick(cycles as u32);
            self.dma.tick(&mut self.bus);

            // Check for VBlank IRQ
            if self.ppu.vblank_pending() {
                self.io.request_interrupt(0x0001); // VBlank IRQ
            }

            // Handle interrupts
            if self.io.interrupt_pending() {
                self.cpu.handle_irq();
            }
        }
    }

    pub fn frame_buffer(&self) -> &[u8; 240 * 160 * 3] {
        self.ppu.frame_buffer()
    }
}
