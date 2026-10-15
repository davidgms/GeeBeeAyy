use std::io::{self, Cursor, Read};

use super::Gba;

/// Save state magic and version
const SAVE_MAGIC: &[u8; 4] = b"GBAS";
/// v2 added the Supervisor, Abort, Undefined and User register banks.
/// v3 fixed the timers (counter and reload were swapped, and the control
/// registers were absent so every timer came back disabled), rebuilt DMA
/// derived state instead of assigning the raw control word, and added the I/O
/// register file and the cartridge's battery save.
/// v4 added the APU, so sound continues across a load instead of restarting
/// from silence.
/// v7 appended the save chip's command state (Flash state machine, bank and
/// ID mode; EEPROM serial phase and address). v6 still loads, leaving the
/// chip as it was.
/// v8 appended the four DMA channel latches (what a DMA read of the BIOS
/// returns). v6 and v7 still load, with the latches cleared.
/// v9 appended the cycles left in a serial transfer. Older states load with
/// no transfer running.
/// v10 appended the CRC-32 of the ROM the state was taken on, and a state
/// for another ROM is refused. Older states carry none and load as before.
/// v11 appended the live state a running machine carries between two steps:
/// the PPU's dot within the scanline and whether it has drawn the line, the
/// affine reference accumulators, the HLE BIOS wait flags, the BIOS read
/// latch, and the cycles owed to the next step (exception entry, a DMA stall,
/// the IRQ delay). Before it a restored machine kept its own values for
/// these. Older states load with them derived from the registers or reset as
/// at power-on.
const SAVE_VERSION: u32 = 11;

/// More cycles than any legitimate value of a counter owed to the next step
/// or of the PPU's dot counter can reach (the longest DMA is ~2^21 cycles).
/// A state past it is hostile: added to on every step, it would overflow.
const MAX_LIVE_CYCLES: u32 = 1 << 24;

/// The affine accumulators start from a 28-bit register and gain at most 160
/// lines of a 16-bit PB/PD before the next reload, so they stay well inside
/// +-2^28; `affine_bg_rgb` adds PA/PC times 240 to them without wrapping.
const MAX_AFFINE: i32 = 1 << 28;

/// Bytes the v8 DMA latch block takes at the end of a state.
#[cfg(test)]
const DMA_LATCHES_LEN: usize = 16;

/// Bytes the v9 serial transfer counter takes at the end of a state.
#[cfg(test)]
const SIO_LEN: usize = 4;

/// Bytes the v10 ROM identity takes at the end of a state.
#[cfg(test)]
const ROM_ID_LEN: usize = 4;

/// Bytes the v11 live state takes at the end of a state.
#[cfg(test)]
const LIVE_LEN: usize = 4 + 1 + 16 + 2 + 4 + 4 + 4 + 8;

/// Save state snapshot of the entire GBA emulator.
pub struct SaveState {
    pub data: Vec<u8>,
}

