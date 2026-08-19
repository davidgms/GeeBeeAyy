pub struct MemoryBus {
    ewram: Vec<u8>,    // 256 KB External Work RAM
    iwram: Vec<u8>,    // 32 KB Internal Work RAM
    io_regs: Vec<u8>,  // I/O Registers
    palette: Vec<u8>,  // 1 KB Palette RAM
    vram: Vec<u8>,     // 96 KB Video RAM
    oam: Vec<u8>,      // 1 KB OAM
    bios: Vec<u8>,     // 16 KB BIOS (HLE or real)
    rom: Vec<u8>,      // Cartridge ROM (up to 32 MB)
    waitcnt: u16,      // Wait State Control
    /// Pending sound register writes (offset from 0x04000000, value)
    pub sound_writes: Vec<(u32, u8)>,
    // Prefetch buffer
    prefetch_enabled: bool,
    prefetch_seq_count: u32,  // sequential prefetches buffered
    prefetch_cyc: u32,        // cycles available in prefetch buffer
    iwram_prefetch: [u32; 8], // prefetched 32-bit words from IWRAM
    iwram_prefetch_pos: usize,
    iwram_prefetch_count: usize,
}

impl MemoryBus {
    pub fn new() -> Self {
        Self {
            ewram: vec![0; 256 * 1024],
            iwram: vec![0; 32 * 1024],
            io_regs: vec![0; 0x400],
            palette: vec![0; 1024],
            vram: vec![0; 96 * 1024],
            oam: vec![0; 1024],
            bios: vec![0; 16 * 1024],
            rom: Vec::new(),
            waitcnt: 0,
            sound_writes: Vec::new(),
            prefetch_enabled: false,
            prefetch_seq_count: 0,
            prefetch_cyc: 0,
            iwram_prefetch: [0; 8],
            iwram_prefetch_pos: 0,
            iwram_prefetch_count: 0,
        }
    }

    pub fn read8(&self, address: u32) -> u8 {
        match address {
            0x0000_0000..=0x0000_3FFF => self.bios[(address & 0x3FFF) as usize],
            0x0200_0000..=0x0203_FFFF => self.ewram[(address & 0x3FFFF) as usize],
            0x0300_0000..=0x0300_7FFF => self.iwram[(address & 0x7FFF) as usize],
            0x0400_0000..=0x0400_03FE => self.io_regs[(address & 0x3FF) as usize],
            0x0500_0000..=0x0500_03FF => self.palette[(address & 0x3FF) as usize],
            0x0600_0000..=0x0601_7FFF => self.vram[(address & 0x17FFF) as usize],
            0x0700_0000..=0x0700_03FF => self.oam[(address & 0x3FF) as usize],
            // ROM region (0x08000000 - 0x09FFFFFF) with mirroring
            0x0800_0000..=0x09FF_FFFF => {
                let addr = (address - 0x0800_0000) as usize;
                if addr < self.rom.len() {
                    self.rom[addr]
                } else {
                    0
                }
            }
            0x0A00_0000..=0x0BFF_FFFF => {
                let addr = (address - 0x0A00_0000) as usize;
                if addr < self.rom.len() {
                    self.rom[addr]
                } else {
                    0
                }
            }
            _ => 0,
        }
    }

    pub fn read16(&self, address: u32) -> u16 {
        let lo = self.read8(address) as u16;
        let hi = self.read8(address + 1) as u16;
        lo | (hi << 8)
    }

    pub fn read32(&self, address: u32) -> u32 {
        let b0 = self.read8(address) as u32;
        let b1 = self.read8(address + 1) as u32;
        let b2 = self.read8(address + 2) as u32;
        let b3 = self.read8(address + 3) as u32;
        b0 | (b1 << 8) | (b2 << 16) | (b3 << 24)
    }

    pub fn write8(&mut self, address: u32, value: u8) {
        match address {
            0x0200_0000..=0x0203_FFFF => self.ewram[(address & 0x3FFFF) as usize] = value,
            0x0300_0000..=0x0300_7FFF => self.iwram[(address & 0x7FFF) as usize] = value,
            0x0400_0000..=0x0400_03FE => {
                let offset = address - 0x0400_0000;
                self.io_regs[(address & 0x3FF) as usize] = value;
                // Queue sound register writes for APU
                match offset {
                    0x60..=0x7F | 0x80..=0x88 | 0x90..=0x9F | 0xA0..=0xA7 => {
                        self.sound_writes.push((offset, value));
                    }
                    _ => {}
                }
            }
            0x0500_0000..=0x0500_03FF => self.palette[(address & 0x3FF) as usize] = value,
            0x0600_0000..=0x0601_7FFF => self.vram[(address & 0x17FFF) as usize] = value,
            0x0700_0000..=0x0700_03FF => self.oam[(address & 0x3FF) as usize] = value,
            _ => {}
        }
    }

