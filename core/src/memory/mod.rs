#[derive(Clone, Copy, PartialEq, Eq)]
enum SioMode {
    Normal8,
    Normal32,
    Multi,
    Uart,
    Gpio,
    Joy,
}

pub struct MemoryBus {
    ewram: Vec<u8>,
    iwram: Vec<u8>,
    io_regs: Vec<u8>,
    palette: Vec<u8>,
    vram: Vec<u8>,
    oam: Vec<u8>,
    bios: Vec<u8>,
    rom: Vec<u8>,
    /// The cartridge lives here because the bus is the only thing that can
    /// reach it during execution: `read8`/`store8` see a `&mut MemoryBus` and
    /// nothing else. It used to hang off `Gba`, where nothing on the memory
    /// path could call it, so save memory was unreachable.
    pub cart: super::cart::Cartridge,
    pub waitcnt: u16,
    pub sound_writes: Vec<(u32, u8)>,
    /// Channels whose DMAxCNT_H was written since `Gba::step` last looked.
    /// The bus cannot apply them itself: an immediate transfer runs inside
    /// `Dma::write_control` and needs `&mut MemoryBus`, which is the borrow
    /// the store is already holding. Same shape as `sound_writes`.
    pub(crate) dma_writes: Vec<usize>,
    /// Timer register writes, same shape again. `Timer::set_control` and
    /// `set_reload` had no caller outside the tests for the project's whole
    /// history, so no timer a game started ever ran: no timer interrupt, and
    /// no DMA sound, which is driven entirely by timer overflows.
    pub(crate) timer_writes: Vec<(usize, bool, u16)>,
    pub io: super::io::IoHandler,
    /// Address and state of the instruction executing now, set by
    /// `Cpu::step`, so a read of nothing can return the open bus.
    pub(crate) exec_pc: u32,
    pub(crate) exec_thumb: bool,
    /// Set while [Self::open_bus] is fetching the prefetched opcode. That
    /// fetch reads from the PC, and a PC that is itself in open-bus memory
    /// would re-enter forever: it overflowed the stack on a phone.
    open_bus_busy: std::cell::Cell<bool>,
    /// The most recent opcode the CPU fetched from the BIOS, which is what a
    /// BIOS read returns while the CPU runs anywhere else (GBATEK, BIOS ROM
    /// read protection). `Cpu::step` refreshes it whenever it executes in
    /// the BIOS, and the HLE SWI path sets the value a real SWI leaves.
    // ponytail: not in save states; the next SWI or IRQ (every frame in a
    // game) restores it. Save it if a game is found reading the BIOS.
    pub(crate) bios_latch: u32,
    /// Cycles left in a normal-mode serial transfer, 0 when none is running.
    pub sio_cycles: u32,
    /// SIOCNT was written since `Gba::step` last looked; see
    /// [`Self::apply_sio_write`].
    pub(crate) sio_written: bool,
}

/// The BIOS read latch after boot or SoftReset: the opcode at [0x0DC+8].
pub(crate) const BIOS_AFTER_BOOT: u32 = 0xE129_F000;
/// ... after a SWI returns: [0x188+8].
pub(crate) const BIOS_AFTER_SWI: u32 = 0xE3A0_2004;
/// ... after an IRQ returns: [0x13C+8].
const BIOS_AFTER_IRQ: u32 = 0xE55E_C002;

impl Default for MemoryBus {
    fn default() -> Self {
        Self::new()
    }
}

impl MemoryBus {
    pub fn new() -> Self {
        let mut bus = Self {
            ewram: vec![0; 256 * 1024],
            iwram: vec![0; 32 * 1024],
            io_regs: vec![0; 0x400],
            palette: vec![0; 1024],
            vram: vec![0; 96 * 1024],
            oam: vec![0; 1024],
            bios: vec![0; 16 * 1024],
            rom: Vec::new(),
            cart: super::cart::Cartridge::empty(),
            waitcnt: 0,
            sound_writes: Vec::new(),
            dma_writes: Vec::new(),
            timer_writes: Vec::new(),
            io: super::io::IoHandler::new(),
            exec_pc: 0,
            exec_thumb: false,
            open_bus_busy: std::cell::Cell::new(false),
            bios_latch: 0,
            sio_cycles: 0,
            sio_written: false,
        };
        bus.init_bios();
        // KEYINPUT is active low ("0=Pressed, 1=Released", GBATEK Keypad Input),
        // so a zero-filled io_regs would read as all ten buttons held forever.
        bus.set_keys(0);
        bus
    }