impl SaveState {
    /// Create a save state from the current GBA state.
    pub fn create(gba: &Gba) -> Self {
        let mut buf = Vec::with_capacity(512 * 1024); // 512KB initial

        // Header
        buf.extend_from_slice(SAVE_MAGIC);
        buf.extend_from_slice(&SAVE_VERSION.to_le_bytes());

        // CPU state
        write_u32_array(&mut buf, &gba.cpu.registers);
        write_u32_array(&mut buf, &gba.cpu.fiq_registers);
        write_u32_array(&mut buf, &gba.cpu.irq_registers);
        write_u32_array(&mut buf, &gba.cpu.svc_registers);
        write_u32_array(&mut buf, &gba.cpu.abt_registers);
        write_u32_array(&mut buf, &gba.cpu.und_registers);
        write_u32_array(&mut buf, &gba.cpu.usr_registers);
        write_u32_array(&mut buf, &gba.cpu.usr_r8_r12);
        write_u32(&mut buf, gba.cpu.cpsr);
        write_u32(&mut buf, gba.cpu.spsr_fiq);
        write_u32(&mut buf, gba.cpu.spsr_irq);
        write_u32(&mut buf, gba.cpu.spsr_svc);
        write_u32(&mut buf, gba.cpu.spsr_abt);
        write_u32(&mut buf, gba.cpu.spsr_und);
        write_bool(&mut buf, gba.cpu.halted);

        // PPU state
        write_u8(&mut buf, gba.ppu.bg_mode);
        write_u16(&mut buf, gba.ppu.dispcnt);
        write_u16(&mut buf, gba.ppu.scanline);
        write_bool(&mut buf, gba.ppu.vblank);
        write_bool(&mut buf, gba.ppu.hblank);
        write_u16(&mut buf, gba.ppu.bg0cnt);
        write_u16(&mut buf, gba.ppu.bg1cnt);
        write_u16(&mut buf, gba.ppu.bg2cnt);
        write_u16(&mut buf, gba.ppu.bg3cnt);
        write_u16(&mut buf, gba.ppu.bg0hofs);
        write_u16(&mut buf, gba.ppu.bg0vofs);
        write_u16(&mut buf, gba.ppu.bg1hofs);
        write_u16(&mut buf, gba.ppu.bg1vofs);
        write_u16(&mut buf, gba.ppu.bg2hofs);
        write_u16(&mut buf, gba.ppu.bg2vofs);
        write_u16(&mut buf, gba.ppu.bg3hofs);
        write_u16(&mut buf, gba.ppu.bg3vofs);
        write_i32(&mut buf, gba.ppu.bg2x);
        write_i32(&mut buf, gba.ppu.bg2y);
        write_i32(&mut buf, gba.ppu.bg3x);
        write_i32(&mut buf, gba.ppu.bg3y);
        write_i16(&mut buf, gba.ppu.bg2pa);
        write_i16(&mut buf, gba.ppu.bg2pb);
        write_i16(&mut buf, gba.ppu.bg2pc);
        write_i16(&mut buf, gba.ppu.bg2pd);
        write_i16(&mut buf, gba.ppu.bg3pa);
        write_i16(&mut buf, gba.ppu.bg3pb);
        write_i16(&mut buf, gba.ppu.bg3pc);
        write_i16(&mut buf, gba.ppu.bg3pd);
        write_u16(&mut buf, gba.ppu.bldcnt);
        write_u16(&mut buf, gba.ppu.bldalpha);
        write_u8(&mut buf, gba.ppu.bldy);
        buf.extend_from_slice(gba.ppu.frame_buffer());

        // Timer state. v2 wrote only the counter and restored it *as the
        // reload*, and carried no control registers at all, so every timer came
        // back disabled - which kills DMA sound and every cascade.
        for i in 0..4 {
            write_u32(&mut buf, gba.timer.counters[i]);
            write_u32(&mut buf, gba.timer.reloads[i]);
            write_u16(&mut buf, gba.timer.controls[i]);
            write_u32(&mut buf, gba.timer.prescaler[i]);
            write_u32(&mut buf, gba.timer.tick_counters[i]);
            write_bool(&mut buf, gba.timer.enabled[i]);
            write_bool(&mut buf, gba.timer.cascaded[i]);
            write_bool(&mut buf, gba.timer.irq_enabled[i]);
        }

        // DMA state
        for i in 0..4 {
            let ch = &gba.dma.channels[i];
            write_u32(&mut buf, ch.source);
            write_u32(&mut buf, ch.dest);
            write_u16(&mut buf, ch.count);
            write_u16(&mut buf, ch.control);
            write_bool(&mut buf, ch.enabled);
        }

        // I/O state
        write_u16(&mut buf, gba.bus.io.ie);
        write_u16(&mut buf, gba.bus.io.if_);
        write_u16(&mut buf, gba.bus.io.ime);
        write_bool(&mut buf, gba.bus.io.halt);

        write_u16(&mut buf, gba.bus.waitcnt());
        // The whole I/O register file. Absent from v2, so DISPCNT, the
        // background scroll registers and everything else came back as
        // whatever the fresh instance happened to hold.
        buf.extend_from_slice(gba.bus.io_regs_data());
        buf.extend_from_slice(gba.bus.ewram_data());
        buf.extend_from_slice(gba.bus.iwram_data());
        buf.extend_from_slice(gba.bus.palette_data());
        buf.extend_from_slice(gba.bus.vram_data());
        buf.extend_from_slice(gba.bus.oam_data());

        // APU. Without this, every channel came back silent until the game
        // next wrote a sound register - a sustained note simply stopped.
        let apu = gba.apu.snapshot();
        write_u32(&mut buf, apu.len() as u32);
        buf.extend_from_slice(&apu);

        // The cartridge's battery save, so a state rolls the .sav back with it.
        match gba.save_data() {
            Some(save) => {
                write_u32(&mut buf, save.len() as u32);
                buf.extend_from_slice(&save);
            }
            None => write_u32(&mut buf, 0),
        }

        // Global state
        write_u64(&mut buf, gba.cycles);

        buf.extend_from_slice(&gba.bus.cart.chip_state());

        for ch in &gba.dma.channels {
            write_u32(&mut buf, ch.latch);
        }

        write_u32(&mut buf, gba.bus.sio_cycles);

        write_u32(&mut buf, gba.bus.cart.rom_crc());

        // v11: live state between two steps.
        let ppu = &gba.ppu;
        write_u32(&mut buf, ppu.cycle_counter);
        write_bool(&mut buf, ppu.line_rendered);
        for v in [
            ppu.bg2x_internal,
            ppu.bg2y_internal,
            ppu.bg3x_internal,
            ppu.bg3y_internal,
        ] {
            write_i32(&mut buf, v);
        }
        write_bool(&mut buf, gba.bus.io.intr_wait_active);
        write_bool(&mut buf, gba.bus.io.halt_swi);
        write_u32(&mut buf, gba.bus.bios_latch);
        write_u32(&mut buf, gba.cpu.entry_cycles);
        write_u32(&mut buf, gba.dma_stall);
        write_u64(&mut buf, gba.irq_since.unwrap_or(u64::MAX));

        SaveState { data: buf }
    }

