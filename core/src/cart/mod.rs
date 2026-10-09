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
    /// Address width of the command currently being clocked in, taken from
    /// the length of the DMA that carries it. `None` until a DMA says.
    eeprom_addr_bits: Option<usize>,
    /// Whether this is an 8 KB chip rather than a 512-byte one.
    ///
    /// Starts as a guess from the ROM size and is **corrected by the game**:
    /// a 14-bit command proves 8 KB, and so does an 8 KB `.sav` on disk. The
    /// guess alone was the bug - a cart of 16 MB or less with a 64 Kbit chip
    /// was given 512 bytes, its 14-bit addresses masked to 6, and block 100
    /// written over block 36. The Minish Cap is exactly that cart.
    eeprom_large: bool,
    /// CRC-32 of the ROM image, taken once at load. A save state records it so
    /// a state from another ROM - a hack sharing its original's header - is
    /// refused instead of replacing this cart's battery save.
    rom_crc: u32,
}

/// CRC-32 (IEEE, the zip/`java.util.zip.CRC32` one), table built at compile
/// time. Hand-rolled because the core takes no dependency it can avoid.
pub(crate) fn crc32(data: &[u8]) -> u32 {
    const TABLE: [u32; 256] = {
        let mut table = [0u32; 256];
        let mut i = 0;
        while i < 256 {
            let mut c = i as u32;
            let mut k = 0;
            while k < 8 {
                c = if c & 1 != 0 {
                    0xEDB8_8320 ^ (c >> 1)
                } else {
                    c >> 1
                };
                k += 1;
            }
            table[i] = c;
            i += 1;
        }
        table
    };
    !data.iter().fold(!0u32, |c, &b| {
        TABLE[((c ^ u32::from(b)) & 0xFF) as usize] ^ (c >> 8)
    })
}