    /// Put the machine back to power-on, keeping the cartridge in the slot.
    ///
    /// Everything volatile is cleared: both work RAMs, VRAM, palette, OAM and
    /// the I/O registers. **The cartridge's save memory is not**, because a
    /// real GBA keeps it across a reset - that is the whole point of a battery
    /// save, and wiping it here would lose a player's game.
    ///
    /// The ROM stays too: a reset is not an eject.
    pub fn reset(&mut self) {
        self.ewram.fill(0);
        self.iwram.fill(0);
        self.io_regs.fill(0);
        self.palette.fill(0);
        self.vram.fill(0);
        self.oam.fill(0);
        self.waitcnt = 0;
        self.sound_writes.clear();
        self.dma_writes.clear();
        self.timer_writes.clear();
        self.sio_cycles = 0;
        self.sio_written = false;
        self.io = super::io::IoHandler::new();
        self.init_bios();
        self.set_keys(0);
    }

    /// Update KEYINPUT from a frontend key bitmask.
    ///
    /// `keys` uses the GBATEK bit order (0=A, 1=B, 2=Select, 3=Start, 4=Right,
    /// 5=Left, 6=Up, 7=Down, 8=R, 9=L) with **1 = pressed**, which is the
    /// natural polarity for a caller. The inversion to the hardware's active-low
    /// KEYINPUT happens here and nowhere else.
    pub fn set_keys(&mut self, keys: u16) {
        let raw = !keys & 0x03FF;
        self.io_regs[super::io::IO_KEYINPUT] = (raw & 0xFF) as u8;
        self.io_regs[super::io::IO_KEYINPUT + 1] = (raw >> 8) as u8;
    }

    /// Populate BIOS memory with the IRQ handler the real BIOS has at 0x18.
    fn init_bios(&mut self) {
        // The real BIOS IRQ path, instruction for instruction (GBATEK, BIOS
        // Interrupt handling):
        //
        //   0x018: b     0x128
        //   0x128: stmfd sp!, {r0-r3, r12, lr}
        //   0x12C: mov   r0, #0x04000000
        //   0x130: add   lr, pc, #0          ; lr = 0x138
        //   0x134: ldr   pc, [r0, #-4]       ; [0x03FFFFFC] = the game's handler
        //   0x138: ldmfd sp!, {r0-r3, r12, lr}
        //   0x13C: subs  pc, lr, #4          ; return from IRQ, restoring CPSR
        //
        // Copying it exactly matters twice over: handlers may rely on r0
        // holding the I/O base, and the mGBA suite times this path to the
        // cycle. An earlier `blx r0` stub reached the handler six cycles early.
        // `ldr pc` does not interwork, so the handler must be ARM code - as on
        // hardware.
        //
        // What the BIOS does **not** do is the point too: it neither
        // acknowledges IF nor touches the IntrWait flags at 0x03007FF8; both
        // are the game handler's job. A stub that acknowledged IF first broke
        // every game whose handler dispatches on `IE & IF`: Yggdra Union's
        // IWRAM dispatcher read zero, fell through its whole chain and called
        // the wrong handler for every interrupt.
        self.bios[0x18..0x1C].copy_from_slice(&0xEA00_0042u32.to_le_bytes()); // b 0x128
        let handler: [u32; 6] = [
            0xE92D_500F, // stmfd sp!, {r0-r3, r12, lr}
            0xE3A0_0301, // mov r0, #0x04000000
            0xE28F_E000, // add lr, pc, #0
            0xE510_F004, // ldr pc, [r0, #-4]
            0xE8BD_500F, // ldmfd sp!, {r0-r3, r12, lr}
            0xE25E_F004, // subs pc, lr, #4
        ];
        for (i, &word) in handler.iter().enumerate() {
            let addr = 0x128 + i * 4;
            self.bios[addr..addr + 4].copy_from_slice(&word.to_le_bytes());
        }

        // The previous stub's epilogue, kept at the address it lived at.
        // Save states carry no BIOS memory, and one taken inside a game IRQ
        // handler under that stub holds LR = 0x28. Without these two words
        // the handler returned into zeroed BIOS, slid down to 0x128 and
        // re-entered the stub forever (Yggdra Union: 4x the work per frame).
        self.bios[0x28..0x2C].copy_from_slice(&handler[4].to_le_bytes()); // ldmfd
        self.bios[0x2C..0x30].copy_from_slice(&handler[5].to_le_bytes()); // subs pc

        // The real BIOS word at 0x144, never executed here: it is the
        // prefetch of `subs pc` at 0x13C, so the BIOS read latch holds it
        // after every IRQ (GBATEK, BIOS ROM: "[013Ch+8] after IRQ").
        self.bios[0x144..0x148].copy_from_slice(&BIOS_AFTER_IRQ.to_le_bytes());
        self.bios_latch = BIOS_AFTER_BOOT;
    }

