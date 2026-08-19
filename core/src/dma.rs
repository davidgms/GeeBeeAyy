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
                    source: 0, dest: 0, count: 0, control: 0,
                    enabled: false, word_count: 0,
                    src_adj: 0, dst_adj: 0, repeat: false,
                    transfer_type: false, timing: 0,
                    irq_on_end: false, enable: false,
                    src_fixed: false, dst_fixed: false, dst_reload: false,
                },
                DmaChannel {
                    source: 0, dest: 0, count: 0, control: 0,
                    enabled: false, word_count: 0,
                    src_adj: 0, dst_adj: 0, repeat: false,
                    transfer_type: false, timing: 0,
                    irq_on_end: false, enable: false,
                    src_fixed: false, dst_fixed: false, dst_reload: false,
                },
                DmaChannel {
                    source: 0, dest: 0, count: 0, control: 0,
                    enabled: false, word_count: 0,
                    src_adj: 0, dst_adj: 0, repeat: false,
                    transfer_type: false, timing: 0,
                    irq_on_end: false, enable: false,
                    src_fixed: false, dst_fixed: false, dst_reload: false,
                },
                DmaChannel {
                    source: 0, dest: 0, count: 0, control: 0,
                    enabled: false, word_count: 0,
                    src_adj: 0, dst_adj: 0, repeat: false,
                    transfer_type: false, timing: 0,
                    irq_on_end: false, enable: false,
                    src_fixed: false, dst_fixed: false, dst_reload: false,
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
            self.channels[channel].count = if value == 0 { 0x10000 } else { value as u32 } as u16;
        }
    }

    pub fn write_control(&mut self, channel: usize, value: u16, bus: &mut super::memory::MemoryBus) {
        if channel >= 4 { return; }
        let ch = &mut self.channels[channel];
        ch.control = value;
        ch.enable = value & 0x8000 != 0;

        if ch.enable {
            let was_enabled = ch.enabled;
            ch.enabled = true;
            ch.word_count = if ch.transfer_type { 4 } else { 2 };

            ch.src_adj = ((value >> 5) & 3) as u8;
            ch.dst_adj = ((value >> 7) & 3) as u8;
            ch.repeat = value & 0x0200 != 0;
            ch.transfer_type = value & 0x0400 != 0;
            ch.timing = ((value >> 12) & 3) as u8;
            ch.irq_on_end = value & 0x4000 != 0;

            ch.src_fixed = ch.src_adj == 2;
            ch.dst_fixed = ch.dst_adj == 2;
            ch.dst_reload = ch.dst_adj == 3;

            // Only start transfer on enable edge for immediate (timing=0)
            if !was_enabled && ch.timing == 0 {
                self.do_transfer(channel, bus);
            }
        } else {
            ch.enabled = false;
        }
    }

    pub fn do_transfer(&mut self, channel: usize, bus: &mut super::memory::MemoryBus) {
        if channel >= 4 { return; }
        let ch = &mut self.channels[channel];
        let count = ch.count as u32;
        if count == 0 { return; }
        let word_size = ch.word_count;
        let src_fixed = ch.src_fixed;
        let dst_fixed = ch.dst_fixed;
        let dst_reload = ch.dst_reload;
        let src_save = ch.source;
        let dst_save = ch.dest;

        for _ in 0..count {
            if word_size == 4 {
                let val = bus.read32(ch.source);
                bus.write32(ch.dest, val);
            } else {
                let val = bus.read16(ch.source);
                bus.write16(ch.dest, val);
            }

            // Source address adjustment
            if !src_fixed {
                ch.source = ch.source.wrapping_add(word_size);
            }

            // Destination address adjustment
            if dst_fixed {
                // Fixed: do nothing
            } else if dst_reload {
                // Increment-reload: increment but reload on repeat
                ch.dest = ch.dest.wrapping_add(word_size);
            } else {
                ch.dest = ch.dest.wrapping_add(word_size);
            }
        }

        // Reload destination if not repeating
        if !ch.repeat || dst_reload {
            ch.dest = dst_save;
        }
        // Reload source if repeating
        if ch.repeat {
            ch.source = src_save;
        }

        if ch.irq_on_end {
            // TODO: Trigger DMA IRQ via IoHandler
        }

        if !ch.repeat {
            ch.enabled = false;
            ch.control &= !0x8000;
        }
    }

    /// Called when HBlank occurs. Triggers HBlank-timed DMA channels.
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

    pub fn tick(&mut self, _bus: &mut super::memory::MemoryBus) {
        self.hblank_fired = false;
    }
}
