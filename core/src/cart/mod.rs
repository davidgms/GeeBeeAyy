use thiserror::Error;

#[derive(Error, Debug)]
pub enum CartError {
    #[error("Invalid ROM size: {0} bytes")]
    InvalidSize(usize),

    #[error("Invalid ROM header checksum")]
    BadChecksum,

    #[error("Unsupported save type")]
    UnsupportedSave,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SaveType {
    None,
    Sram,
    Flash64,
    Flash128,
    Eeprom512,
    Eeprom8k,
}

pub struct Cartridge {
    rom: Vec<u8>,
    save_type: SaveType,
    title: String,
    sram: Vec<u8>,       // SRAM 32KB
    flash: Vec<u8>,      // Flash 64KB/128KB
    eeprom: Vec<u8>,     // EEPROM 512B/8KB
    flash_state: u8,     // Flash command state machine
    flash_bank: usize,   // Flash bank select (for 128KB)
    /// Set whenever a byte of save memory actually changes, so a frontend can
    /// flush without diffing the whole buffer every frame.
    save_dirty: bool,
}

impl Cartridge {
    pub fn empty() -> Self {
        Self {
            rom: Vec::new(),
            save_type: SaveType::None,
            title: String::new(),
            sram: vec![0xFF; 32 * 1024],
            flash: vec![0xFF; 128 * 1024],
            eeprom: vec![0xFF; 8 * 1024],
            flash_state: 0,
            flash_bank: 0,
            save_dirty: false,
        }
    }

    pub fn from_bytes(data: &[u8]) -> Result<Self, CartError> {
        if data.len() < 0xC0 || data.len() > 32 * 1024 * 1024 {
            return Err(CartError::InvalidSize(data.len()));
        }

        // Verify header checksum (at 0xBD)
        let mut checksum: u8 = 0;
        for i in 0x0A..0xBD {
            checksum = checksum.wrapping_sub(data[i]);
        }
        checksum = checksum.wrapping_sub(0x19);
        if checksum != data[0xBD] {
            eprintln!("Warning: bad header checksum (got 0x{:02X}, expected 0x{:02X})", checksum, data[0xBD]);
        }

        let title_bytes = &data[0xA0..0xAC];
        let title = String::from_utf8_lossy(title_bytes)
            .trim_end_matches('\0')
            .to_string();

        let save_type = detect_save_type(data, &title);

        let sram_size = 32 * 1024;
        let flash_size = match save_type {
            SaveType::Flash128 => 128 * 1024,
            SaveType::Flash64 => 64 * 1024,
            _ => 0,
        };
        let eeprom_size = match save_type {
            SaveType::Eeprom8k => 8 * 1024,
            SaveType::Eeprom512 => 512,
            _ => 0,
        };

        Ok(Self {
            rom: data.to_vec(),
            save_type,
            title,
            // Erased save memory reads 0xFF; every gba-suite save ROM checks
            // this before anything else.
            sram: vec![0xFF; sram_size],
            flash: vec![0xFF; flash_size],
            eeprom: vec![0xFF; eeprom_size],
            flash_state: 0,
            flash_bank: 0,
            save_dirty: false,
        })
    }

    pub fn read(&self, address: u32) -> u8 {
        let addr = address as usize;
        if addr < self.rom.len() {
            self.rom[addr]
        } else {
            0
        }
    }

    pub fn read16(&self, address: u32) -> u16 {
        let lo = self.read(address) as u16;
        let hi = self.read(address + 1) as u16;
        lo | (hi << 8)
    }