    /// VRAM mirrors in 128 KB steps, but the region is 96 KB: the upper 32 KB
    /// is itself a mirror of the block before it, so 0x18000..0x20000 folds
    /// back onto 0x10000..0x18000.
    fn vram_offset(address: u32) -> usize {
        let offset = (address & 0x1_FFFF) as usize;
        if offset >= 0x18000 {
            offset - 0x8000
        } else {
            offset
        }
    }

    /// Read a byte for an observer, never for the emulated CPU.
    ///
    /// Achievement evaluation walks memory once a frame, and it must not be
    /// able to change what the game sees. [`read8`](Self::read8) cannot be
    /// used for that: its first act is to answer the EEPROM window with a
    /// "chip ready" 1, which is right for a game polling after a write and
    /// wrong for anything that is only looking.
    ///
    /// Only the three regions an achievement runtime asks a GBA for are
    /// visible - work RAM, internal work RAM and save memory. Everything else
    /// reads 0, because an achievement has no business in VRAM, the I/O
    /// registers or the cartridge ROM, and a peek that could reach them is a
    /// bigger hole than the feature is worth.
    pub fn peek(&self, address: u32) -> u8 {
        match address {
            0x0200_0000..=0x0203_FFFF => self.ewram[(address & 0x3_FFFF) as usize],
            0x0300_0000..=0x0300_7FFF => self.iwram[(address & 0x7FFF) as usize],
            // `save_read` takes `&self` and changes nothing, which is what
            // makes it safe here. Flash in ID mode answers with its identity
            // rather than data, exactly as the CPU would see it.
            0x0E00_0000..=0x0E00_FFFF => self.cart.save_read(address),
            _ => 0,
        }
    }

    pub fn read8(&self, address: u32) -> u8 {
        // A serial EEPROM cannot be read by the CPU - GBATEK requires DMA to
        // clock data out - but the standard save library still polls this
        // window with plain loads after a write, spinning until bit 0 comes
        // back 1 to say the chip has finished programming. Without this the
        // poll read the ROM mirror underneath instead: at 0x0DFFFF00 that is
        // 0xFF80, bit 0 clear, so the game waited for a chip that was never
        // going to answer and gave up with "Save failed!". Programming here
        // is instant, so the chip is always ready. The check sits in the
        // cartridge arm below, where the window lives, to keep it off every
        // other read.
        match address {
            0x0000_0000..=0x0000_3FFF if self.exec_pc < 0x4000 => {
                self.bios[(address & 0x3FFF) as usize]
            }
            0x0000_0000..=0x0000_3FFF => (self.bios_latch >> ((address & 3) * 8)) as u8,
            0x0200_0000..=0x02FF_FFFF => self.ewram[(address & 0x3_FFFF) as usize],
            0x0300_0000..=0x03FF_FFFF => self.iwram[(address & 0x7FFF) as usize],
            0x0400_0000..=0x04FF_FFFF => {
                let offset = (address & 0xFF_FFFF) as usize;
                if offset >= 0x400 {
                    return self.open_bus_byte(address);
                }
                match super::io::read_rule(offset & !1) {
                    super::io::IoRead::Mask(mask) => {
                        self.io_byte(offset) & (mask >> ((offset & 1) * 8)) as u8
                    }
                    super::io::IoRead::OpenBus => self.open_bus_byte(address),
                }
            }
            0x0500_0000..=0x05FF_FFFF => self.palette[(address & 0x3FF) as usize],
            0x0600_0000..=0x06FF_FFFF => self.vram[Self::vram_offset(address)],
            0x0700_0000..=0x07FF_FFFF => self.oam[(address & 0x3FF) as usize],
            // The cartridge appears three times, once per wait-state region:
            // 0x08 (WS0), 0x0A (WS1) and 0x0C (WS2). Same data, different
            // access timing.
            0x0800_0000..=0x0DFF_FFFF => {
                // `eeprom_window_start` is never below 0x0D000000.
                if address >= 0x0D00_0000 && self.is_eeprom_region(address) {
                    return 1;
                }
                let addr = (address & 0x01FF_FFFF) as usize;
                if addr < self.rom.len() {
                    self.rom[addr]
                } else {
                    // Past the end of the cartridge each halfword reads as
                    // its own address / 2 (GBATEK, Reading from Unused
                    // Memory; the mGBA suite's ROM out-of-bounds values).
                    (address >> 1 >> ((address & 1) * 8)) as u8
                }
            }
            // Cartridge backup. GBATEK, GBA Cart Backup SRAM/FRAM: mapped at
            // 0x0E000000, "the databus is restricted to 8 bits, it should be
            // accessed by LDRB, LDRSB, and STRB opcodes only".
            0x0E00_0000..=0x0FFF_FFFF => self.cart.save_read(address),
            _ => self.open_bus_byte(address),
        }
    }

