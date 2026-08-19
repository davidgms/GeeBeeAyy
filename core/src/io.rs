// GBA I/O Register Addresses
// Reference: GBATEK Section 9 (I/O Map)

// --- Display ---
pub const DISPCNT: u32 = 0x0400_0000;
pub const DISPSTAT: u32 = 0x0400_0004;
pub const VCOUNT: u32 = 0x0400_0006;

// --- BG Control ---
pub const BG0CNT: u32 = 0x0400_0008;
pub const BG1CNT: u32 = 0x0400_000A;
pub const BG2CNT: u32 = 0x0400_000C;
pub const BG3CNT: u32 = 0x0400_000E;

// --- BG Scroll ---
pub const BG0HOFS: u32 = 0x0400_0010;
pub const BG0VOFS: u32 = 0x0400_0012;
pub const BG1HOFS: u32 = 0x0400_0014;
pub const BG1VOFS: u32 = 0x0400_0016;
pub const BG2HOFS: u32 = 0x0400_0018;
pub const BG2VOFS: u32 = 0x0400_001A;
pub const BG3HOFS: u32 = 0x0400_001C;
pub const BG3VOFS: u32 = 0x0400_001E;

// --- BG2/3 Rotation/Scaling ---
pub const BG2PA: u32 = 0x0400_0020;
pub const BG2PB: u32 = 0x0400_0022;
pub const BG2PC: u32 = 0x0400_0024;
pub const BG2PD: u32 = 0x0400_0026;
pub const BG2X_L: u32 = 0x0400_0028;
pub const BG2X_H: u32 = 0x0400_002A;
pub const BG2Y_L: u32 = 0x0400_002C;
pub const BG2Y_H: u32 = 0x0400_002E;
pub const BG3PA: u32 = 0x0400_0030;
pub const BG3PB: u32 = 0x0400_0032;
pub const BG3PC: u32 = 0x0400_0034;
pub const BG3PD: u32 = 0x0400_0036;
pub const BG3X_L: u32 = 0x0400_0038;
pub const BG3X_H: u32 = 0x0400_003A;
pub const BG3Y_L: u32 = 0x0400_003C;
pub const BG3Y_H: u32 = 0x0400_003E;

// --- Window ---
pub const WIN0H: u32 = 0x0400_0040;
pub const WIN1H: u32 = 0x0400_0042;
pub const WIN0V: u32 = 0x0400_0044;
pub const WIN1V: u32 = 0x0400_0046;
pub const WININ: u32 = 0x0400_0048;
pub const WINOUT: u32 = 0x0400_004A;

// --- Mosaic ---
pub const MOSAIC: u32 = 0x0400_004C;

// --- Color Special ---
pub const BLDCNT: u32 = 0x0400_0050;
pub const BLDALPHA: u32 = 0x0400_0052;
pub const BLDY: u32 = 0x0400_0054;

// --- Sound ---
pub const SOUND1CNT_L: u32 = 0x0400_0060;
pub const SOUND1CNT_H: u32 = 0x0400_0062;
pub const SOUND1CNT_X: u32 = 0x0400_0064;
pub const SOUND2CNT_L: u32 = 0x0400_0068;
pub const SOUND2CNT_H: u32 = 0x0400_006C;
pub const SOUND2CNT_X: u32 = 0x0400_0070;
pub const SOUND3CNT_L: u32 = 0x0400_0070;
pub const SOUND3CNT_H: u32 = 0x0400_0072;
pub const SOUND3CNT_X: u32 = 0x0400_0074;
pub const SOUND4CNT_L: u32 = 0x0400_0078;
pub const SOUND4CNT_H: u32 = 0x0400_007C;
pub const SOUNDCNT_L: u32 = 0x0400_0080;
pub const SOUNDCNT_H: u32 = 0x0400_0082;
pub const SOUNDCNT_X: u32 = 0x0400_0084;
pub const SOUNDBIAS: u32 = 0x0400_0088;
pub const WAVE_RAM: u32 = 0x0400_0090;
pub const FIFO_A: u32 = 0x0400_00A0;
pub const FIFO_B: u32 = 0x0400_00A4;

// --- DMA ---
pub const DMA0SAD: u32 = 0x0400_00B0;
pub const DMA0DAD: u32 = 0x0400_00B4;
pub const DMA0CNT_L: u32 = 0x0400_00B8;
pub const DMA0CNT_H: u32 = 0x0400_00BA;
pub const DMA1SAD: u32 = 0x0400_00BC;
pub const DMA1DAD: u32 = 0x0400_00C0;
pub const DMA1CNT_L: u32 = 0x0400_00C4;
pub const DMA1CNT_H: u32 = 0x0400_00C6;
pub const DMA2SAD: u32 = 0x0400_00C8;
pub const DMA2DAD: u32 = 0x0400_00CC;
pub const DMA2CNT_L: u32 = 0x0400_00D0;
pub const DMA2CNT_H: u32 = 0x0400_00D2;
pub const DMA3SAD: u32 = 0x0400_00D4;
pub const DMA3DAD: u32 = 0x0400_00D8;
pub const DMA3CNT_L: u32 = 0x0400_00DC;
pub const DMA3CNT_H: u32 = 0x0400_00DE;