/// Bytes of [`Cartridge::chip_state`] in a save state.
pub(crate) const CHIP_STATE_LEN: usize = 18;

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
            eeprom_addr_bits: None,
            eeprom_large: false,
            rom_crc: crc32(&[]),
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
        // Always backed by 8 KB, whichever chip the header suggests. The size
        // a game actually uses is learned from its own commands - see
        // `eeprom_large` - and storage that is already big enough means that
        // discovery can never alias one block onto another.
        let eeprom_size = match save_type {
            SaveType::Eeprom8k | SaveType::Eeprom512 => 8 * 1024,
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
            eeprom_addr_bits: None,
            eeprom_large: save_type == SaveType::Eeprom8k,
            rom_crc: crc32(data),
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

    pub fn is_empty(&self) -> bool {
        self.rom.is_empty()
    }

    /// Address width in bits: 6 for a 512-byte EEPROM, 14 for an 8 KB one.
    ///
    /// The chip size is only a fallback. What actually decides it is the
    /// length of the DMA carrying the command, because a game detects the
    /// chip by trying both widths and believing whichever answers - so a
    /// cartridge with an 8 KB part is still addressed with 6 bits during
    /// detection, and hardcoding 14 desynchronises the whole bit stream.
    fn eeprom_address_bits(&self) -> usize {
        self.eeprom_addr_bits
            .unwrap_or(if self.eeprom_large { 14 } else { 6 })
    }

    /// Told by the DMA unit how long the transfer driving the EEPROM is, in
    /// halfwords, before a single bit is clocked.
    ///
    /// GBATEK, *GBA Cart Backup EEPROM*: a set-address command is 2 opcode
    /// bits + address + 1 stop, and a write is that plus 64 data bits. So the
    /// length names the address width outright:
    ///
    /// | words | command | address bits |
    /// | --- | --- | --- |
    /// | 9  | set read address | 6 |
    /// | 17 | set read address | 14 |
    /// | 73 | write | 6 |
    /// | 81 | write | 14 |
    ///
    /// A 68-word read-back carries no address, so it says nothing and is
    /// left alone.
    pub fn eeprom_begin_dma(&mut self, words: usize) {
        match words {
            9 | 73 => self.eeprom_addr_bits = Some(6),
            17 | 81 => {
                self.eeprom_addr_bits = Some(14);
                // Sticky. A game probes with 6 bits first and settles on 14,
                // so a 6-bit command after this one is detection, not proof
                // the chip shrank.
                self.eeprom_large = true;
            }
            _ => {}
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

    /// First address of the EEPROM window.
    ///
    /// GBATEK, *GBA Cart Backup EEPROM*: the chip answers anywhere in
    /// `0x0D000000-0x0DFFFFFF` **only** on a cartridge of 16 MB or less. Past
    /// that the ROM itself reaches into the same addresses through the WS2
    /// mirror, and the window shrinks to the last 256 bytes.
    ///
    /// Getting this wrong is not a subtle timing issue: on a 32 MB ROM the
    /// whole upper half of the cartridge reads back as EEPROM bits instead of
    /// code, and every command the game sends lands in a state machine that
    /// the ROM reads have already desynchronised.
    pub fn eeprom_window_start(&self) -> u32 {
        if self.rom.len() > 16 * 1024 * 1024 {
            0x0DFF_FF00
        } else {
            0x0D00_0000
        }
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
            // The chip the game has shown it has, not the header's guess, so an
            // existing `.sav` keeps the size it was written at.
            SaveType::Eeprom512 | SaveType::Eeprom8k => Some(if self.eeprom_large {
                self.eeprom.clone()
            } else {
                self.eeprom[..512].to_vec()
            }),
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
                // A save file bigger than 512 bytes can only have come from an
                // 8 KB chip. Without this an 8 KB `.sav` was truncated on load.
                if data.len() > 512 {
                    self.eeprom_large = true;
                }
                let len = data.len().min(self.eeprom.len());
                self.eeprom[..len].copy_from_slice(&data[..len]);
            }
            SaveType::None => {}
        }
    }

    /// The save chip's volatile state - Flash command state, bank and ID
    /// mode, and where the EEPROM's serial protocol stands - for a save state.
    /// `save_data` carries only the memory; without this a state taken
    /// mid-command resumed with the chip idle and bank 0 selected.
    /// CRC-32 of the loaded ROM image.
    pub(crate) fn rom_crc(&self) -> u32 {
        self.rom_crc
    }

    pub(crate) fn chip_state(&self) -> [u8; CHIP_STATE_LEN] {
        let e = &self.eeprom_state;
        let phase = match e.phase {
            EepromPhase::Opcode => 0,
            EepromPhase::Address { write: false } => 1,
            EepromPhase::Address { write: true } => 2,
            EepromPhase::WriteData => 3,
            EepromPhase::ReadStop => 4,
            EepromPhase::WriteStop => 5,
            EepromPhase::Reading => 6,
        };
        let mut b = [0u8; CHIP_STATE_LEN];
        b[0] = self.flash_state;
        b[1] = self.flash_bank as u8;
        b[2] = self.flash_id_mode as u8;
        b[3] = self.eeprom_addr_bits.unwrap_or(0) as u8;
        b[4] = self.eeprom_large as u8;
        b[5] = phase;
        b[6] = e.count as u8;
        b[7..9].copy_from_slice(&(e.addr as u16).to_le_bytes());
        b[9] = e.read_pos as u8;
        b[10..18].copy_from_slice(&e.acc.to_le_bytes());
        b
    }

    /// Inverse of [`Self::chip_state`]. The bytes come from a file, so every
    /// field is range-checked against what the chip can actually hold, and
    /// nothing is applied unless all of them pass.
    pub(crate) fn restore_chip_state(
        &mut self,
        b: &[u8; CHIP_STATE_LEN],
    ) -> Result<(), &'static str> {
        let phase = match b[5] {
            0 => EepromPhase::Opcode,
            1 => EepromPhase::Address { write: false },
            2 => EepromPhase::Address { write: true },
            3 => EepromPhase::WriteData,
            4 => EepromPhase::ReadStop,
            5 => EepromPhase::WriteStop,
            6 => EepromPhase::Reading,
            _ => return Err("EEPROM phase"),
        };
        let addr = u16::from_le_bytes([b[7], b[8]]) as usize;
        if !matches!(b[0], 0..=2 | 4..=8) {
            return Err("flash command state");
        }
        if b[1] > 1 || b[2] > 1 || b[4] > 1 {
            return Err("flash bank or flag");
        }
        if !matches!(b[3], 0 | 6 | 14) {
            return Err("EEPROM address width");
        }
        // 8 KB is the largest chip: 1024 blocks of 8 bytes, 68 read-back bits.
        if b[6] > 64 || addr >= 1024 || b[9] >= 68 {
            return Err("EEPROM serial state");
        }
        // Each counting phase ends when `count` reaches its length, compared
        // with `==`: a count already there never matches again and the chip
        // stays wedged. The other phases do not count at all.
        let addr_bits = match (b[3], b[4]) {
            (0, 0) => 6,
            (0, _) => 14,
            (bits, _) => bits,
        };
        let count_ok = match phase {
            EepromPhase::Opcode => b[6] < 2,
            EepromPhase::Address { .. } => b[6] < addr_bits,
            EepromPhase::WriteData => b[6] < 64,
            _ => b[6] == 0,
        };
        if !count_ok {
            return Err("EEPROM count past its phase");
        }
        self.flash_state = b[0];
        self.flash_bank = b[1] as usize;
        self.flash_id_mode = b[2] != 0;
        self.eeprom_addr_bits = (b[3] != 0).then_some(b[3] as usize);
        self.eeprom_large = b[4] != 0;
        self.eeprom_state = EepromState {
            phase,
            acc: u64::from_le_bytes(b[10..18].try_into().unwrap()),
            count: b[6] as usize,
            addr,
            read_pos: b[9] as usize,
        };
        Ok(())
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

    // Scan ROM for save type strings. A byte-window search, not
    // `String::from_utf8_lossy(rom)`: a ROM is never valid UTF-8, so that
    // allocated a fresh String with every invalid byte expanded to a 3-byte
    // replacement char - up to ~96 MB transiently for a 32 MB cart.
    let contains = |pat: &str| rom.windows(pat.len()).any(|w| w == pat.as_bytes());
    if contains("SRAM_V") {
        SaveType::Sram
    } else if contains("FLASH_V") || contains("FLASH512_V") {
        SaveType::Flash64
    } else if contains("FLASH1M_V") {
        SaveType::Flash128
    } else if contains("EEPROM_V") {
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

#[cfg(test)]
mod tests {
    /// The standard CRC-32 check value, so the identity matches what any
    /// other tool (zip, `java.util.zip.CRC32`) computes for the same ROM.
    #[test]
    fn crc32_matches_the_standard_check_value() {
        assert_eq!(super::crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(super::crc32(&[]), 0);
    }
}
