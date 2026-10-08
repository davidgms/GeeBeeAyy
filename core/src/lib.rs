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

/// Cycles between an interrupt source raising IF and the CPU taking the IRQ
/// at an instruction boundary. Fitted to the mGBA suite's timer-IRQ test,
/// whose expected values were recorded on hardware: a read four cycles after
/// a timer overflow is pre-empted by the IRQ, a read three cycles after is
/// not.
const IRQ_DELAY: u64 = 4;

pub struct Gba {
    pub cpu: Cpu,
    pub ppu: Ppu,
    pub apu: Apu,
    pub bus: MemoryBus,
    pub timer: Timer,
    pub dma: Dma,
    pub cycles: u64,
    pub run_frame_counter: u64,
    /// `cycles` when the current frame's first slice began; see
    /// `run_frame_slice`. Not machine state, so not in a save state.
    frame_start: u64,
    /// When the CPU's IRQ line (IE & IF with IME set) went high, for
    /// `IRQ_DELAY`. A few cycles of state, so not in a save state either.
    irq_since: Option<u64>,
    /// DMA cycles owed to the end of the next instruction: a transfer starts
    /// two cycles after the write that enabled it, which a one-cycle opcode
    /// fetch and the access after it get in ahead of. Transient.
    dma_stall: u32,
}

