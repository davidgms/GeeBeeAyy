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
            dma: Dma::new(),
            cycles: 0,
            run_frame_counter: 0,
        }
    }

    pub fn load_rom(&mut self, data: &[u8]) -> Result<(), cart::CartError> {
        // One owner: the bus. It is the only thing reachable from `read8` and
        // `store8`, which is where save accesses land.
        self.bus.cart = Cartridge::from_bytes(data)?;
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

    /// Route a captured sound-register byte write to the APU.
    ///
    /// The bus records byte writes; the APU decodes whole 16-bit registers, so
    /// every offset is folded onto its containing register and the full value
    /// re-read from I/O memory. The previous version matched a hand-listed set
    /// of offsets and silently dropped the rest - roughly half the sound
    /// registers, including `0x65` where the channel-1 trigger bit lives, and
    /// all of wave RAM.
    fn apu_sound_write(&mut self, offset: u32, _value: u8) {
        let base = match offset {
            0x60..=0x81 | 0x84..=0x85 | 0x90..=0x9F => offset & !1,
            // FIFO writes are byte streams, not registers.
            0xA0..=0xA3 => {
                self.apu.write_fifo_a(_value as i8);
                return;
            }
            0xA4..=0xA7 => {
                self.apu.write_fifo_b(_value as i8);
                return;
            }
            0x82..=0x83 => 0x82,
            _ => return,
        };
        let lo = self.bus.read8(0x0400_0000 + base) as u16;
        let hi = self.bus.read8(0x0400_0000 + base + 1) as u16;
        self.apu.write_register(base, lo | (hi << 8));
    }

    /// The cartridge's battery-backed save, or `None` if the cart has no save
    /// chip. Check [`Gba::take_save_dirty`] **before** calling this: reversed,
    /// a write landing between the two is lost.
    pub fn save_data(&self) -> Option<Vec<u8>> {
        self.bus.cart.save_data()
    }

    /// Restore a battery save previously produced by [`Gba::save_data`].
    pub fn load_save(&mut self, data: &[u8]) {
        self.bus.cart.load_save(data);
    }

    /// Whether save memory changed since this was last called, clearing the flag.
    pub fn take_save_dirty(&mut self) -> bool {
        self.bus.cart.take_save_dirty()
    }

    /// The cartridge, for save type and title.
    pub fn cartridge(&self) -> &Cartridge {
        &self.bus.cart
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
