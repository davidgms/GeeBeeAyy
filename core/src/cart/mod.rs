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
    sram: Vec<u8>,     // SRAM 32KB
    flash: Vec<u8>,    // Flash 64KB/128KB
    eeprom: Vec<u8>,   // EEPROM 512B/8KB
    flash_state: u8,   // Flash command state machine
    flash_bank: usize, // Flash bank select (for 128KB)
    /// Set whenever a byte of save memory actually changes, so a frontend can
    /// flush without diffing the whole buffer every frame.
    save_dirty: bool,
    /// Flash chip-ID mode: reads return the manufacturer and device bytes
    /// instead of data until the mode is terminated.
    flash_id_mode: bool,
    /// Serial EEPROM state. Unlike SRAM and Flash, EEPROM is not addressable
    /// memory - it is a one-bit serial device driven by DMA.
    eeprom_state: EepromState,
}

/// The header's complement check, per GBATEK: subtract every byte from 0xA0 to
/// 0xBC, then subtract 0x19. The result is what byte 0xBD must hold.
///
/// Public so the range itself is testable - it was wrong (starting at 0x0A,
/// which swept in the Nintendo logo and the entry point) and nothing but a
/// warning on stderr said so.
pub fn header_checksum(data: &[u8]) -> u8 {
    let mut sum: u8 = 0;
    for &byte in &data[0xA0..=0xBC] {
        sum = sum.wrapping_sub(byte);
    }
    sum.wrapping_sub(0x19)
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
            flash_id_mode: false,
            eeprom_state: EepromState::new(),
        }
    }

    pub fn from_bytes(data: &[u8]) -> Result<Self, CartError> {
        if data.len() < 0xC0 || data.len() > 32 * 1024 * 1024 {
            return Err(CartError::InvalidSize(data.len()));
        }

        // GBATEK, GBA Cartridge Header: the complement check at 0xBD covers
        // bytes 0xA0 through 0xBC inclusive - the title, game code, maker
        // code and the fields beside them. The range used to start at 0x0A,
        // which swept in the Nintendo logo and the entry point and made every
        // commercial ROM report a bad checksum on load.
        let checksum = header_checksum(data);
        if checksum != data[0xBD] {
            log::warn!(
                "bad header checksum (got 0x{checksum:02X}, expected 0x{:02X})",
                data[0xBD]
            );
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
            flash_id_mode: false,
            eeprom_state: EepromState::new(),
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

    /// Address width in bits: 6 for a 512-byte EEPROM, 14 for an 8 KB one.
    fn eeprom_address_bits(&self) -> usize {
        if self.eeprom.len() > 512 {
            14
        } else {
            6
        }
    }

    /// One serial bit out of the EEPROM, in bit 0 as the hardware presents it.
    pub fn eeprom_read(&mut self) -> u16 {
        let bits = self.eeprom_address_bits();
        self.eeprom_state.read_bit(&self.eeprom, bits)
    }

    /// One serial bit into the EEPROM, taken from bit 0.
    pub fn eeprom_write(&mut self, value: u16) {
        let bits = self.eeprom_address_bits();
        if self
            .eeprom_state
            .write_bit(&mut self.eeprom, bits, value & 1 == 1)
        {
            self.save_dirty = true;
        }
    }

    /// Whether this cartridge uses EEPROM, which is addressed through
    /// `0x0D000000` rather than the usual save region.
    pub fn uses_eeprom(&self) -> bool {
        matches!(self.save_type, SaveType::Eeprom512 | SaveType::Eeprom8k)
    }

    /// Read a byte from save memory
    pub fn save_read(&self, address: u32) -> u8 {
        match self.save_type {
            SaveType::Sram => {
                let addr = (address as usize) & 0x7FFF;
                if addr < self.sram.len() {
                    self.sram[addr]
                } else {
                    0
                }
            }
            SaveType::Flash64 | SaveType::Flash128 => {
                // In ID mode the chip answers with its identity rather than
                // data. Games probe this before writing, and one that gets
                // flash contents back concludes there is no chip and refuses
                // to save. GBATEK: man=[E000000h], dev=[E000001h], with the ID
                // written MSB=device, LSB=manufacturer.
                if self.flash_id_mode {
                    let id: u16 = match self.save_type {
                        // Panasonic 64K, the common 64K part.
                        SaveType::Flash64 => 0x1B32,
                        // Sanyo 128K.
                        _ => 0x1362,
                    };
                    return match address & 1 {
                        0 => (id & 0xFF) as u8,
                        _ => (id >> 8) as u8,
                    };
                }
                let addr = ((self.flash_bank << 16) | (address as usize & 0xFFFF))
                    & (self.flash.len() - 1);
                if addr < self.flash.len() {
                    self.flash[addr]
                } else {
                    0xFF
                }
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
                        if value == 0xAA {
                            self.flash_state = 1;
                        }
                    }
                    1 => {
                        if value == 0x55 {
                            self.flash_state = 2;
                        } else {
                            self.flash_state = 0;
                        }
                    }
                    2 => {
                        match value {
                            0x90 => {
                                // Enter chip ID mode
                                self.flash_id_mode = true;
                                self.flash_state = 0;
                            }
                            0xF0 => {
                                // Terminate chip ID mode
                                self.flash_id_mode = false;
                                self.flash_state = 0;
                            }
                            0xA0 => self.flash_state = 4, // Byte program
                            0xB0 => self.flash_state = 5, // Bank select
                            0x80 => self.flash_state = 6, // Erase
                            _ => self.flash_state = 0,
                        }
                    }
                    4 => {
                        // Byte program
                        let addr = ((self.flash_bank << 16) | (address as usize & 0xFFFF))
                            & (self.flash.len() - 1);
                        if addr < self.flash.len() {
                            self.flash[addr] = value;
                            self.save_dirty = true;
                        }
                        self.flash_state = 0;
                    }
                    5 => {
                        // Bank select
                        self.flash_bank = (value as usize) & 1;
                        self.flash_state = 0;
                    }
                    // Erase needs a SECOND unlock sequence before the command:
                    // the full form is AA,55,80,AA,55, then 0x10 (chip) or 0x30
                    // (sector). Taking the command straight after 0x80 meant the
                    // 0xAA was swallowed here and the erase never ran.
                    6 => {
                        if value == 0xAA {
                            self.flash_state = 7;
                        } else {
                            self.flash_state = 0;
                        }
                    }
                    7 => {
                        if value == 0x55 {
                            self.flash_state = 8;
                        } else {
                            self.flash_state = 0;
                        }
                    }
                    8 => {
                        match value {
                            0x30 => {
                                let addr = ((self.flash_bank << 16) | (address as usize & 0xFFFF))
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

/// The serial EEPROM protocol, per GBATEK's *GBA Cart Backup EEPROM*.
///
/// EEPROM is not addressable memory. It hangs off bit 0 of the data bus and a
/// game talks to it by DMAing a bit stream:
///
/// - **Set read address**: `11`, then 6 or 14 address bits (MSB first), then `0`.
/// - **Read**: 68 bits back - 4 to ignore, then 64 data bits, MSB first.
/// - **Write**: `10`, the address bits, 64 data bits, then `0`.
///
/// Addressing is in units of 64 bits, so an address selects an 8-byte block.
/// Treating the region as byte-addressed RAM, as this emulator did before, is
/// not a simplification of that - it is a different device, and no real game
/// would read back what it wrote.
///
/// A write command runs to 81 bits for a 14-bit address, so this collects the
/// stream in phases rather than one integer.
#[derive(Clone, Copy, PartialEq)]
enum EepromPhase {
    /// Collecting the two opcode bits.
    Opcode,
    /// Collecting address bits; `write` records which opcode we saw.
    Address { write: bool },
    /// Collecting the 64 data bits of a write.
    WriteData,
    /// A read command ends with one "0" bit before the data comes back. It has
    /// to be consumed here: treating it as the start of a new command cancels
    /// the read that was just set up.
    ReadStop,
    /// A write command also ends with a "0". Left unconsumed it lands in the
    /// next command's opcode and desynchronises the whole stream.
    WriteStop,
    /// Streaming 68 bits back out.
    Reading,
}

#[derive(Clone)]
struct EepromState {
    phase: EepromPhase,
    /// Bits accumulated in the current phase.
    acc: u64,
    count: usize,
    addr: usize,
    read_pos: usize,
}

impl EepromState {
    fn new() -> Self {
        Self {
            phase: EepromPhase::Opcode,
            acc: 0,
            count: 0,
            addr: 0,
            read_pos: 0,
        }
    }

    fn reset(&mut self) {
        self.phase = EepromPhase::Opcode;
        self.acc = 0;
        self.count = 0;
    }

    /// Serial output: four bits the game discards, then 64 data bits MSB first.
    fn read_bit(&mut self, eeprom: &[u8], _addr_bits: usize) -> u16 {
        if self.phase != EepromPhase::Reading {
            // Idle, or the game is polling for a write to finish. Writes here
            // complete instantly, so report ready.
            return 1;
        }
        let pos = self.read_pos;
        self.read_pos += 1;
        if self.read_pos >= 68 {
            self.read_pos = 0;
            self.reset();
        }
        if pos < 4 {
            return 0;
        }
        let bit_index = pos - 4;
        let byte = self.addr * 8 + bit_index / 8;
        if byte >= eeprom.len() {
            return 1;
        }
        ((eeprom[byte] >> (7 - (bit_index % 8))) & 1) as u16
    }

    /// Serial input. Returns true when a write actually modified the chip.
    fn write_bit(&mut self, eeprom: &mut [u8], addr_bits: usize, bit: bool) -> bool {
        match self.phase {
            EepromPhase::Reading => {
                // A new command while a read is outstanding: abandon it.
                self.read_pos = 0;
                self.reset();
                self.write_bit(eeprom, addr_bits, bit)
            }
            EepromPhase::Opcode => {
                self.acc = (self.acc << 1) | bit as u64;
                self.count += 1;
                if self.count == 2 {
                    let write = match self.acc {
                        0b11 => false,
                        0b10 => true,
                        // Not a command; wait for a clean start.
                        _ => {
                            self.reset();
                            return false;
                        }
                    };
                    self.phase = EepromPhase::Address { write };
                    self.acc = 0;
                    self.count = 0;
                }
                false
            }
            EepromPhase::Address { write } => {
                self.acc = (self.acc << 1) | bit as u64;
                self.count += 1;
                if self.count == addr_bits {
                    // Only the low bits address the array; GBATEK notes the
                    // upper 4 of a 14-bit address should be zero.
                    self.addr = (self.acc as usize) & (eeprom.len() / 8 - 1);
                    self.acc = 0;
                    self.count = 0;
                    self.phase = if write {
                        EepromPhase::WriteData
                    } else {
                        EepromPhase::ReadStop
                    };
                }
                false
            }
            EepromPhase::WriteStop => {
                // Programming completes instantly here, so nothing to wait for.
                self.reset();
                false
            }
            EepromPhase::ReadStop => {
                // Whatever this bit is, the address is set and the data comes
                // out next.
                self.phase = EepromPhase::Reading;
                self.read_pos = 0;
                false
            }
            EepromPhase::WriteData => {
                self.acc = (self.acc << 1) | bit as u64;
                self.count += 1;
                if self.count == 64 {
                    let base = self.addr * 8;
                    let dirty = if base + 8 <= eeprom.len() {
                        for i in 0..8 {
                            eeprom[base + i] = (self.acc >> (56 - i * 8)) as u8;
                        }
                        true
                    } else {
                        false
                    };
                    self.acc = 0;
                    self.count = 0;
                    self.phase = EepromPhase::WriteStop;
                    return dirty;
                }
                false
            }
        }
    }
}