    /// An I/O register byte as the hardware latched it, write-only bits
    /// included. This is the PPU's and APU's view; the CPU's goes through
    /// `read8` and `io::read_rule`.
    fn io_byte(&self, offset: usize) -> u8 {
        match offset {
            super::io::IO_IE => self.io.ie as u8,
            0x201 => (self.io.ie >> 8) as u8,
            super::io::IO_IF => self.io.if_ as u8,
            0x203 => (self.io.if_ >> 8) as u8,
            super::io::IO_IME => self.io.ime as u8,
            0x209 => (self.io.ime >> 8) as u8,
            super::io::IO_HALTCNT => {
                if self.io.halt {
                    0x80
                } else {
                    0
                }
            }
            // SIOCNT's read-only status bits, with no link cable attached
            // (GBATEK, GBA Communication Ports): normal mode reads SI high,
            // multiplayer reads SI and SD high, UART reads "receive FIFO
            // empty". Values from the mGBA suite's SIO register table.
            0x128 => {
                self.io_regs[0x128]
                    | match self.sio_mode() {
                        SioMode::Normal8 | SioMode::Normal32 => 0x04,
                        SioMode::Multi => 0x0C,
                        SioMode::Uart => 0x20,
                        SioMode::Gpio | SioMode::Joy => 0,
                    }
            }
            // UART's SIODATA8 is the receive FIFO, and it is empty.
            0x12A | 0x12B if self.sio_mode() == SioMode::Uart => 0,
            // RCNT bits 0-3 are the SC/SD/SI/SO pin levels. Only general
            // purpose mode drives them from the register; otherwise they
            // are what the idle port shows: normal mode SC and SI high and SO
            // from SIOCNT bit 3, JOY bus SI and SO high, the rest all high.
            0x134 => {
                let stored = self.io_regs[0x134];
                let pins = match self.sio_mode() {
                    SioMode::Gpio => stored & 0x0F,
                    SioMode::Normal8 | SioMode::Normal32 => 0x05 | (self.io_regs[0x128] & 0x08),
                    SioMode::Joy => 0x0C,
                    SioMode::Multi | SioMode::Uart => 0x0F,
                };
                (stored & 0xF0) | pins
            }
            _ => self.io_regs[offset],
        }
    }

    /// Start or cancel a transfer after a SIOCNT write. Deferred to the end
    /// of the instruction, so a halfword write sees its mode bits and its
    /// start bit together.
    ///
    /// Only a normal-mode transfer on the internal clock can finish with no
    /// cable: GBATEK, SIO Normal Mode, 8 or 32 bits at 256 KHz (64 cycles a
    /// bit) or 2 MHz (8). An external clock, multiplayer and UART wait for a
    /// peer that never comes, as on hardware with nothing plugged in.
    pub(crate) fn apply_sio_write(&mut self) {
        self.sio_written = false;
        let cnt = u16::from_le_bytes([self.io_regs[0x128], self.io_regs[0x129]]);
        if cnt & 0x80 == 0 {
            self.sio_cycles = 0;
            return;
        }
        let normal = matches!(self.sio_mode(), SioMode::Normal8 | SioMode::Normal32);
        if self.sio_cycles == 0 && normal && cnt & 1 != 0 {
            let bits = if cnt & 0x1000 != 0 { 32 } else { 8 };
            let per_bit = if cnt & 2 != 0 { 8 } else { 64 };
            self.sio_cycles = bits * per_bit;
        }
    }

    /// Advance a running transfer by `cycles`. On completion the start bit
    /// clears and, if SIOCNT bit 14 asks, IRQ 7 is requested; the return is
    /// how far into `cycles` that happened.
    pub(crate) fn tick_sio(&mut self, cycles: u32) -> Option<u32> {
        if self.sio_cycles == 0 {
            return None;
        }
        if cycles < self.sio_cycles {
            self.sio_cycles -= cycles;
            return None;
        }
        let at = self.sio_cycles;
        self.sio_cycles = 0;
        // The bits shifted in come from SI, which reads high with no cable
        // (GBATEK, SIOCNT bit 2: "1=High/None").
        if self.io_regs[0x129] & 0x10 != 0 {
            self.io_regs[0x120..0x124].fill(0xFF);
        } else {
            self.io_regs[0x12A] = 0xFF;
        }
        self.io_regs[0x128] &= !0x80;
        if self.io_regs[0x129] & 0x40 != 0 {
            self.io.request_interrupt(0x80);
            return Some(at);
        }
        None
    }