    pub fn read32(&self, address: u32) -> u32 {
        let b0 = self.read(address) as u32;
        let b1 = self.read(address + 1) as u32;
        let b2 = self.read(address + 2) as u32;
        let b3 = self.read(address + 3) as u32;
        b0 | (b1 << 8) | (b2 << 16) | (b3 << 24)
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn save_type(&self) -> SaveType {
        self.save_type
    }

    pub fn len(&self) -> usize {
        self.rom.len()
    }

    /// Read a byte from save memory
    pub fn save_read(&self, address: u32) -> u8 {
        match self.save_type {
            SaveType::Sram => {
                let addr = (address as usize) & 0x7FFF;
                if addr < self.sram.len() { self.sram[addr] } else { 0 }
            }
            SaveType::Flash64 | SaveType::Flash128 => {
                let addr = ((self.flash_bank << 16) | (address as usize & 0xFFFF))
                    & (self.flash.len() - 1);
                if addr < self.flash.len() { self.flash[addr] } else { 0xFF }
            }
            SaveType::Eeprom512 | SaveType::Eeprom8k => {
                let addr = (address as usize) & (self.eeprom.len() - 1);
                self.eeprom[addr]
            }
            // An absent save chip floats high on the 8-bit databus.
            SaveType::None => 0xFF,
        }
    }

    /// Take the dirty flag, clearing it.
    ///
    /// Check this **before** reading the bytes out. Reversed, a write landing
    /// between the read and the clear is lost.
    pub fn take_save_dirty(&mut self) -> bool {
        std::mem::replace(&mut self.save_dirty, false)
    }

    /// Write a byte to save memory
    pub fn save_write(&mut self, address: u32, value: u8) {
        match self.save_type {
            SaveType::Sram => {
                let addr = (address as usize) & 0x7FFF;
                if addr < self.sram.len() {
                    self.sram[addr] = value;
                    self.save_dirty = true;
                }
            }
            SaveType::Flash64 | SaveType::Flash128 => {
                // Flash command state machine
                match self.flash_state {
                    0 => {
                        // Command unlock sequence
                        if value == 0xAA { self.flash_state = 1; }
                    }
                    1 => {
                        if value == 0x55 { self.flash_state = 2; }
                        else { self.flash_state = 0; }
                    }
                    2 => {
                        match value {
                            0x90 => self.flash_state = 3, // Chip ID mode
                            0xA0 => self.flash_state = 4, // Byte program
                            0xB0 => self.flash_state = 5, // Bank select
                            0x80 => self.flash_state = 6, // Erase
                            _ => self.flash_state = 0,
                        }
                    }
                    3 => { // Chip ID mode
                        self.flash_state = 0;
                    }
                    4 => { // Byte program
                        let addr = ((self.flash_bank << 16) | (address as usize & 0xFFFF)) & (self.flash.len() - 1);
                        if addr < self.flash.len() {
                            self.flash[addr] = value;
                            self.save_dirty = true;
                        }
                        self.flash_state = 0;
                    }
                    5 => { // Bank select
                        self.flash_bank = (value as usize) & 1;
                        self.flash_state = 0;
                    }
                    // Erase needs a SECOND unlock sequence before the command:
                    // the full form is AA,55,80,AA,55, then 0x10 (chip) or 0x30
                    // (sector). Taking the command straight after 0x80 meant the
                    // 0xAA was swallowed here and the erase never ran.
                    6 => {
                        if value == 0xAA { self.flash_state = 7; } else { self.flash_state = 0; }
                    }
                    7 => {
                        if value == 0x55 { self.flash_state = 8; } else { self.flash_state = 0; }
                    }
                    8 => {
                        match value {
                            0x30 => {
                                let addr = ((self.flash_bank << 16)
                                    | (address as usize & 0xFFFF))
                                    & (self.flash.len() - 1);
                                let sector = addr & !0xFFF;
                                if sector + 0x1000 <= self.flash.len() {
                                    self.flash[sector..sector + 0x1000].fill(0xFF);
                                    self.save_dirty = true;
                                }
                            }
                            0x10 => {
                                self.flash.fill(0xFF);
                                self.save_dirty = true;
                            }
                            _ => {}
                        }
                        self.flash_state = 0;
                    }
                    _ => self.flash_state = 0,
                }
            }
            SaveType::Eeprom512 | SaveType::Eeprom8k => {
                let addr = (address as usize) & (self.eeprom.len() - 1);
                self.eeprom[addr] = value;
            }
            SaveType::None => {}
        }
    }

    /// Get save data for saving to file
    pub fn save_data(&self) -> Option<Vec<u8>> {
        match self.save_type {
            SaveType::Sram => Some(self.sram.clone()),
            SaveType::Flash64 => Some(self.flash[..64 * 1024].to_vec()),
            SaveType::Flash128 => Some(self.flash.clone()),
            SaveType::Eeprom512 => Some(self.eeprom[..512].to_vec()),
            SaveType::Eeprom8k => Some(self.eeprom.clone()),
            SaveType::None => None,
        }
    }

    /// Load save data from file
    pub fn load_save(&mut self, data: &[u8]) {
        match self.save_type {
            SaveType::Sram => {
                let len = data.len().min(self.sram.len());
                self.sram[..len].copy_from_slice(&data[..len]);
            }
            SaveType::Flash64 | SaveType::Flash128 => {
                let len = data.len().min(self.flash.len());
                self.flash[..len].copy_from_slice(&data[..len]);
            }
            SaveType::Eeprom512 | SaveType::Eeprom8k => {
                let len = data.len().min(self.eeprom.len());
                self.eeprom[..len].copy_from_slice(&data[..len]);
            }
            SaveType::None => {}
        }
    }
}

fn detect_save_type(rom: &[u8], title: &str) -> SaveType {
    // First try game code database
    if rom.len() >= 0xAC {
        let game_code = &rom[0xAC..0xB0];
        let code = std::str::from_utf8(game_code).unwrap_or("");

        // Known save types by game code
        match code {
            "GBXP" | "GBXJ" | "GBXE" => return SaveType::Sram,
            "GB4P" | "GB4J" | "GB4E" => return SaveType::Flash128,
            _ => {}
        }
    }

    // Scan ROM for save type strings
    let rom_str = String::from_utf8_lossy(rom);
    if rom_str.contains("SRAM_V") {
        SaveType::Sram
    } else if rom_str.contains("FLASH_V") || rom_str.contains("FLASH512_V") {
        SaveType::Flash64
    } else if rom_str.contains("FLASH1M_V") {
        SaveType::Flash128
    } else if rom_str.contains("EEPROM_V") {
        // Determine size from ROM size or game code
        if rom.len() > 0x1000000 {
            SaveType::Eeprom8k
        } else {
            SaveType::Eeprom512
        }
    } else {
        // Fallback: check title for known games
        match title {
            "POKEMON RUBY" | "POKEMON SAPPHIRE" | "POKEMON EMERALD" => SaveType::Flash128,
            "POKEMON FIRERED" | "POKEMON LEAFGREEN" => SaveType::Flash128,
            _ => SaveType::None,
        }
    }
}