// --- Timer ---
pub const TM0CNT_L: u32 = 0x0400_0100;
pub const TM0CNT_H: u32 = 0x0400_0102;
pub const TM1CNT_L: u32 = 0x0400_0104;
pub const TM1CNT_H: u32 = 0x0400_0106;
pub const TM2CNT_L: u32 = 0x0400_0108;
pub const TM2CNT_H: u32 = 0x0400_010A;
pub const TM3CNT_L: u32 = 0x0400_010C;
pub const TM3CNT_H: u32 = 0x0400_010E;

// --- Key Input ---
pub const KEYINPUT: u32 = 0x0400_0130;
pub const KEYCNT: u32 = 0x0400_0132;

// --- Interrupts ---
pub const IE: u32 = 0x0400_0200;
pub const IF: u32 = 0x0400_0202;
pub const IME: u32 = 0x0400_0208;

// --- Wait States ---
pub const WAITCNT: u32 = 0x0400_0204;

// --- Post Boot ---
pub const POSTFLG: u32 = 0x0400_0300;
pub const HALTCNT: u32 = 0x0400_0301;

// --- PPU I/O register offsets (relative to 0x04000000) ---
pub const IO_DISPCNT: usize = 0x000;
pub const IO_DISPSTAT: usize = 0x004;
pub const IO_VCOUNT: usize = 0x006;
pub const IO_BG0CNT: usize = 0x008;
pub const IO_BG1CNT: usize = 0x00A;
pub const IO_BG2CNT: usize = 0x00C;
pub const IO_BG3CNT: usize = 0x00E;
pub const IO_BG0HOFS: usize = 0x010;
pub const IO_BG0VOFS: usize = 0x012;
pub const IO_BG1HOFS: usize = 0x014;
pub const IO_BG1VOFS: usize = 0x016;
pub const IO_BG2HOFS: usize = 0x018;
pub const IO_BG2VOFS: usize = 0x01A;
pub const IO_BG3HOFS: usize = 0x01C;
pub const IO_BG3VOFS: usize = 0x01E;

// --- Timer I/O register offsets ---
pub const IO_TM0CNT_L: usize = 0x100;
pub const IO_TM0CNT_H: usize = 0x102;
pub const IO_TM1CNT_L: usize = 0x104;
pub const IO_TM1CNT_H: usize = 0x106;
pub const IO_TM2CNT_L: usize = 0x108;
pub const IO_TM2CNT_H: usize = 0x10A;
pub const IO_TM3CNT_L: usize = 0x10C;
pub const IO_TM3CNT_H: usize = 0x10E;

// --- Interrupt I/O register offsets ---
pub const IO_IE: usize = 0x200;
pub const IO_IF: usize = 0x202;
pub const IO_IME: usize = 0x208;
pub const IO_WAITCNT: usize = 0x204;
pub const IO_HALTCNT: usize = 0x301;

/// I/O register handler — decodes reads/writes to hardware registers.
pub struct IoHandler {
    pub ie: u16,
    pub if_: u16,
    pub ime: u16,
    pub halt: bool,
}

impl IoHandler {
    pub fn new() -> Self {
        Self {
            ie: 0,
            if_: 0,
            ime: 0,
            halt: false,
        }
    }

    /// Handle a 16-bit I/O read at the given offset (relative to 0x04000000).
    pub fn read16(&self, offset: usize) -> u16 {
        match offset {
            IO_IE => self.ie,
            IO_IF => self.if_,
            IO_IME => self.ime,
            _ => 0,
        }
    }

    /// Handle a 16-bit I/O write at the given offset (relative to 0x04000000).
    pub fn write16(&mut self, offset: usize, value: u16) {
        match offset {
            IO_IE => self.ie = value,
            IO_IF => self.if_ &= !value, // W1C: writing 1 clears the bit
            IO_IME => self.ime = value & 1,
            _ => {}
        }
    }

    /// Check if an interrupt is pending (IE & IF & IME).
    pub fn interrupt_pending(&self) -> bool {
        self.ime & 1 == 1 && self.ie & self.if_ != 0
    }

    /// Request an interrupt by setting a bit in IF.
    pub fn request_interrupt(&mut self, bit: u16) {
        self.if_ |= bit;
    }
}