    /// The serial port mode RCNT bits 14-15 and SIOCNT bits 12-13 select.
    fn sio_mode(&self) -> SioMode {
        match (self.io_regs[0x135] >> 6, (self.io_regs[0x129] >> 4) & 3) {
            (2, _) => SioMode::Gpio,
            (3, _) => SioMode::Joy,
            (_, 0) => SioMode::Normal8,
            (_, 1) => SioMode::Normal32,
            (_, 2) => SioMode::Multi,
            _ => SioMode::Uart,
        }
    }

    /// A store to the serial registers, 0x120-0x15F, with the writable bits
    /// the mGBA suite's hardware table shows. False leaves the byte to the
    /// plain `io_regs` store.
    fn sio_store(&mut self, offset: usize, value: u8) -> bool {
        let stored = match offset {
            // SIODATA32 is writable in 32-bit normal mode only; in the other
            // modes these are the received SIOMULTI0/1, as SIOMULTI2/3 are
            // in every mode.
            0x120..=0x123 if self.sio_mode() == SioMode::Normal32 => value,
            0x120..=0x127 => return true,
            0x128 => {
                self.sio_written = true;
                value & 0x8F
            }
            0x129 => {
                self.sio_written = true;
                value & 0x7F
            }
            0x135 => value & 0xC1,
            // JOYCNT: bits 0-2 are acknowledged by writing 1, bit 6 is R/W.
            0x140 => (self.io_regs[0x140] & 0x07 & !value) | (value & 0x40),
            0x141 => 0,
            // JOY_RECV and JOY_TRANS read zero without a JOY bus host.
            0x150..=0x157 => return true,
            _ => return false,
        };
        self.io_regs[offset] = stored;
        true
    }

    /// The I/O byte at `address` as latched, for the PPU and APU.
    pub fn io_read8(&self, address: u32) -> u8 {
        self.io_byte((address & 0x3FF) as usize)
    }

    /// The I/O halfword at `address` as latched, for the PPU and APU.
    pub fn io_read16(&self, address: u32) -> u16 {
        let offset = (address & 0x3FE) as usize;
        u16::from_le_bytes([self.io_byte(offset), self.io_byte(offset + 1)])
    }

    /// GBATEK, Reading from Unused Memory: the bus still holds the most
    /// recently prefetched opcode. In ARM state that is `[$+8]`; in THUMB it
    /// is two halfwords whose source depends on the memory the code runs from.
    fn open_bus(&self) -> u32 {
        // Running from open-bus memory: there is no fetched opcode to return,
        // so read zero rather than recurse.
        if self.open_bus_busy.replace(true) {
            return 0;
        }
        let value = self.open_bus_fetch();
        self.open_bus_busy.set(false);
        value
    }

    fn open_bus_fetch(&self) -> u32 {
        let pc = self.exec_pc;
        if !self.exec_thumb {
            return self.read32(pc.wrapping_add(8));
        }
        let half = |offset: u32| u32::from(self.read16(pc.wrapping_add(offset)));
        let aligned = pc & 2 == 0;
        match pc >> 24 {
            // BIOS and OAM: a 32-bit bus fetching two opcodes at once.
            0x00 | 0x07 if aligned => half(4) | half(6) << 16,
            0x00 | 0x07 => half(2) | half(4) << 16,
            // IWRAM keeps the older halfword ("OldLO/OldHI", usually [$+2]).
            0x03 if aligned => half(4) | half(2) << 16,
            0x03 => half(2) | half(4) << 16,
            // 16-bit buses: the same halfword on both halves.
            _ => half(4) * 0x0001_0001,
        }
    }

    /// Record the opcode the CPU is prefetching while it runs in the BIOS.
    pub(crate) fn latch_bios_prefetch(&mut self) {
        self.bios_latch = self.open_bus();
    }

    /// The byte of the open-bus word that a read of `address` lands on.
    fn open_bus_byte(&self, address: u32) -> u8 {
        (self.open_bus() >> ((address & 3) * 8)) as u8
    }

    /// Word load with ARM's misaligned-access semantics: the bus always
    /// fetches the aligned word and the CPU rotates it right by the byte
    /// offset. Reading the unaligned bytes directly and then rotating gives a
    /// different, wrong answer.
    pub fn read32_rotated(&self, address: u32) -> u32 {
        // The 8-bit save bus returns the addressed byte on every lane, so
        // there is no aligned word to rotate.
        if Self::is_save_region(address) {
            return self.read32(address);
        }
        self.read32(address & !3).rotate_right((address & 3) * 8)
    }

