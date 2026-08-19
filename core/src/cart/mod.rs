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
}

impl Cartridge {
    pub fn empty() -> Self {
        Self {
            rom: Vec::new(),
            save_type: SaveType::None,
            title: String::new(),
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
            return Err(CartError::BadChecksum);
        }

        // Extract title (12 bytes at 0xA0)
        let title_bytes = &data[0xA0..0xAC];
        let title = String::from_utf8_lossy(title_bytes)
            .trim_end_matches('\0')
            .to_string();

        // Detect save type from game code or ROM content
        let save_type = detect_save_type(data, &title);

        Ok(Self {
            rom: data.to_vec(),
            save_type,
            title,
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
}

fn detect_save_type(_rom: &[u8], title: &str) -> SaveType {
    // TODO: More sophisticated detection based on game code
    // This is a placeholder - real detection uses game database
    // and ROM content analysis (looking for save-related strings)
    match title {
        "POKEMON RUBY" | "POKEMON SAPPHIRE" | "POKEMON EMERALD" => SaveType::Flash128,
        "POKEMON FIRERED" | "POKEMON LEAFGREEN" => SaveType::Flash128,
        _ => SaveType::None,
    }
}
