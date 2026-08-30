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
    dma_writes: Vec<usize>,
    /// Timer register writes, same shape again. `Timer::set_control` and
    /// `set_reload` had no caller outside the tests for the project's whole
    /// history, so no timer a game started ever ran: no timer interrupt, and
    /// no DMA sound, which is driven entirely by timer overflows.
    timer_writes: Vec<(usize, bool, u16)>,
    prefetch_enabled: bool,
    prefetch_seq_count: u32,
    prefetch_cyc: u32,
    iwram_prefetch: [u32; 8],
    iwram_prefetch_pos: usize,
    iwram_prefetch_count: usize,
    pub io: super::io::IoHandler,
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
            prefetch_enabled: false,
            prefetch_seq_count: 0,
            prefetch_cyc: 0,
            iwram_prefetch: [0; 8],
            iwram_prefetch_pos: 0,
            iwram_prefetch_count: 0,
            io: super::io::IoHandler::new(),
        };
        bus.init_bios();
        // KEYINPUT is active low ("0=Pressed, 1=Released", GBATEK Keypad Input),
        // so a zero-filled io_regs would read as all ten buttons held forever.
        bus.set_keys(0);
        bus
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

    /// Populate BIOS memory with the IRQ handler the real BIOS has at 0x18:
    /// save r0-r3/r12/lr, call the game's handler from [0x03007FFC], restore,
    /// and return with `SUBS PC, LR, #4`. Acknowledging IF and updating the
    /// IntrWait flags at 0x03007FF8 are the game handler's responsibility on
    /// hardware, and doing either here breaks games that read IF themselves.
    fn init_bios(&mut self) {
        // BIOS IRQ handler at 0x18: save registers, call the game handler at
        // [0x03007FFC], restore, return.
        //
        // Layout:
        //   0x18: stmdb sp!, {r0-r3, r12, lr}
        //   0x1C: mov r12, #0x04000000
        //   0x20: ldr r0, [r12, #-4]     ; [0x03FFFFFC] = the game's handler
        //   0x24: blx r0
        //   0x28: ldmia sp!, {r0-r3, r12, lr}
        //   0x2C: subs pc, lr, #4        ; return from IRQ, restoring CPSR
        //
        // That is all the real BIOS does, and the omissions are the point.
        // It does **not** acknowledge IF, and it does **not** touch the
        // IntrWait flags at 0x03007FF8 - both are the game handler's job.
        // A stub that acknowledged IF first broke every game whose handler
        // dispatches on `IE & IF`: Yggdra Union's IWRAM dispatcher read zero,
        // fell through its whole chain and called the wrong handler for every
        // interrupt, so its VBlank work - which is where DISPCNT is written -
        // never ran at all.
        let bios_irq_handler: [u32; 4] = [
            0xE92D500F, // stmdb sp!, {r0-r3, r12, lr}

            0xE3A0C301, // mov r12, #0x04000000
            0xE51C0004, // ldr r0, [r12, #-4]        ; r0 = [0x03FFFFFC]
            0xE12FFF30, // blx r0                    ; call game handler
        ];

        let epilogue: [u32; 2] = [
            0xE8BD500F, // ldmia sp!, {r0-r3, r12, lr}
            0xE25EF004, // subs pc, lr, #4
        ];

        let offset = 0x18usize;
        let words = bios_irq_handler.iter().chain(epilogue.iter());
        for (i, &word) in words.enumerate() {
            let addr = offset + i * 4;
            self.bios[addr..addr + 4].copy_from_slice(&word.to_le_bytes());
        }
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

    pub fn read8(&self, address: u32) -> u8 {
        match address {
            0x0000_0000..=0x0000_3FFF => self.bios[(address & 0x3FFF) as usize],
            0x0200_0000..=0x02FF_FFFF => self.ewram[(address & 0x3_FFFF) as usize],
            0x0300_0000..=0x03FF_FFFF => self.iwram[(address & 0x7FFF) as usize],
            0x0400_0000..=0x0400_03FE => {
                let offset = (address & 0x3FF) as usize;
                if offset == super::io::IO_IE {
                    (self.io.ie & 0xFF) as u8
                } else if offset == super::io::IO_IE + 1 {
                    (self.io.ie >> 8) as u8
                } else if offset == super::io::IO_IF {
                    (self.io.if_ & 0xFF) as u8
                } else if offset == super::io::IO_IF + 1 {
                    (self.io.if_ >> 8) as u8
                } else if offset == super::io::IO_IME {
                    (self.io.ime & 0xFF) as u8
                } else if offset == super::io::IO_IME + 1 {
                    (self.io.ime >> 8) as u8
                } else if offset == 0x301 {
                    if self.io.halt { 0x80 } else { 0 }
                } else {
                    self.io_regs[offset]
                }
            }
            0x0500_0000..=0x05FF_FFFF => self.palette[(address & 0x3FF) as usize],
            0x0600_0000..=0x06FF_FFFF => self.vram[Self::vram_offset(address)],
            0x0700_0000..=0x07FF_FFFF => self.oam[(address & 0x3FF) as usize],
            // The cartridge appears three times, once per wait-state region:
            // 0x08 (WS0), 0x0A (WS1) and 0x0C (WS2). Same data, different
            // access timing.
            0x0800_0000..=0x0DFF_FFFF => {
                let addr = (address & 0x01FF_FFFF) as usize;
                if addr < self.rom.len() { self.rom[addr] } else { 0 }
            }
            // Cartridge backup. GBATEK, GBA Cart Backup SRAM/FRAM: mapped at
            // 0x0E000000, "the databus is restricted to 8 bits, it should be
            // accessed by LDRB, LDRSB, and STRB opcodes only".
            0x0E00_0000..=0x0FFF_FFFF => self.cart.save_read(address),
            _ => 0,
        }
    }

    /// Word load with ARM's misaligned-access semantics: the bus always
    /// fetches the aligned word and the CPU rotates it right by the byte
    /// offset. Reading the unaligned bytes directly and then rotating gives a
    /// different, wrong answer.
    pub fn read32_rotated(&self, address: u32) -> u32 {
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
        (0x0D00_0000..=0x0DFF_FFFF).contains(&address) && self.cart.uses_eeprom()
    }

    /// 16-bit read that can advance the EEPROM state machine. Reading EEPROM
    /// changes it, which a `&self` accessor cannot express.
    pub fn read16_mut(&mut self, address: u32) -> u16 {
        if self.is_eeprom_region(address) {
            return self.cart.eeprom_read();
        }
        self.read16(address)
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
                let obj_base = if self.io_regs[0] & 7 >= 3 { 0x14000 } else { 0x10000 };
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
                    self.prefetch_enabled = self.waitcnt & 0x4000 != 0;
                } else if offset == 0x205 {
                    self.waitcnt = (self.waitcnt & 0x00FF) | ((value as u16) << 8);
                    self.prefetch_enabled = self.waitcnt & 0x4000 != 0;
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
                    // High byte of TMxCNT_L (reload) or TMxCNT_H (control),
                    // latched for the same reason as DMA's: it is the last
                    // byte a 16-bit write touches.
                    0x101 | 0x103 | 0x105 | 0x107 | 0x109 | 0x10B | 0x10D | 0x10F => {
                        let timer = (offset - 0x100) / 4;
                        let is_control = offset % 4 == 3;
                        let base = offset & !1;
                        let word = self.io_regs[base] as u16 | ((self.io_regs[base + 1] as u16) << 8);
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
            0x0200_0000..=0x0203_FFFF => if is_32bit { 6 } else { 3 },
            0x0300_0000..=0x0300_7FFF => if is_32bit { 2 } else { 1 },
            0x0400_0000..=0x0400_03FE => 1,
            0x0500_0000..=0x0500_03FF => 1,
            0x0600_0000..=0x0601_7FFF => 1,
            0x0700_0000..=0x0700_03FF => 1,
            0x0800_0000..=0x09FF_FFFF => {
                let ws = (self.waitcnt >> 2) & 3;
                if is_32bit { ws as u32 * 2 + 6 } else { ws as u32 + 3 }
            }
            0x0A00_0000..=0x0BFF_FFFF => {
                let ws = (self.waitcnt >> 5) & 3;
                if is_32bit { ws as u32 * 2 + 6 } else { ws as u32 + 3 }
            }
            0x0C00_0000..=0x0DFF_FFFF => {
                let ws = (self.waitcnt >> 8) & 3;
                if is_32bit { ws as u32 * 2 + 6 } else { ws as u32 + 3 }
            }
            _ => 1,
        }
    }

    pub fn write_cycles(&self, address: u32, is_32bit: bool) -> u32 {
        match address {
            0x0200_0000..=0x0203_FFFF => if is_32bit { 6 } else { 3 },
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

    /// Advance the gamepak prefetch buffer by one cycle.
    pub fn prefetch_tick(&mut self) {
        if !self.prefetch_enabled { return; }
        if self.prefetch_cyc > 0 {
            self.prefetch_cyc -= 1;
        }
    }

    pub fn io_regs_data(&self) -> &[u8] { &self.io_regs }
    pub fn io_regs_data_mut(&mut self) -> &mut [u8] { &mut self.io_regs }
    pub fn ewram_data(&self) -> &[u8] { &self.ewram }
    pub fn ewram_data_mut(&mut self) -> &mut [u8] { &mut self.ewram }
    pub fn iwram_data(&self) -> &[u8] { &self.iwram }
    pub fn iwram_data_mut(&mut self) -> &mut [u8] { &mut self.iwram }
    pub fn palette_data(&self) -> &[u8] { &self.palette }
    pub fn palette_data_mut(&mut self) -> &mut [u8] { &mut self.palette }
    pub fn vram_data(&self) -> &[u8] { &self.vram }
    pub fn vram_data_mut(&mut self) -> &mut [u8] { &mut self.vram }
    pub fn oam_data(&self) -> &[u8] { &self.oam }
    pub fn oam_data_mut(&mut self) -> &mut [u8] { &mut self.oam }
}