    /// Alignment for 16- and 32-bit access is forced here rather than by the
    /// callers, because the backup region must see the *unmasked* address: its
    /// databus is 8 bits, so which byte is touched depends on the low bits
    /// that alignment would throw away.
    ///
    /// True for the cartridge backup region, whose databus is 8 bits wide.
    fn is_save_region(address: u32) -> bool {
        (0x0E00_0000..=0x0FFF_FFFF).contains(&address)
    }

    /// EEPROM lives at 0x0D000000 and is driven one bit at a time by DMA, so
    /// it needs the 16-bit path rather than the byte path the other save types
    /// use. On a cart without EEPROM the region stays a ROM mirror.
    fn is_eeprom_region(&self, address: u32) -> bool {
        self.cart.uses_eeprom()
            && (self.cart.eeprom_window_start()..=0x0DFF_FFFF).contains(&address)
    }

    /// The DMA unit announcing a transfer that touches the EEPROM window, so
    /// the cart can read the command's address width off its length.
    pub fn eeprom_begin_dma(&mut self, address: u32, words: usize) {
        if self.is_eeprom_region(address) {
            self.cart.eeprom_begin_dma(words);
        }
    }

    /// 16-bit read that can advance the EEPROM state machine. Reading EEPROM
    /// changes it, which a `&self` accessor cannot express.
    pub fn read16_mut(&mut self, address: u32) -> u16 {
        if self.is_eeprom_region(address) {
            return self.cart.eeprom_read();
        }
        self.read16(address)
    }

    /// The bytes of an aligned `width`-byte read from memory with no side
    /// effects and no per-byte rules - work RAM and cartridge ROM below the
    /// EEPROM window - so instruction fetches skip `read8`'s dispatch once per
    /// byte. `None` (anything else, or a read past the ROM's end) sends the
    /// read down the byte path, which stays the reference.
    fn plain(&self, address: u32, width: usize) -> Option<&[u8]> {
        let (mem, offset) = match address >> 24 {
            0x02 => (&self.ewram, (address & 0x3_FFFF) as usize),
            0x03 => (&self.iwram, (address & 0x7FFF) as usize),
            0x08..=0x0C => (&self.rom, (address & 0x01FF_FFFF) as usize),
            _ => return None,
        };
        mem.get(offset..offset + width)
    }

    pub fn read16(&self, address: u32) -> u16 {
        // An 8-bit databus cannot deliver two distinct bytes, so a halfword
        // read of the backup region returns the one byte replicated. Reading
        // consecutive addresses instead gives 0xFF01 where hardware gives
        // 0x0101 (gba-suite save test 4).
        if Self::is_save_region(address) {
            let byte = self.read8(address) as u16;
            return byte | (byte << 8);
        }
        let address = address & !1;
        if let Some(b) = self.plain(address, 2) {
            return u16::from_le_bytes([b[0], b[1]]);
        }
        let lo = self.read8(address) as u16;
        let hi = self.read8(address + 1) as u16;
        lo | (hi << 8)
    }

    pub fn read32(&self, address: u32) -> u32 {
        if Self::is_save_region(address) {
            let byte = self.read8(address) as u32;
            return byte * 0x0101_0101;
        }
        let address = address & !3;
        if let Some(b) = self.plain(address, 4) {
            return u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
        }
        let b0 = self.read8(address) as u32;
        let b1 = self.read8(address + 1) as u32;
        let b2 = self.read8(address + 2) as u32;
        let b3 = self.read8(address + 3) as u32;
        b0 | (b1 << 8) | (b2 << 16) | (b3 << 24)
    }

    /// 8-bit store with the GBA's video-memory rules applied.
    ///
    /// OAM ignores byte writes entirely; palette and the BG half of VRAM
    /// duplicate the byte across the addressed halfword; the OBJ half of VRAM
    /// ignores them. The OBJ boundary moves with the video mode: 0x14000 in
    /// the bitmap modes, 0x10000 in the tiled ones.
    ///
    /// `write16` and `write32` deliberately bypass this and use `store8`,
    /// because those rules apply only to genuine byte stores.
    pub fn write8(&mut self, address: u32, value: u8) {
        match address {
            0x0500_0000..=0x05FF_FFFF => {
                let offset = (address & 0x3FF) as usize & !1;
                self.palette[offset] = value;
                self.palette[offset + 1] = value;
            }
            0x0600_0000..=0x06FF_FFFF => {
                let offset = Self::vram_offset(address);
                let obj_base = if self.io_regs[0] & 7 >= 3 {
                    0x14000
                } else {
                    0x10000
                };
                if offset < obj_base {
                    let offset = offset & !1;
                    self.vram[offset] = value;
                    self.vram[offset + 1] = value;
                }
            }
            0x0700_0000..=0x07FF_FFFF => {}
            _ => self.store8(address, value),
        }
    }