impl Default for Gba {
    fn default() -> Self {
        Self::new()
    }
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
            frame_start: 0,
            irq_since: None,
            dma_stall: 0,
        }
    }

    pub fn load_rom(&mut self, data: &[u8]) -> Result<(), cart::CartError> {
        // One owner: the bus. It is the only thing reachable from `read8` and
        // `store8`, which is where save accesses land.
        self.bus.cart = Cartridge::from_bytes(data)?;
        self.bus.load_rom(data);
        // A new cartridge is a power cycle. Booting the CPU alone left the
        // previous game's RAM, I/O, timers and DMA running under the new one.
        self.reset();
        Ok(())
    }

    /// Restart the loaded game, as the console's own reset would.
    ///
    /// The cart stays in the slot **and keeps its battery save** - a real GBA
    /// does not wipe SRAM on reset, and a player who resets to get past a
    /// crash must not lose their game doing it. Everything volatile goes back
    /// to power-on.
    pub fn reset(&mut self) {
        self.bus.reset();
        self.cpu = Cpu::new();
        self.cpu.boot();
        // Interframe blending is a frontend display setting, not machine
        // state, and the frontend may set it before the ROM is loaded.
        let blend = self.ppu.interframe_blend();
        self.ppu = Ppu::new();
        self.ppu.set_interframe_blend(blend);
        self.apu = Apu::new();
        self.timer = Timer::new();
        self.dma = Dma::new();
        self.cycles = 0;
        self.run_frame_counter = 0;
        self.frame_start = 0;
        self.irq_since = None;
        self.dma_stall = 0;
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
            //
            // A timer IRQ is an event too: stepping over it woke the CPU at
            // the PPU event after the overflow instead of on it.
            let step = self
                .timer
                .cycles_to_irq()
                .unwrap_or(u32::MAX)
                .min(self.ppu.cycles_to_next_event());
            // A serial transfer's end is a wake-up event as well.
            let step = match self.bus.sio_cycles {
                0 => step,
                sio => step.min(sio),
            };
            self.apply_dma_writes();
            self.apply_timer_writes();
            self.ppu.tick(step, &mut self.bus, &mut self.dma);
            // A transfer while the CPU is halted stops nothing that runs.
            self.dma.stall = 0;
            self.dma.stall_gamepak = None;
            self.dma_stall = 0;
            self.timer.tick(step, &mut self.bus);
            self.bus.tick_sio(step);
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
                // After `handle_irq`, which resets `entry_cycles`: on
                // hardware the BIOS epilogue runs once the handler returns,
                // and the total to the caller's next instruction is the same.
                if std::mem::take(&mut self.bus.io.halt_swi) {
                    self.cpu.entry_cycles += bios::HALT_RETURN_CYCLES;
                }
            }
            return (self.cycles - before) as u32;
        }

        let mut cycles = self.cpu.step(&mut self.bus) + std::mem::take(&mut self.dma_stall);
        // Before the PPU tick, so a channel enabled by this instruction is
        // configured in time for an HBlank or VBlank that lands in the same
        // step.
        self.apply_dma_writes();
        // The CPU stops while a DMA owns the bus (GBATEK, DMA Transfers):
        // this instruction's transfers, and the HBlank, VBlank and sound ones
        // the last step's ticks ran. A transfer starts two cycles after it
        // is enabled ("wait 2 clock cycles"), so when the next opcode fetch
        // takes one cycle that instruction's first access goes before it,
        // and the stall lands after that instruction instead (mGBA suite,
        // DMA timing, the IWRAM and prefetched columns).
        let mut stall = std::mem::take(&mut self.dma.stall);
        if let Some(lead) = self.dma.stall_gamepak.take() {
            // It takes the GamePak bus from the prefetch buffer.
            stall += self.cpu.prefetch_handover(lead);
        }
        if stall != 0 {
            if self.cpu.next_fetch_cycles(&self.bus) < 2 {
                self.dma_stall = stall;
            } else {
                cycles += stall;
            }
        }
        self.cycles += cycles as u64;
        // Timer writes land *after* the tick. A store writes in its last
        // cycle, so none of this instruction's cycles belong to a timer it
        // starts, and an overflow in that last cycle still reloads the old
        // value (mGBA suite, timer-IRQ test).
        let timer_irq = self.timer.tick(cycles, &mut self.bus);
        self.apply_timer_writes();
        // Same order as the timers: a transfer started by this instruction
        // owns none of its cycles.
        let mut sio_irq = None;
        if self.bus.sio_cycles != 0 || self.bus.sio_written {
            sio_irq = self.bus.tick_sio(cycles);
            if self.bus.sio_written {
                self.bus.apply_sio_write();
            }
        }
        self.ppu.tick(cycles, &mut self.bus, &mut self.dma);
        self.apu.tick(cycles);

        self.post_tick();

        // Deliver IRQs to CPU, `IRQ_DELAY` cycles after the line went high.
        if self.bus.io.interrupt_pending() {
            let raised = match (timer_irq, sio_irq) {
                (Some(a), Some(b)) => before + u64::from(a.min(b)),
                (Some(at), None) | (None, Some(at)) => before + u64::from(at),
                (None, None) => self.cycles,
            };
            let since = *self.irq_since.get_or_insert(raised);
            if self.cycles >= since + IRQ_DELAY {
                self.cpu.handle_irq();
            }
        } else {
            self.irq_since = None;
        }

        (self.cycles - before) as u32
    }

    pub fn run_frame(&mut self) {
        self.run_frame_slice(0, 1);
    }

    /// Run slice `index` of a frame cut into `count` equal parts, so the
    /// frontend can push fresh keys between slices and the game sees input
    /// mid-frame (NanoBoyAdvance polls four times a frame).
    ///
    /// Call it for `index` in `0..count`, in order. Slice 0 marks the frame
    /// start and the last slice stops on the same target `run_frame` uses, so
    /// a sliced frame is cycle-for-cycle the same emulation as a whole one:
    /// both are "step while `cycles` < target", only paused in between.
    /// `count` of 0 is treated as 1 and an out-of-range `index` as the last.
    pub fn run_frame_slice(&mut self, index: u32, count: u32) {
        let count = count.max(1);
        let index = index.min(count - 1);
        if index == 0 {
            self.frame_start = self.cycles;
        }
        let target = self.frame_start + CYCLES_PER_FRAME * u64::from(index + 1) / u64::from(count);
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
        // Runs every step and is almost always empty; draining an empty Vec
        // is not free.
        if self.bus.dma_writes.is_empty() {
            return;
        }
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
                // The low bits a unit cannot address are dropped (mGBA suite,
                // Memory tests, unaligned SRAM DMA; the 8-bit SRAM bus is the
                // only place the bus would otherwise still see them).
                let align = if control & 0x0400 != 0 { !3 } else { !1 };
                self.dma.write_sad(ch, sad & src_mask & align);
                self.dma.write_dad(ch, dad & dst_mask & align);
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
        if self.bus.timer_writes.is_empty() {
            return;
        }
        for (timer, is_control, value) in self.bus.drain_timer_writes() {
            if is_control {
                let was_enabled = self.timer.enabled[timer];
                self.timer.set_control(timer, value);
                if !was_enabled && self.timer.enabled[timer] {
                    self.timer.delay_start(timer, 1);
                    self.timer.align_start(timer, self.cycles);
                }
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
        let counters = self.timer.counters(self.cpu.next_fetch_cycles(&self.bus));
        let regs = &mut self.bus.io_regs_data_mut()[0x100..0x110];
        for (reg, c) in regs.chunks_exact_mut(4).zip(counters) {
            reg[..2].copy_from_slice(&c.to_le_bytes());
        }

        if !self.bus.sound_writes.is_empty() {
            for (offset, value) in self.bus.drain_sound_writes() {
                self.apu_sound_write(offset, value);
            }
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
        // The latched value: the APU needs the write-only bits (frequency,
        // length, trigger) that a CPU read masks off.
        let value = self.bus.io_read16(0x0400_0000 + base);
        self.apu.write_register(base, value);
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
        state.restore(self)?;
        // A restored `cycles` from another frame would make a following
        // non-zero slice run far too long or not at all.
        self.frame_start = self.cycles;
        self.irq_since = None;
        Ok(())
    }

    /// Run `count` frames, drawing only the last one.
    ///
    /// This is the fast-forward path, and the frames in between are never
    /// shown: drawing them costs as much as the one that is, for a picture
    /// that is overwritten microseconds later. Everything else - CPU, DMA,
    /// timers, interrupts, audio - runs exactly as it would frame by frame,
    /// so nothing about the emulation changes, only what reaches the frame
    /// buffer.
    /// Copy emulated memory into `out`, for an observer that must not disturb
    /// the machine.
    ///
    /// Returns how many bytes were readable. Written for achievement
    /// evaluation, which walks a handful of addresses once a frame; see
    /// `docs/achievements.md`. Only work RAM, internal work RAM and save
    /// memory are visible, and nothing here can write.
    pub fn peek_memory(&self, address: u32, out: &mut [u8]) -> usize {
        for (i, byte) in out.iter_mut().enumerate() {
            // Wrapping, not saturating: an address that runs off the end of a
            // region lands outside every readable range and reads 0, which is
            // what a caller asking for too much should get.
            *byte = self.bus.peek(address.wrapping_add(i as u32));
        }
        out.len()
    }

    pub fn run_frames(&mut self, count: u32) {
        for frame in 0..count {
            self.ppu.set_render_enabled(frame + 1 == count);
            self.run_frame();
        }
        self.ppu.set_render_enabled(true);
    }
}