    /// Restore GBA state from a save state.
    /// Restore, rolling back if the data turns out to be bad.
    ///
    /// `restore_unchecked` writes into the live machine as it parses, so a
    /// truncated or corrupt state used to leave a hybrid of two machines while
    /// returning `Err` - the frontend reported failure and carried on running
    /// something that was not a valid GBA, which then surfaced as a game bug
    /// minutes later. Snapshotting first costs one allocation and makes a
    /// failed load a no-op.
    pub fn restore(&self, gba: &mut Gba) -> Result<(), SaveStateError> {
        let rollback = SaveState::create(gba);
        match self.restore_unchecked(gba) {
            Ok(()) => Ok(()),
            Err(e) => {
                // If the rollback itself fails the machine is unrecoverable
                // either way; report the original error, which is the useful one.
                let _ = rollback.restore_unchecked(gba);
                Err(e)
            }
        }
    }

    fn restore_unchecked(&self, gba: &mut Gba) -> Result<(), SaveStateError> {
        let mut cursor = Cursor::new(&self.data);

        // Header
        let mut magic = [0u8; 4];
        cursor.read_exact(&mut magic)?;
        if magic != *SAVE_MAGIC {
            return Err(SaveStateError::InvalidMagic);
        }
        let version = read_u32(&mut cursor)?;
        if !(6..=SAVE_VERSION).contains(&version) {
            return Err(SaveStateError::UnsupportedVersion(version));
        }

        // CPU
        read_u32_array(&mut cursor, &mut gba.cpu.registers)?;
        read_u32_array(&mut cursor, &mut gba.cpu.fiq_registers)?;
        read_u32_array(&mut cursor, &mut gba.cpu.irq_registers)?;
        read_u32_array(&mut cursor, &mut gba.cpu.svc_registers)?;
        read_u32_array(&mut cursor, &mut gba.cpu.abt_registers)?;
        read_u32_array(&mut cursor, &mut gba.cpu.und_registers)?;
        read_u32_array(&mut cursor, &mut gba.cpu.usr_registers)?;
        read_u32_array(&mut cursor, &mut gba.cpu.usr_r8_r12)?;
        gba.cpu.cpsr = read_u32(&mut cursor)?;
        gba.cpu.spsr_fiq = read_u32(&mut cursor)?;
        gba.cpu.spsr_irq = read_u32(&mut cursor)?;
        gba.cpu.spsr_svc = read_u32(&mut cursor)?;
        gba.cpu.spsr_abt = read_u32(&mut cursor)?;
        gba.cpu.spsr_und = read_u32(&mut cursor)?;
        gba.cpu.halted = read_bool(&mut cursor)?;

        // PPU
        gba.ppu.bg_mode = read_u8(&mut cursor)?;
        gba.ppu.dispcnt = read_u16(&mut cursor)?;
        gba.ppu.scanline = read_u16(&mut cursor)?;
        if gba.ppu.scanline >= 228 {
            return Err(SaveStateError::Corrupt("scanline past 227"));
        }
        gba.ppu.vblank = read_bool(&mut cursor)?;
        gba.ppu.hblank = read_bool(&mut cursor)?;
        gba.ppu.bg0cnt = read_u16(&mut cursor)?;
        gba.ppu.bg1cnt = read_u16(&mut cursor)?;
        gba.ppu.bg2cnt = read_u16(&mut cursor)?;
        gba.ppu.bg3cnt = read_u16(&mut cursor)?;
        gba.ppu.bg0hofs = read_u16(&mut cursor)?;
        gba.ppu.bg0vofs = read_u16(&mut cursor)?;
        gba.ppu.bg1hofs = read_u16(&mut cursor)?;
        gba.ppu.bg1vofs = read_u16(&mut cursor)?;
        gba.ppu.bg2hofs = read_u16(&mut cursor)?;
        gba.ppu.bg2vofs = read_u16(&mut cursor)?;
        gba.ppu.bg3hofs = read_u16(&mut cursor)?;
        gba.ppu.bg3vofs = read_u16(&mut cursor)?;
        gba.ppu.bg2x = read_i32(&mut cursor)?;
        gba.ppu.bg2y = read_i32(&mut cursor)?;
        gba.ppu.bg3x = read_i32(&mut cursor)?;
        gba.ppu.bg3y = read_i32(&mut cursor)?;
        gba.ppu.bg2pa = read_i16(&mut cursor)?;
        gba.ppu.bg2pb = read_i16(&mut cursor)?;
        gba.ppu.bg2pc = read_i16(&mut cursor)?;
        gba.ppu.bg2pd = read_i16(&mut cursor)?;
        gba.ppu.bg3pa = read_i16(&mut cursor)?;
        gba.ppu.bg3pb = read_i16(&mut cursor)?;
        gba.ppu.bg3pc = read_i16(&mut cursor)?;
        gba.ppu.bg3pd = read_i16(&mut cursor)?;
        gba.ppu.bldcnt = read_u16(&mut cursor)?;
        gba.ppu.bldalpha = read_u16(&mut cursor)?;
        gba.ppu.bldy = read_u8(&mut cursor)?;
        cursor.read_exact(gba.ppu.frame_buffer_mut())?;

        // Timer
        for i in 0..4 {
            gba.timer.counters[i] = read_u32(&mut cursor)?;
            gba.timer.reloads[i] = read_u32(&mut cursor)?;
            // Both are 16-bit registers. A reload of 0x10000 divided by zero
            // in `counter_after` on every step; past that it underflowed.
            if gba.timer.counters[i] > 0xFFFF || gba.timer.reloads[i] > 0xFFFF {
                return Err(SaveStateError::Corrupt("timer counter or reload"));
            }
            gba.timer.controls[i] = read_u16(&mut cursor)?;
            gba.timer.prescaler[i] = read_u32(&mut cursor)?;
            // The four the hardware has (GBATEK, TMxCNT_H bits 0-1). Zero is
            // the dangerous one: `while tick >= 0` in Timer::tick never ends.
            if !matches!(gba.timer.prescaler[i], 1 | 64 | 256 | 1024) {
                return Err(SaveStateError::Corrupt("timer prescaler"));
            }
            gba.timer.tick_counters[i] = read_u32(&mut cursor)?;
            gba.timer.enabled[i] = read_bool(&mut cursor)?;
            gba.timer.cascaded[i] = read_bool(&mut cursor)?;
            gba.timer.irq_enabled[i] = read_bool(&mut cursor)?;
        }

        // DMA. Rebuild the derived fields through `restore_control` rather than
        // assigning the raw control word: `timing`, `word_count` and the address
        // modes all come from decoding it, and sound DMA is `timing == 3`, so a
        // direct assignment left sound DMA dead after every restore.
        for i in 0..4 {
            gba.dma.channels[i].source = read_u32(&mut cursor)?;
            gba.dma.channels[i].dest = read_u32(&mut cursor)?;
            gba.dma.channels[i].count = read_u16(&mut cursor)?;
            let control = read_u16(&mut cursor)?;
            let enabled = read_bool(&mut cursor)?;
            gba.dma.restore_control(i, control, enabled);
        }

        // I/O
        gba.bus.io.ie = read_u16(&mut cursor)?;
        gba.bus.io.if_ = read_u16(&mut cursor)?;
        gba.bus.io.ime = read_u16(&mut cursor)?;
        gba.bus.io.halt = read_bool(&mut cursor)?;

        // Through the setter: the wait table is derived from it.
        let waitcnt = read_u16(&mut cursor)?;
        gba.bus.set_waitcnt(waitcnt);
        read_exact_vec(&mut cursor, gba.bus.io_regs_data_mut())?;
        read_exact_vec(&mut cursor, gba.bus.ewram_data_mut())?;
        read_exact_vec(&mut cursor, gba.bus.iwram_data_mut())?;
        read_exact_vec(&mut cursor, gba.bus.palette_data_mut())?;
        read_exact_vec(&mut cursor, gba.bus.vram_data_mut())?;
        read_exact_vec(&mut cursor, gba.bus.oam_data_mut())?;

        // Bound the length against what is actually left: these come from a
        // file on disk, and a corrupt or truncated state with a length field
        // of 0xFFFFFFFF asked for a 4 GB allocation and aborted the process
        // instead of returning Err.
        let apu_len = read_u32(&mut cursor)? as usize;
        if apu_len > self.data.len().saturating_sub(cursor.position() as usize) {
            return Err(SaveStateError::InsufficientData);
        }
        let mut apu = vec![0u8; apu_len];
        read_exact_vec(&mut cursor, &mut apu)?;
        if !gba.apu.restore(&apu) {
            return Err(SaveStateError::InsufficientData);
        }

        let save_len = read_u32(&mut cursor)? as usize;
        if save_len > self.data.len().saturating_sub(cursor.position() as usize) {
            return Err(SaveStateError::InsufficientData);
        }
        if save_len > 0 {
            let mut save = vec![0u8; save_len];
            read_exact_vec(&mut cursor, &mut save)?;
            gba.load_save(&save);
        }

        // Global
        gba.cycles = read_u64(&mut cursor)?;
        // 2^62 cycles is 8,700 years of play. Near u64::MAX the frame target
        // wrapped below `cycles` and every frame returned without stepping.
        if gba.cycles > 1 << 62 {
            return Err(SaveStateError::Corrupt("cycle count"));
        }

        if version >= 7 {
            let mut chip = [0u8; crate::cart::CHIP_STATE_LEN];
            cursor.read_exact(&mut chip)?;
            gba.bus
                .cart
                .restore_chip_state(&chip)
                .map_err(SaveStateError::Corrupt)?;
        }

        for ch in &mut gba.dma.channels {
            ch.latch = if version >= 8 {
                read_u32(&mut cursor)?
            } else {
                0
            };
        }

        gba.bus.sio_cycles = if version >= 9 {
            read_u32(&mut cursor)?
        } else {
            0
        };

        // The rollback undoes everything applied above. Without it a ROM
        // hack's state - same header as the original, so the frontend offers
        // the same slots - loaded and replaced this cart's battery save with
        // the other game's.
        if version >= 10 && read_u32(&mut cursor)? != gba.bus.cart.rom_crc() {
            return Err(SaveStateError::DifferentRom);
        }

        sync_dispstat_cache(gba);
        if version >= 11 {
            restore_live(&mut cursor, gba)
        } else {
            derive_live(gba);
            Ok(())
        }
    }