    fn store8(&mut self, address: u32, value: u8) {
        match address {
            0x0200_0000..=0x02FF_FFFF => self.ewram[(address & 0x3_FFFF) as usize] = value,
            0x0300_0000..=0x03FF_FFFF => self.iwram[(address & 0x7FFF) as usize] = value,
            0x0400_0000..=0x0400_03FE => {
                let offset = (address & 0x3FF) as usize;
                if offset == super::io::IO_IE {
                    self.io.ie = (self.io.ie & 0xFF00) | (value as u16);
                    self.io_regs[offset] = value;
                    return;
                } else if offset == super::io::IO_IE + 1 {
                    self.io.ie = (self.io.ie & 0x00FF) | ((value as u16) << 8);
                    self.io_regs[offset] = value;
                    return;
                } else if offset == super::io::IO_IF {
                    self.io.if_ &= 0xFF00 | !(value as u16);
                    self.io_regs[offset] = value;
                    return;
                } else if offset == super::io::IO_IF + 1 {
                    self.io.if_ &= 0x00FF | !((value as u16) << 8);
                    self.io_regs[offset] = value;
                    return;
                } else if offset == super::io::IO_IME {
                    self.io.ime = value as u16 & 1;
                    self.io_regs[offset] = value;
                    return;
                } else if offset == super::io::IO_HALTCNT {
                    if value & 0x80 == 0 {
                        self.io.halt = true;
                    }
                } else if offset == 0x204 {
                    self.waitcnt = (self.waitcnt & 0xFF00) | (value as u16);
                } else if offset == 0x205 {
                    self.waitcnt = (self.waitcnt & 0x00FF) | ((value as u16) << 8);
                } else if (0x120..0x160).contains(&offset) && self.sio_store(offset, value) {
                    return;
                }
                self.io_regs[offset] = value;
                match offset {
                    0x60..=0x7F | 0x80..=0x88 | 0x90..=0x9F | 0xA0..=0xA7 => {
                        self.sound_writes.push((offset as u32, value));
                    }
                    // High byte of DMAxCNT_H (0x0BA + 12*x). It carries the
                    // enable bit and is the last byte a 16- or 32-bit write
                    // to the control register touches, so latching here
                    // always sees a complete control word.
                    0xBB | 0xC7 | 0xD3 | 0xDF => {
                        self.dma_writes.push((offset - 0xBB) / 12);
                    }
                    // High byte of TMxCNT_L (reload), or either byte of
                    // TMxCNT_H (control). The control register's enable bit is
                    // bit 7, in its *low* byte, so latching control only on
                    // the high byte dropped every `strb` that started a timer.
                    // A 16-bit control write latches twice; the first sees the
                    // old high byte, which is unused (GBATEK, TMxCNT_H bits
                    // 8-15). The reload stays high-byte only: `post_tick`
                    // keeps the live counter in TMxCNT_L's io_regs bytes, so a
                    // lone low-byte write would merge with the counter.
                    0x101 | 0x105 | 0x109 | 0x10D | 0x102 | 0x103 | 0x106 | 0x107 | 0x10A
                    | 0x10B | 0x10E | 0x10F => {
                        let timer = (offset - 0x100) / 4;
                        let is_control = offset & 2 != 0;
                        let base = offset & !1;
                        let word =
                            self.io_regs[base] as u16 | ((self.io_regs[base + 1] as u16) << 8);
                        self.timer_writes.push((timer, is_control, word));
                    }
                    _ => {}
                }
            }
            0x0500_0000..=0x05FF_FFFF => self.palette[(address & 0x3FF) as usize] = value,
            0x0600_0000..=0x06FF_FFFF => self.vram[Self::vram_offset(address)] = value,
            0x0700_0000..=0x07FF_FFFF => self.oam[(address & 0x3FF) as usize] = value,
            0x0E00_0000..=0x0FFF_FFFF => self.cart.save_write(address, value),
            _ => {}
        }
    }

    pub fn write16(&mut self, address: u32, value: u16) {
        if self.is_eeprom_region(address) {
            self.cart.eeprom_write(value);
            return;
        }
        // Only one byte reaches an 8-bit databus: the one selected by the
        // accessed address.
        if Self::is_save_region(address) {
            let byte = (value >> (8 * (address & 1))) as u8;
            self.store8(address, byte);
            return;
        }
        let address = address & !1;
        self.store8(address, (value & 0xFF) as u8);
        self.store8(address + 1, (value >> 8) as u8);
    }

