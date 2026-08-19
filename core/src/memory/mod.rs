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
            0x0400_0000..=0x0400_03FE => self.io_regs[(address & 0x3FF) as usize] = value,
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
}