    pub fn write16(&mut self, address: u32, value: u16) {
        self.write8(address, (value & 0xFF) as u8);
        self.write8(address + 1, (value >> 8) as u8);
    }

    pub fn write32(&mut self, address: u32, value: u32) {
        self.write8(address, (value & 0xFF) as u8);
        self.write8(address + 1, ((value >> 8) & 0xFF) as u8);
        self.write8(address + 2, ((value >> 16) & 0xFF) as u8);
        self.write8(address + 3, ((value >> 24) & 0xFF) as u8);
    }

    pub fn load_bios(&mut self, data: &[u8]) {
        let len = data.len().min(self.bios.len());
        self.bios[..len].copy_from_slice(&data[..len]);
    }

    pub fn load_rom(&mut self, data: &[u8]) {
        self.rom = data.to_vec();
    }

    /// Returns the number of CPU cycles for a read at the given address.
    /// GBA memory timing depends on region and access type.
    pub fn read_cycles(&self, address: u32, is_32bit: bool) -> u32 {
        match address {
            0x0000_0000..=0x0000_3FFF => 1,                      // BIOS: 1 cycle (cached)
            0x0200_0000..=0x0203_FFFF => if is_32bit { 6 } else { 3 },  // EWRAM
            0x0300_0000..=0x0300_7FFF => if is_32bit { 2 } else { 1 },  // IWRAM
            0x0400_0000..=0x0400_03FE => 1,                       // I/O
            0x0500_0000..=0x0500_03FF => 1,                       // Palette
            0x0600_0000..=0x0601_7FFF => 1,                       // VRAM
            0x0700_0000..=0x0700_03FF => 1,                       // OAM
            0x0800_0000..=0x09FF_FFFF => {                         // ROM Wait State 0
                let ws = (self.waitcnt >> 2) & 3;
                if is_32bit { ws as u32 * 2 + 6 } else { ws as u32 + 3 }
            }
            0x0A00_0000..=0x0BFF_FFFF => {                         // ROM Wait State 1
                let ws = (self.waitcnt >> 5) & 3;
                if is_32bit { ws as u32 * 2 + 6 } else { ws as u32 + 3 }
            }
            0x0C00_0000..=0x0DFF_FFFF => {                         // ROM Wait State 2
                let ws = (self.waitcnt >> 8) & 3;
                if is_32bit { ws as u32 * 2 + 6 } else { ws as u32 + 3 }
            }
            _ => 1,
        }
    }

    /// Returns the number of CPU cycles for a write at the given address.
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

    /// Update WAITCNT register
    pub fn set_waitcnt(&mut self, value: u16) {
        self.waitcnt = value;
        self.prefetch_enabled = value & 0x4000 != 0;
    }

    /// Get WAITCNT register
    pub fn get_waitcnt(&self) -> u16 {
        self.waitcnt
    }

    /// Drain pending sound register writes
    pub fn drain_sound_writes(&mut self) -> Vec<(u32, u8)> {
        std::mem::take(&mut self.sound_writes)
    }

    // Accessor methods for save states
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

    /// Tick the prefetch buffer (called each CPU cycle).
    /// When prefetch is enabled and CPU is executing from ROM,
    /// we can buffer sequential reads at 0 cost.
    pub fn prefetch_tick(&mut self) {
        if !self.prefetch_enabled {
            return;
        }
        // Simple model: accumulate 1 cycle of prefetch budget per tick
        // Real hardware: prefetches happen during non-sequential cycles
        if self.prefetch_cyc < 8 {
            self.prefetch_cyc += 1;
        }
    }

    /// Returns read cycles for ROM, accounting for prefetch buffer.
    /// If prefetch is enabled and we have buffered data, return 0 (free).
    pub fn rom_read_cycles_prefetch(&mut self, address: u32, is_32bit: bool, is_seq: bool) -> u32 {
        if !self.prefetch_enabled || !is_seq {
            return self.read_cycles(address, is_32bit);
        }

        // Sequential ROM read with prefetch: free if we have budget
        let cost = self.read_cycles(address, is_32bit);
        if self.prefetch_cyc >= cost {
            self.prefetch_cyc -= cost;
            0 // Free! Prefetch absorbed the cost
        } else {
            let remaining = cost - self.prefetch_cyc;
            self.prefetch_cyc = 0;
            remaining
        }
    }

    /// Prefetch from IWRAM (internal memory, 2 cycles)
    pub fn iwram_prefetch_tick(&mut self) {
        // IWRAM has 2-cycle sequential access; prefetch can buffer words
        if self.iwram_prefetch_count < 8 {
            // Simulate prefetching a word from IWRAM
            self.iwram_prefetch_count += 1;
        }
    }
}
