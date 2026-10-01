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
const SAVE_VERSION: u32 = 7;

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

        write_u16(&mut buf, gba.bus.waitcnt);
        // The whole I/O register file. Absent from v2, so DISPCNT, the
        // background scroll registers and everything else came back as
        // whatever the fresh instance happened to hold.
        buf.extend_from_slice(gba.bus.io_regs_data());
        buf.extend_from_slice(&gba.bus.ewram_data());
        buf.extend_from_slice(&gba.bus.iwram_data());
        buf.extend_from_slice(&gba.bus.palette_data());
        buf.extend_from_slice(&gba.bus.vram_data());
        buf.extend_from_slice(&gba.bus.oam_data());

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

        gba.bus.waitcnt = read_u16(&mut cursor)?;
        read_exact_vec(&mut cursor, gba.bus.io_regs_data_mut())?;
        read_exact_vec(&mut cursor, &mut gba.bus.ewram_data_mut())?;
        read_exact_vec(&mut cursor, &mut gba.bus.iwram_data_mut())?;
        read_exact_vec(&mut cursor, &mut gba.bus.palette_data_mut())?;
        read_exact_vec(&mut cursor, &mut gba.bus.vram_data_mut())?;
        read_exact_vec(&mut cursor, &mut gba.bus.oam_data_mut())?;

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

        if version >= 7 {
            let mut chip = [0u8; crate::cart::CHIP_STATE_LEN];
            cursor.read_exact(&mut chip)?;
            gba.bus
                .cart
                .restore_chip_state(&chip)
                .map_err(SaveStateError::Corrupt)?;
        }

        Ok(())
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
        let chip = &mut state.data[len - crate::cart::CHIP_STATE_LEN..];
        chip[7..9].copy_from_slice(&0xFFFFu16.to_le_bytes()); // EEPROM block
        let mut target = machine_with(b"EEPROM_V124");
        assert!(matches!(
            state.restore(&mut target),
            Err(SaveStateError::Corrupt(_))
        ));
        target.run_frame();
    }

    /// v6 had no chip block. It still loads.
    #[test]
    fn a_version_6_state_still_loads() {
        let mut state = SaveState::create(&machine_with(b"FLASH1M_V103"));
        state
            .data
            .truncate(state.data.len() - crate::cart::CHIP_STATE_LEN);
        state.data[4..8].copy_from_slice(&6u32.to_le_bytes());
        assert!(state.restore(&mut machine_with(b"FLASH1M_V103")).is_ok());
    }
}
