/// DMAxCNT_H offsets from 0x04000000, one per channel.
const DMA_CNT_H: [usize; 4] = [0x0BA, 0x0C6, 0x0D2, 0x0DE];

pub struct DmaChannel {
    pub source: u32,
    pub dest: u32,
    pub count: u16,
    pub control: u16,
    pub enabled: bool,
    pub word_count: u32,
    pub src_adj: u8,
    pub dst_adj: u8,
    pub repeat: bool,
    pub transfer_type: bool,
    pub timing: u8,
    pub irq_on_end: bool,
    pub enable: bool,
    pub src_fixed: bool,
    pub dst_fixed: bool,
    pub dst_reload: bool,
}

pub struct Dma {
    pub channels: [DmaChannel; 4],
    hblank_fired: bool,
}

impl Dma {
    pub fn new() -> Self {
        Self {
            channels: [
                DmaChannel {
                    source: 0,
                    dest: 0,
                    count: 0,
                    control: 0,
                    enabled: false,
                    word_count: 0,
                    src_adj: 0,
                    dst_adj: 0,
                    repeat: false,
                    transfer_type: false,
                    timing: 0,
                    irq_on_end: false,
                    enable: false,
                    src_fixed: false,
                    dst_fixed: false,
                    dst_reload: false,
                },
                DmaChannel {
                    source: 0,
                    dest: 0,
                    count: 0,
                    control: 0,
                    enabled: false,
                    word_count: 0,
                    src_adj: 0,
                    dst_adj: 0,
                    repeat: false,
                    transfer_type: false,
                    timing: 0,
                    irq_on_end: false,
                    enable: false,
                    src_fixed: false,
                    dst_fixed: false,
                    dst_reload: false,
                },
                DmaChannel {
                    source: 0,
                    dest: 0,
                    count: 0,
                    control: 0,
                    enabled: false,
                    word_count: 0,
                    src_adj: 0,
                    dst_adj: 0,
                    repeat: false,
                    transfer_type: false,
                    timing: 0,
                    irq_on_end: false,
                    enable: false,
                    src_fixed: false,
                    dst_fixed: false,
                    dst_reload: false,
                },
                DmaChannel {
                    source: 0,
                    dest: 0,
                    count: 0,
                    control: 0,
                    enabled: false,
                    word_count: 0,
                    src_adj: 0,
                    dst_adj: 0,
                    repeat: false,
                    transfer_type: false,
                    timing: 0,
                    irq_on_end: false,
                    enable: false,
                    src_fixed: false,
                    dst_fixed: false,
                    dst_reload: false,
                },
            ],
            hblank_fired: false,
        }
    }

    pub fn write_sad(&mut self, channel: usize, value: u32) {
        if channel < 4 {
            self.channels[channel].source = value;
        }
    }

    pub fn write_dad(&mut self, channel: usize, value: u32) {
        if channel < 4 {
            self.channels[channel].dest = value;
        }
    }

    pub fn write_count(&mut self, channel: usize, value: u16) {
        if channel < 4 {
            // GBATEK, GBA DMA Transfers: the unit count is 14 bits on DMA0-2
            // and 16 on DMA3, and 0 means the maximum (0x4000 / 0x10000).
            // The maximum does not fit the field, so `do_transfer` expands it
            // - the old code computed 0x10000 and then truncated it back to 0
            // with an `as u16`, which turned every maximum-length transfer
            // into a no-op.
            let mask = if channel == 3 { 0xFFFF } else { 0x3FFF };
            self.channels[channel].count = value & mask;
        }
    }

    /// Decode DMAxCNT_H into the channel's derived fields, without starting
    /// anything. Split out so a save-state restore can rebuild the derived
    /// state without re-running an immediate transfer.
    fn decode_control(&mut self, channel: usize, value: u16) {
        let ch = &mut self.channels[channel];
        ch.control = value;
        ch.enable = value & 0x8000 != 0;
        ch.repeat = value & 0x0200 != 0;
        // Must precede word_count: it used to be read one write stale.
        ch.transfer_type = value & 0x0400 != 0;
        ch.word_count = if ch.transfer_type { 4 } else { 2 };
        // GBATEK: bits 5-6 are the *destination* address control and bits 7-8
        // the *source*. They were read the other way round, which inverts
        // every transfer's addressing - a sound FIFO DMA (control 0xB640,
        // dest fixed) would have walked the destination through I/O space
        // while re-reading one source word.
        ch.dst_adj = ((value >> 5) & 3) as u8;
        ch.src_adj = ((value >> 7) & 3) as u8;
        ch.timing = ((value >> 12) & 3) as u8;
        ch.irq_on_end = value & 0x4000 != 0;
        ch.src_fixed = ch.src_adj == 2;
        ch.dst_fixed = ch.dst_adj == 2;
        ch.dst_reload = ch.dst_adj == 3;
    }

    pub fn write_control(
        &mut self,
        channel: usize,
        value: u16,
        bus: &mut super::memory::MemoryBus,
    ) {
        if channel >= 4 {
            return;
        }
        let was_enabled = self.channels[channel].enabled;
        self.decode_control(channel, value);

        if self.channels[channel].enable {
            self.channels[channel].enabled = true;
            // Only start on the enable edge, and only for immediate timing.
            if !was_enabled && self.channels[channel].timing == 0 {
                self.do_transfer(channel, bus);
            }
        } else {
            self.channels[channel].enabled = false;
        }
    }

