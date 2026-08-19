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
    pub transfer_type: bool, // false=16bit, true=32bit
    pub timing: u8,
    pub irq_on_end: bool,
    pub enable: bool,
}

pub struct Dma {
    pub channels: [DmaChannel; 4],
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
                },
                DmaChannel {
                    source: 0, dest: 0, count: 0, control: 0,
                    enabled: false, word_count: 0,
                    src_adj: 0, dst_adj: 0, repeat: false,
                    transfer_type: false, timing: 0,
                    irq_on_end: false, enable: false,
                },
                DmaChannel {
                    source: 0, dest: 0, count: 0, control: 0,
                    enabled: false, word_count: 0,
                    src_adj: 0, dst_adj: 0, repeat: false,
                    transfer_type: false, timing: 0,
                    irq_on_end: false, enable: false,
                },
                DmaChannel {
                    source: 0, dest: 0, count: 0, control: 0,
                    enabled: false, word_count: 0,
                    src_adj: 0, dst_adj: 0, repeat: false,
                    transfer_type: false, timing: 0,
                    irq_on_end: false, enable: false,
                },
            ],
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

            // Only start transfer on enable edge
            if !was_enabled && ch.timing == 0 {
                self.do_transfer(channel, bus);
            }
        }
    }

    fn do_transfer(&mut self, channel: usize, bus: &mut super::memory::MemoryBus) {
        let ch = &mut self.channels[channel];
        let count = ch.count as u32;
        let word_size = ch.word_count;

        for _i in 0..count {
            if word_size == 4 {
                let val = bus.read32(ch.source);
                bus.write32(ch.dest, val);
            } else {
                let val = bus.read16(ch.source);
                bus.write16(ch.dest, val);
            }

            match ch.src_adj {
                0 => ch.source = ch.source.wrapping_add(word_size),
                1 => ch.source = ch.source.wrapping_sub(word_size),
                2 => {} // Fixed
                _ => {}
            }

            match ch.dst_adj {
                0 => ch.dest = ch.dest.wrapping_add(word_size),
                1 => ch.dest = ch.dest.wrapping_sub(word_size),
                2 => {} // Fixed
                _ => {}
            }
        }

        if ch.irq_on_end {
            // TODO: Trigger DMA IRQ
        }

        if !ch.repeat {
            ch.enabled = false;
            ch.control &= !0x8000;
        }
    }

    pub fn tick(&mut self, bus: &mut super::memory::MemoryBus) {
        // Handle immediate (timing=0) transfers that weren't caught on enable edge
        for i in 0..4 {
            if self.channels[i].enabled && self.channels[i].timing == 0 {
                self.do_transfer(i, bus);
            }
        }
    }
}