    pub fn write32(&mut self, address: u32, value: u32) {
        if Self::is_save_region(address) {
            let byte = (value >> (8 * (address & 3))) as u8;
            self.store8(address, byte);
            return;
        }
        let address = address & !3;
        self.store8(address, (value & 0xFF) as u8);
        self.store8(address + 1, ((value >> 8) & 0xFF) as u8);
        self.store8(address + 2, ((value >> 16) & 0xFF) as u8);
        self.store8(address + 3, ((value >> 24) & 0xFF) as u8);
    }

    pub fn load_bios(&mut self, data: &[u8]) {
        let len = data.len().min(self.bios.len());
        self.bios[..len].copy_from_slice(&data[..len]);
    }

    pub fn load_rom(&mut self, data: &[u8]) {
        self.rom = data.to_vec();
    }

    pub fn read_cycles(&self, address: u32, is_32bit: bool) -> u32 {
        match address {
            0x0000_0000..=0x0000_3FFF => 1,
            0x0200_0000..=0x0203_FFFF => {
                if is_32bit {
                    6
                } else {
                    3
                }
            }
            0x0300_0000..=0x0300_7FFF => {
                if is_32bit {
                    2
                } else {
                    1
                }
            }
            0x0400_0000..=0x0400_03FE => 1,
            0x0500_0000..=0x0500_03FF => 1,
            0x0600_0000..=0x0601_7FFF => 1,
            0x0700_0000..=0x0700_03FF => 1,
            0x0800_0000..=0x09FF_FFFF => {
                let ws = (self.waitcnt >> 2) & 3;
                if is_32bit {
                    ws as u32 * 2 + 6
                } else {
                    ws as u32 + 3
                }
            }
            0x0A00_0000..=0x0BFF_FFFF => {
                let ws = (self.waitcnt >> 5) & 3;
                if is_32bit {
                    ws as u32 * 2 + 6
                } else {
                    ws as u32 + 3
                }
            }
            0x0C00_0000..=0x0DFF_FFFF => {
                let ws = (self.waitcnt >> 8) & 3;
                if is_32bit {
                    ws as u32 * 2 + 6
                } else {
                    ws as u32 + 3
                }
            }
            _ => 1,
        }
    }

    pub fn write_cycles(&self, address: u32, is_32bit: bool) -> u32 {
        match address {
            0x0200_0000..=0x0203_FFFF => {
                if is_32bit {
                    6
                } else {
                    3
                }
            }
            0x0300_0000..=0x0300_7FFF => 1,
            0x0400_0000..=0x0400_03FE => 1,
            0x0500_0000..=0x0500_03FF => 1,
            0x0600_0000..=0x0601_7FFF => 2,
            0x0700_0000..=0x0700_03FF => 1,
            _ => 1,
        }
    }

    pub fn drain_sound_writes(&mut self) -> Vec<(u32, u8)> {
        std::mem::take(&mut self.sound_writes)
    }

    pub fn drain_timer_writes(&mut self) -> Vec<(usize, bool, u16)> {
        std::mem::take(&mut self.timer_writes)
    }

    pub fn drain_dma_writes(&mut self) -> Vec<usize> {
        std::mem::take(&mut self.dma_writes)
    }

    pub fn io_regs_data(&self) -> &[u8] {
        &self.io_regs
    }
    pub fn io_regs_data_mut(&mut self) -> &mut [u8] {
        &mut self.io_regs
    }
    pub fn ewram_data(&self) -> &[u8] {
        &self.ewram
    }
    pub fn ewram_data_mut(&mut self) -> &mut [u8] {
        &mut self.ewram
    }
    pub fn iwram_data(&self) -> &[u8] {
        &self.iwram
    }
    pub fn iwram_data_mut(&mut self) -> &mut [u8] {
        &mut self.iwram
    }
    pub fn palette_data(&self) -> &[u8] {
        &self.palette
    }
    pub fn palette_data_mut(&mut self) -> &mut [u8] {
        &mut self.palette
    }
    pub fn vram_data(&self) -> &[u8] {
        &self.vram
    }
    pub fn vram_data_mut(&mut self) -> &mut [u8] {
        &mut self.vram
    }
    pub fn oam_data(&self) -> &[u8] {
        &self.oam
    }
    pub fn oam_data_mut(&mut self) -> &mut [u8] {
        &mut self.oam
    }
}