    /// Rebuild a channel from a save state: decode the control word but never
    /// start a transfer.
    pub fn restore_control(&mut self, channel: usize, value: u16, enabled: bool) {
        if channel >= 4 {
            return;
        }
        self.decode_control(channel, value);
        self.channels[channel].enabled = enabled;
    }

    pub fn do_transfer(&mut self, channel: usize, bus: &mut super::memory::MemoryBus) {
        if channel >= 4 {
            return;
        }
        let ch = &mut self.channels[channel];
        // A count of 0 in the register means the maximum length.
        let count = if ch.count == 0 {
            if channel == 3 {
                0x1_0000
            } else {
                0x4000
            }
        } else {
            ch.count as u32
        };
        let word_size = ch.word_count;
        let src_adj = ch.src_adj;
        let dst_adj = ch.dst_adj;
        let dst_save = ch.dest;

        for _ in 0..count {
            if word_size == 4 {
                let val = bus.read32(ch.source);
                bus.write32(ch.dest, val);
            } else {
                // `read16_mut` so an EEPROM read advances its state machine.
                // DMA is the only way a game reaches EEPROM at all.
                let val = bus.read16_mut(ch.source);
                bus.write16(ch.dest, val);
            }

            // 0 = increment, 1 = decrement, 2 = fixed, 3 = increment/reload
            // (destination only; prohibited on the source).
            match src_adj {
                1 => ch.source = ch.source.wrapping_sub(word_size),
                2 => {}
                _ => ch.source = ch.source.wrapping_add(word_size),
            }
            match dst_adj {
                1 => ch.dest = ch.dest.wrapping_sub(word_size),
                2 => {}
                _ => ch.dest = ch.dest.wrapping_add(word_size),
            }
        }

        // On a repeat, only Increment/Reload restores the destination. The
        // source is never reloaded: it used to be, which made every repeating
        // HBlank DMA re-send the same words and flattened any per-scanline
        // table into a single value. A non-repeating channel keeps its
        // advanced addresses too - they are reloaded from SAD/DAD on the next
        // enable edge, which is what hardware latches on.
        if dst_adj == 3 {
            ch.dest = dst_save;
        }

        // GBATEK, GBA Interrupt Control: IF bits 8,9,10,11 are DMA 0,1,2,3.
        // Raised once the word count is exhausted, not mid-transfer.
        if ch.irq_on_end {
            bus.io.request_interrupt(1 << (8 + channel));
        }

        if !ch.repeat {
            ch.enabled = false;
            ch.control &= !0x8000;
            // Clear the enable bit in the register the game actually reads.
            // Clearing only the internal struct left DMAxCNT_H bit 15 set
            // forever, and a game that starts a transfer and polls for it to
            // finish - which Yggdra Union does during startup - never leaves
            // that loop.
            let offset = DMA_CNT_H[channel];
            let regs = bus.io_regs_data_mut();
            regs[offset + 1] &= 0x7F;
        }
    }

    /// Called when HBlank occurs, and only on a visible scanline: GBATEK notes
    /// HBlank DMA is not performed during VBlank. The PPU applies that gate.
    /// Triggers HBlank-timed DMA channels.
    pub fn on_hblank(&mut self, bus: &mut super::memory::MemoryBus) {
        self.hblank_fired = true;
        for i in 0..4 {
            let timing = self.channels[i].timing;
            let enabled = self.channels[i].enabled;
            if enabled && timing == 2 {
                self.do_transfer(i, bus);
            }
        }
    }

    /// Called when VBlank occurs. Triggers VBlank-timed DMA channels.
    pub fn on_vblank(&mut self, bus: &mut super::memory::MemoryBus) {
        for i in 0..4 {
            let timing = self.channels[i].timing;
            let enabled = self.channels[i].enabled;
            if enabled && timing == 1 {
                self.do_transfer(i, bus);
            }
        }
    }

    /// Called on scanline 0 (start of frame). Triggers special DMA channels.
    pub fn on_vcounter(&mut self, _bus: &mut super::memory::MemoryBus) {
        for i in 0..4 {
            let timing = self.channels[i].timing;
            let enabled = self.channels[i].enabled;
            // Timing 3 = special (Video capture DMA for DMA channels 1 and 2)
            if enabled && timing == 3 && (i == 1 || i == 2) {
                // TODO: Video capture DMA
            }
        }
    }

    /// Check if a DMA channel is set up for sound FIFO refill.
    /// Returns (channel_index, source_address) if it's a sound DMA.
    pub fn is_sound_dma(&self, channel: usize) -> bool {
        if channel >= 4 {
            return false;
        }
        let ch = &self.channels[channel];
        // Sound DMA: timing=3 (special), dest fixed to 0x040000A0/0x040000A4
        ch.timing == 3 && ch.enabled && (ch.dest == 0x0400_00A0 || ch.dest == 0x0400_00A4)
    }

    /// Perform a sound DMA transfer (4 words = 16 bytes to FIFO).
    pub fn do_sound_transfer(
        &mut self,
        channel: usize,
        bus: &mut super::memory::MemoryBus,
    ) -> Option<(u32, Vec<u8>)> {
        if channel >= 4 {
            return None;
        }
        let ch = &mut self.channels[channel];
        if !ch.enabled || ch.timing != 3 {
            return None;
        }

        let mut data = Vec::with_capacity(16);
        for _ in 0..4 {
            let val = bus.read32(ch.source);
            data.extend_from_slice(&val.to_le_bytes());
            ch.source = ch.source.wrapping_add(4);
        }

        Some((ch.dest, data))
    }

    pub fn tick(&mut self, _bus: &mut super::memory::MemoryBus) {
        self.hblank_fired = false;
    }
}