    /// Save state to a file.
    pub fn save_to_file(&self, path: &str) -> io::Result<()> {
        std::fs::write(path, &self.data)
    }

    /// Load state from a file.
    pub fn load_from_file(path: &str) -> io::Result<Self> {
        let data = std::fs::read(path)?;
        Ok(SaveState { data })
    }
}

#[derive(Debug)]
pub enum SaveStateError {
    Io(io::Error),
    InvalidMagic,
    UnsupportedVersion(u32),
    InsufficientData,
    /// The bytes parsed, but describe a machine that cannot exist: a scanline
    /// past 227, a timer prescaler that is not 1/64/256/1024, a FIFO cursor
    /// outside its 32-byte ring. A save state is bytes from outside the app -
    /// a file a player picked, or one that was truncated or edited - and
    /// restored as-is these did not fail politely: an index past an array is
    /// a panic, which aborts the app through FFI, and a zero prescaler is an
    /// infinite loop in the timer. Rejected here, the restore rolls back.
    Corrupt(&'static str),
    /// The state was taken on another ROM (its CRC-32 differs).
    DifferentRom,
}

impl From<io::Error> for SaveStateError {
    fn from(e: io::Error) -> Self {
        SaveStateError::Io(e)
    }
}

// Serialization helpers
fn write_u8(buf: &mut Vec<u8>, v: u8) {
    buf.push(v);
}
fn write_bool(buf: &mut Vec<u8>, v: bool) {
    buf.push(v as u8);
}
fn write_u16(buf: &mut Vec<u8>, v: u16) {
    buf.extend_from_slice(&v.to_le_bytes());
}
fn write_i16(buf: &mut Vec<u8>, v: i16) {
    buf.extend_from_slice(&v.to_le_bytes());
}
fn write_u32(buf: &mut Vec<u8>, v: u32) {
    buf.extend_from_slice(&v.to_le_bytes());
}
fn write_i32(buf: &mut Vec<u8>, v: i32) {
    buf.extend_from_slice(&v.to_le_bytes());
}
fn write_u64(buf: &mut Vec<u8>, v: u64) {
    buf.extend_from_slice(&v.to_le_bytes());
}
fn write_u32_array(buf: &mut Vec<u8>, arr: &[u32]) {
    for &v in arr {
        write_u32(buf, v);
    }
}

fn read_u8(r: &mut Cursor<&Vec<u8>>) -> io::Result<u8> {
    let mut b = [0u8; 1];
    r.read_exact(&mut b)?;
    Ok(b[0])
}
fn read_bool(r: &mut Cursor<&Vec<u8>>) -> io::Result<bool> {
    Ok(read_u8(r)? != 0)
}
fn read_u16(r: &mut Cursor<&Vec<u8>>) -> io::Result<u16> {
    let mut b = [0u8; 2];
    r.read_exact(&mut b)?;
    Ok(u16::from_le_bytes(b))
}
fn read_i16(r: &mut Cursor<&Vec<u8>>) -> io::Result<i16> {
    let mut b = [0u8; 2];
    r.read_exact(&mut b)?;
    Ok(i16::from_le_bytes(b))
}
fn read_u32(r: &mut Cursor<&Vec<u8>>) -> io::Result<u32> {
    let mut b = [0u8; 4];
    r.read_exact(&mut b)?;
    Ok(u32::from_le_bytes(b))
}
fn read_i32(r: &mut Cursor<&Vec<u8>>) -> io::Result<i32> {
    let mut b = [0u8; 4];
    r.read_exact(&mut b)?;
    Ok(i32::from_le_bytes(b))
}
fn read_u64(r: &mut Cursor<&Vec<u8>>) -> io::Result<u64> {
    let mut b = [0u8; 8];
    r.read_exact(&mut b)?;
    Ok(u64::from_le_bytes(b))
}
fn read_u32_array(r: &mut Cursor<&Vec<u8>>, arr: &mut [u32]) -> io::Result<()> {
    for v in arr.iter_mut() {
        *v = read_u32(r)?;
    }
    Ok(())
}
/// The v11 live state. Left out before it, so a restored machine kept the
/// target's own: the line boundary moved, a rotated background drew the
/// wrong rows until VBlank, and a state saved in VBlankIntrWait woke a frame
/// late.
fn restore_live(r: &mut Cursor<&Vec<u8>>, gba: &mut Gba) -> Result<(), SaveStateError> {
    let ppu = &mut gba.ppu;
    ppu.cycle_counter = read_u32(r)?;
    if ppu.cycle_counter > MAX_LIVE_CYCLES {
        return Err(SaveStateError::Corrupt("PPU dot counter"));
    }
    ppu.line_rendered = read_bool(r)?;
    for v in [
        &mut ppu.bg2x_internal,
        &mut ppu.bg2y_internal,
        &mut ppu.bg3x_internal,
        &mut ppu.bg3y_internal,
    ] {
        *v = read_i32(r)?;
        if !(-MAX_AFFINE..MAX_AFFINE).contains(v) {
            return Err(SaveStateError::Corrupt("affine reference accumulator"));
        }
    }
    gba.bus.io.intr_wait_active = read_bool(r)?;
    gba.bus.io.halt_swi = read_bool(r)?;
    gba.bus.bios_latch = read_u32(r)?;
    gba.cpu.entry_cycles = read_u32(r)?;
    gba.dma_stall = read_u32(r)?;
    if gba.cpu.entry_cycles > MAX_LIVE_CYCLES || gba.dma_stall > MAX_LIVE_CYCLES {
        return Err(SaveStateError::Corrupt("cycles owed to the next step"));
    }
    gba.irq_since = match read_u64(r)? {
        u64::MAX => None,
        // A line raised in the future would hold the IRQ off until then.
        at if at <= gba.cycles => Some(at),
        _ => return Err(SaveStateError::Corrupt("IRQ line raised after now")),
    };
    Ok(())
}

/// The PPU's copies of DISPSTAT: the VCount match it edge-detects the IRQ
/// on, and the three IRQ enables. Every PPU tick writes them back into
/// DISPSTAT, so at a step boundary they are the restored register. Left as
/// the target's, a state taken on the LYC line raised the VCount IRQ again.
fn sync_dispstat_cache(gba: &mut Gba) {
    let dispstat = gba.bus.io_read16(0x0400_0004);
    let ppu = &mut gba.ppu;
    ppu.vcount_match = dispstat & 0x0004 != 0;
    ppu.dispstat_vblank_ie = dispstat & 0x0008 != 0;
    ppu.dispstat_hblank_ie = dispstat & 0x0010 != 0;
    ppu.dispstat_vcount_ie = dispstat & 0x0020 != 0;
}

/// A pre-v11 state carries no live state. Derive what the registers give and
/// reset the rest as at power-on, so the result does not depend on what the
/// target machine was running.
fn derive_live(gba: &mut Gba) {
    let ppu = &mut gba.ppu;
    // Start of the line, or of its HBlank if the state was in one.
    ppu.cycle_counter = if ppu.hblank { 960 } else { 0 };
    ppu.line_rendered = ppu.hblank;
    ppu.reload_affine_reference();
    gba.bus.io.intr_wait_active = false;
    gba.bus.io.halt_swi = false;
    gba.bus.bios_latch = crate::memory::BIOS_AFTER_BOOT;
    gba.cpu.entry_cycles = 0;
    gba.dma_stall = 0;
    gba.irq_since = None;
}

fn read_exact_vec(r: &mut Cursor<&Vec<u8>>, buf: &mut [u8]) -> io::Result<()> {
    r.read_exact(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A state taken from a real machine with one field forced to a value
    /// the hardware cannot hold. `pub(crate)` fields are why this lives in
    /// the crate rather than in `core/tests`.
    fn state_with(edit: impl FnOnce(&mut Gba)) -> SaveState {
        let mut gba = Gba::new();
        edit(&mut gba);
        SaveState::create(&gba)
    }

    /// A zero prescaler is an infinite loop in `Timer::tick`: the restore has
    /// to refuse it, or the next frame never returns.
    #[test]
    fn a_zero_timer_prescaler_is_rejected() {
        let state = state_with(|gba| gba.timer.prescaler[0] = 0);
        let mut target = Gba::new();
        assert!(matches!(
            state.restore(&mut target),
            Err(SaveStateError::Corrupt(_))
        ));
    }

    #[test]
    fn every_real_prescaler_is_accepted() {
        for p in [1u32, 64, 256, 1024] {
            let state = state_with(|gba| gba.timer.prescaler[2] = p);
            assert!(
                state.restore(&mut Gba::new()).is_ok(),
                "prescaler {p} is real"
            );
        }
    }

    /// Line 228 does not exist. Restored as-is, it sent the PPU past its
    /// frame buffer on the next forced-blank line.
    #[test]
    fn a_scanline_past_227_is_rejected() {
        let state = state_with(|gba| gba.ppu.scanline = 300);
        assert!(matches!(
            state.restore(&mut Gba::new()),
            Err(SaveStateError::Corrupt(_))
        ));
    }

    /// The rollback is the point: a rejected state must leave the machine as
    /// it was, not half-restored with the bad value already applied.
    #[test]
    fn a_rejected_state_leaves_the_machine_untouched() {
        let bad = state_with(|gba| gba.timer.prescaler[1] = 0);
        let mut target = Gba::new();
        target.run_frame();
        let before = SaveState::create(&target).data;
        assert!(bad.restore(&mut target).is_err());
        assert_eq!(SaveState::create(&target).data, before);
        // And it still runs - the bad prescaler never reached the timer.
        target.run_frame();
    }

    /// A blank ROM carrying a save-library tag, so `detect_save_type` picks
    /// the chip.
    fn machine_with(tag: &[u8]) -> Gba {
        let mut rom = vec![0u8; 0x200];
        rom[0x100..0x100 + tag.len()].copy_from_slice(tag);
        let mut gba = Gba::new();
        gba.load_rom(&rom).expect("ROM should load");
        gba
    }

    /// The Flash command state machine and bank select are chip state, not
    /// save memory. A state taken between `AA 55 A0` and the data byte, with
    /// bank 1 selected, has to finish that program into bank 1 after a load.
    #[test]
    fn flash_command_state_and_bank_survive_a_save_state() {
        let mut gba = machine_with(b"FLASH1M_V103");
        let cmd = |gba: &mut Gba, c: u8| {
            gba.bus.write8(0x0E00_5555, 0xAA);
            gba.bus.write8(0x0E00_2AAA, 0x55);
            gba.bus.write8(0x0E00_5555, c);
        };
        cmd(&mut gba, 0xB0);
        gba.bus.write8(0x0E00_0000, 1); // bank 1
        cmd(&mut gba, 0xA0); // byte program, data byte still to come
        let state = SaveState::create(&gba);

        let mut target = machine_with(b"FLASH1M_V103");
        state.restore(&mut target).expect("state should load");
        target.bus.write8(0x0E00_0010, 0x42);
        assert_eq!(
            target.save_data().expect("flash cart")[0x1_0010],
            0x42,
            "the pending program was lost, or landed in the wrong bank"
        );
    }

    /// EEPROM is a serial device: a state taken after a set-address command
    /// and before the read-back has to stream the addressed block afterwards.
    #[test]
    fn an_eeprom_read_in_progress_survives_a_save_state() {
        let mut gba = machine_with(b"EEPROM_V124");
        let mut save = vec![0xFFu8; 8 * 1024];
        save[5 * 8..6 * 8].fill(0xA5);
        gba.load_save(&save);
        gba.bus.cart.eeprom_begin_dma(17);
        let mut bits = vec![1u16, 1];
        bits.extend((0..14).rev().map(|i| (5u16 >> i) & 1));
        bits.push(0);
        for b in bits {
            gba.bus.cart.eeprom_write(b);
        }
        let state = SaveState::create(&gba);

        let mut target = machine_with(b"EEPROM_V124");
        state.restore(&mut target).expect("state should load");
        let out: Vec<u16> = (0..68).map(|_| target.bus.cart.eeprom_read()).collect();
        let want: Vec<u16> = [0u16; 4]
            .into_iter()
            .chain((0..64).map(|i| (0xA5u16 >> (7 - i % 8)) & 1))
            .collect();
        assert_eq!(out, want, "the read-back did not resume at block 5");
    }

    /// Chip bytes come from a file. One naming an EEPROM block past the chip
    /// must be refused, not indexed.
    #[test]
    fn a_hostile_chip_state_is_rejected() {
        let mut state = SaveState::create(&machine_with(b"EEPROM_V124"));
        let len = state.data.len();
        let chip_at =
            len - LIVE_LEN - ROM_ID_LEN - SIO_LEN - DMA_LATCHES_LEN - crate::cart::CHIP_STATE_LEN;
        let chip = &mut state.data[chip_at..];
        chip[7..9].copy_from_slice(&0xFFFFu16.to_le_bytes()); // EEPROM block
        let mut target = machine_with(b"EEPROM_V124");
        assert!(matches!(
            state.restore(&mut target),
            Err(SaveStateError::Corrupt(_))
        ));
        target.run_frame();
    }

    /// A DMA reading the BIOS returns its channel's last value, so the four
    /// latches are machine state.
    #[test]
    fn the_dma_latches_survive_a_save_state() {
        let state = state_with(|gba| {
            for (i, ch) in gba.dma.channels.iter_mut().enumerate() {
                ch.latch = 0x1111_1111 * (i as u32 + 1);
            }
        });
        let mut target = Gba::new();
        state.restore(&mut target).expect("state should load");
        let latches: Vec<u32> = target.dma.channels.iter().map(|c| c.latch).collect();
        assert_eq!(
            latches,
            [0x1111_1111, 0x2222_2222, 0x3333_3333, 0x4444_4444]
        );
    }

    /// v7 had no DMA latches. It still loads, and does not keep the latches
    /// of whatever the machine was running before.
    #[test]
    fn a_version_7_state_still_loads_with_clear_latches() {
        let mut state = SaveState::create(&Gba::new());
        state
            .data
            .truncate(state.data.len() - LIVE_LEN - ROM_ID_LEN - SIO_LEN - DMA_LATCHES_LEN);
        state.data[4..8].copy_from_slice(&7u32.to_le_bytes());
        let mut target = Gba::new();
        target.dma.channels[1].latch = 0xDEAD_BEEF;
        state.restore(&mut target).expect("v7 state should load");
        assert_eq!(target.dma.channels[1].latch, 0);
    }

    /// v6 had no chip block. It still loads.
    #[test]
    fn a_version_6_state_still_loads() {
        let mut state = SaveState::create(&machine_with(b"FLASH1M_V103"));
        state.data.truncate(
            state.data.len()
                - LIVE_LEN
                - ROM_ID_LEN
                - SIO_LEN
                - DMA_LATCHES_LEN
                - crate::cart::CHIP_STATE_LEN,
        );
        state.data[4..8].copy_from_slice(&6u32.to_le_bytes());
        assert!(state.restore(&mut machine_with(b"FLASH1M_V103")).is_ok());
    }

    /// A serial transfer in flight is machine state: without it, a state
    /// saved mid-transfer would leave SIOCNT's start bit set forever.
    #[test]
    fn a_serial_transfer_survives_a_save_state() {
        let state = state_with(|gba| gba.bus.sio_cycles = 123);
        let mut target = Gba::new();
        state.restore(&mut target).expect("state should load");
        assert_eq!(target.bus.sio_cycles, 123);
    }

    /// v8 had no serial counter. It still loads, with no transfer running.
    #[test]
    fn a_version_8_state_still_loads_with_no_serial_transfer() {
        let mut state = SaveState::create(&Gba::new());
        state
            .data
            .truncate(state.data.len() - LIVE_LEN - ROM_ID_LEN - SIO_LEN);
        state.data[4..8].copy_from_slice(&8u32.to_le_bytes());
        let mut target = Gba::new();
        target.bus.sio_cycles = 99;
        state.restore(&mut target).expect("v8 state should load");
        assert_eq!(target.bus.sio_cycles, 0);
    }

    /// The v11 block has exactly the length the offset tests assume.
    #[test]
    fn the_live_block_is_appended_last() {
        let gba = Gba::new();
        let mut v10 = SaveState::create(&gba);
        v10.data.truncate(v10.data.len() - LIVE_LEN);
        v10.data[4..8].copy_from_slice(&10u32.to_le_bytes());
        let rom_crc = gba.bus.cart.rom_crc().to_le_bytes();
        assert_eq!(v10.data[v10.data.len() - ROM_ID_LEN..], rom_crc);
    }

    /// v10 had no live state. It still loads, and the result is derived from
    /// the state, not kept from the machine it was loaded into.
    #[test]
    fn a_version_10_state_still_loads_with_derived_live_state() {
        let mut state = SaveState::create(&state_with_parts());
        state.data.truncate(state.data.len() - LIVE_LEN);
        state.data[4..8].copy_from_slice(&10u32.to_le_bytes());

        let mut target = Gba::new();
        target.ppu.cycle_counter = 700;
        target.ppu.line_rendered = true;
        target.ppu.bg2y_internal = 0x1234;
        target.bus.io.intr_wait_active = true;
        target.bus.io.halt_swi = true;
        target.bus.bios_latch = 0xDEAD_BEEF;
        target.cpu.entry_cycles = 45;
        target.dma_stall = 99;
        target.irq_since = Some(0);
        state.restore(&mut target).expect("v10 state should load");

        assert_eq!(
            (target.ppu.cycle_counter, target.ppu.line_rendered),
            (960, true),
            "in HBlank: the dot is HBlank's start"
        );
        assert_eq!(target.ppu.bg2y_internal, 0x500, "reloaded from BG2Y");
        assert!(target.ppu.vcount_match, "from DISPSTAT bit 2");
        assert!(!target.bus.io.intr_wait_active && !target.bus.io.halt_swi);
        assert_eq!(target.bus.bios_latch, crate::memory::BIOS_AFTER_BOOT);
        assert_eq!(
            (target.cpu.entry_cycles, target.dma_stall, target.irq_since),
            (0, 0, None)
        );
    }

    /// A machine in HBlank with BG2Y = 0x500 and DISPSTAT's VCount bit set.
    fn state_with_parts() -> Gba {
        let mut gba = Gba::new();
        gba.ppu.hblank = true;
        gba.ppu.bg2y = 0x500;
        gba.bus.write16(0x0400_0004, 0x0004);
        gba
    }

    /// Live values the machine cannot hold, each of which would overflow or
    /// stall it, are refused.
    #[test]
    fn hostile_live_state_is_rejected() {
        type Edit = fn(&mut Gba);
        let cases: [(&str, Edit); 5] = [
            ("dot counter", |g| g.ppu.cycle_counter = u32::MAX),
            ("affine", |g| g.ppu.bg3x_internal = i32::MAX),
            ("entry cycles", |g| g.cpu.entry_cycles = u32::MAX),
            ("DMA stall", |g| g.dma_stall = 1 << 30),
            ("IRQ in the future", |g| g.irq_since = Some(1_000)),
        ];
        for (what, edit) in cases {
            let state = state_with(edit);
            let mut target = Gba::new();
            let before = SaveState::create(&target).data;
            assert!(
                matches!(state.restore(&mut target), Err(SaveStateError::Corrupt(_))),
                "{what} was accepted"
            );
            assert_eq!(SaveState::create(&target).data, before, "{what}");
        }
    }

    #[test]
    fn real_live_state_is_accepted() {
        let state = state_with(|g| {
            g.cycles = 5_000;
            g.ppu.cycle_counter = 1231;
            g.ppu.bg2x_internal = -(1 << 27) - 160 * 0x8000;
            g.cpu.entry_cycles = 48;
            g.dma_stall = 1 << 21;
            g.irq_since = Some(4_998);
        });
        let mut target = Gba::new();
        state.restore(&mut target).expect("real values load");
        assert_eq!(target.irq_since, Some(4_998));
        assert_eq!(target.ppu.bg2x_internal, -(1 << 27) - 160 * 0x8000);
    }
}
