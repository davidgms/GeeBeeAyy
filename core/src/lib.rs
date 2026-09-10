pub mod apu;
pub mod bios;
pub mod cart;
pub mod cpu;
pub mod dma;
pub mod ffi;
pub mod io;
pub mod memory;
pub mod ppu;
pub mod rewind;
pub mod savestate;
pub mod timer;

use apu::Apu;
use cart::Cartridge;
use cpu::Cpu;
use dma::Dma;
use memory::MemoryBus;
use ppu::Ppu;
use timer::Timer;

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
            // Advance to the next PPU event, never past it. A whole
            // scanline in one tick steps over HBlank, so a game halted with
            // the HBlank IRQ enabled - and that is most of every frame - was
            // getting one interrupt a frame instead of 228.
            let step = self.ppu.cycles_to_next_event();
            self.apply_dma_writes();
            self.apply_timer_writes();
            self.ppu.tick(step, &mut self.bus, &mut self.dma);
            self.timer.tick(step, &mut self.bus);
            self.apu.tick(step);
            self.cycles += step as u64;

            // The hardware does not stop for HALT, so neither can this. The
            // sound FIFOs, the timer overflows that drain them and the PPU's
            // interrupt flags all have to keep moving, and a game spends most
            // of every frame halted: skipping this block here left the FIFOs
            // starved except during the brief run between interrupts, which
            // is audible as a thump once a frame instead of music.
            self.post_tick();

            // GBATEK: HALT ends when an *enabled* interrupt occurs, judged on
            // IE & IF alone. IME gates whether the CPU jumps to the handler,
            // not whether it wakes, so a game that halts with IME clear and
            // polls IF still resumes.
            if self.bus.io.ie & self.bus.io.if_ != 0 {
                self.cpu.halted = false;
                self.bus.io.halt = false;
                if self.bus.io.interrupt_pending() {
                    self.cpu.handle_irq();
                }
            }
            return (self.cycles - before) as u32;
        }

        let cycles = self.cpu.step(&mut self.bus);
        self.cycles += cycles as u32 as u64;
        // Before the PPU tick, so a channel enabled by this instruction is
        // configured in time for an HBlank or VBlank that lands in the same
        // step.
        self.apply_dma_writes();
        self.apply_timer_writes();
        self.timer.tick(cycles as u32, &mut self.bus);
        self.ppu.tick(cycles as u32, &mut self.bus, &mut self.dma);
        self.apu.tick(cycles as u32);

        self.post_tick();

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

    /// Apply the DMA register writes the bus captured this step.
    ///
    /// The bus records the channel rather than acting on it, because an
    /// immediate transfer runs inside `Dma::write_control` and needs
    /// `&mut MemoryBus` - the borrow the store is already holding. Same shape
    /// as `sound_writes`, and the reason `write_sad`/`write_dad`/`write_count`/
    /// `write_control` had no caller at all until now: nothing could reach
    /// both halves.
    fn apply_dma_writes(&mut self) {
        for ch in self.bus.drain_dma_writes() {
            let base = 0xB0 + ch * 12;
            let regs = self.bus.io_regs_data();
            let word = |o: usize| u32::from_le_bytes(regs[o..o + 4].try_into().unwrap());
            let half = |o: usize| u16::from_le_bytes(regs[o..o + 2].try_into().unwrap());
            let (sad, dad, count, control) =
                (word(base), word(base + 4), half(base + 8), half(base + 10));

            // SAD, DAD and CNT_L latch into the channel's internal registers
            // on the enable edge only. A channel that is already running keeps
            // the addresses it has advanced to, so rewriting CNT_H - to change
            // the IRQ bit, say - must not rewind it.
            if control & 0x8000 != 0 && !self.dma.channels[ch].enabled {
                // GBATEK: DMA0 addresses internal memory only (27 bits); only
                // DMA3's destination reaches the gamepak (28 bits).
                let src_mask = if ch == 0 { 0x07FF_FFFF } else { 0x0FFF_FFFF };
                let dst_mask = if ch == 3 { 0x0FFF_FFFF } else { 0x07FF_FFFF };
                self.dma.write_sad(ch, sad & src_mask);
                self.dma.write_dad(ch, dad & dst_mask);
                self.dma.write_count(ch, count);
            }
            self.dma.write_control(ch, control, &mut self.bus);
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
    /// Move the PPU's pending VBlank/HBlank flags into IF.
    ///
    /// Called from both the running and the halted path: a halted CPU is
    /// woken by IF, so skipping this while halted deadlocks the machine.
    /// Work that follows every tick of the machine, whether the CPU executed
    /// an instruction or was halted: refill the sound FIFOs, hand the timers'
    /// overflows to the APU, apply queued sound-register writes, and route the
    /// PPU's pending interrupts into IF.
    /// Hand the timer register writes the bus queued to the timer unit.
    /// A write to `TMxCNT_L` is a reload, one to `TMxCNT_H` is control.
    fn apply_timer_writes(&mut self) {
        for (timer, is_control, value) in self.bus.drain_timer_writes() {
            if is_control {
                self.timer.set_control(timer, value);
            } else {
                self.timer.set_reload(timer, value);
            }
        }
    }

    fn post_tick(&mut self) {
        // DMA Sound. Either channel can feed either FIFO - the destination
        // register decides, not the channel number. Keying FIFO A to DMA1 and
        // B to DMA2 dropped the data of any game that wired them the other way
        // round, *after* `do_sound_transfer` had already advanced the source.
        for ch in [1usize, 2] {
            let wanted = match self.dma.channels[ch].dest {
                0x0400_00A0 => self.apu.fifo_a_half_empty(),
                0x0400_00A4 => self.apu.fifo_b_half_empty(),
                _ => false,
            };
            if !wanted {
                continue;
            }
            if let Some((dest, data)) = self.dma.do_sound_transfer(ch, &mut self.bus) {
                for &byte in &data {
                    if dest == 0x0400_00A0 {
                        self.apu.write_fifo_a(byte as i8);
                    } else {
                        self.apu.write_fifo_b(byte as i8);
                    }
                }
            }
        }

        let overflows = self.timer.drain_overflows();
        for (i, &count) in overflows.iter().enumerate() {
            for _ in 0..count {
                self.apu.on_timer_overflow(i as u8);
            }
        }
        // TMxCNT_L reads the live counter, not the reload the game wrote
        // there. Nothing published it before, so a game polling a timer saw
        // its own reload value forever.
        for (i, &c) in self.timer.counters().iter().enumerate() {
            let base = 0x100 + i * 4;
            let regs = self.bus.io_regs_data_mut();
            regs[base] = c as u8;
            regs[base + 1] = (c >> 8) as u8;
        }

        let writes = self.bus.drain_sound_writes();
        for (offset, value) in writes {
            self.apu_sound_write(offset, value);
        }

        self.route_ppu_interrupts();
    }

    fn route_ppu_interrupts(&mut self) {
        if self.ppu.vblank_pending() {
            self.bus.io.request_interrupt(0x0001);
        }
        if self.ppu.hblank_pending() {
            self.bus.io.request_interrupt(0x0002);
        }
        if self.ppu.vcount_pending() {
            self.bus.io.request_interrupt(0x0004);
        }
    }

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

    /// Average each finished frame with the one before it, the way the GBA's
    /// slow LCD did. Games that fake transparency by alternating what they
    /// draw every other frame - Yggdra Union's "SAVE DATA" title, for one -
    /// flicker hard on a modern panel without it.
    pub fn set_interframe_blend(&mut self, on: bool) {
        self.ppu.set_interframe_blend(on);
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

    pub fn load_state(
        &mut self,
        state: &savestate::SaveState,
    ) -> Result<(), savestate::SaveStateError> {
        state.restore(self)
    }

    /// Run `count` frames, drawing only the last one.
    ///
    /// This is the fast-forward path, and the frames in between are never
    /// shown: drawing them costs as much as the one that is, for a picture
    /// that is overwritten microseconds later. Everything else - CPU, DMA,
    /// timers, interrupts, audio - runs exactly as it would frame by frame,
    /// so nothing about the emulation changes, only what reaches the frame
    /// buffer.
    pub fn run_frames(&mut self, count: u32) {
        for frame in 0..count {
            self.ppu.set_render_enabled(frame + 1 == count);
            self.run_frame();
        }
        self.ppu.set_render_enabled(true);
    }
}
