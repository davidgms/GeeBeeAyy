const PRESCALER_TABLE: [u32; 4] = [1, 64, 256, 1024];

pub struct Timer {
    counters: [u32; 4],
    reloads: [u32; 4],
    controls: [u16; 4],
    enabled: [bool; 4],
    cascaded: [bool; 4],
    prescaler: [u32; 4],
    irq_enabled: [bool; 4],
    tick_counters: [u32; 4],
    /// Tracks which timers overflowed this tick (for APU sound DMA)
    pub overflow_flags: [bool; 4],
}

impl Timer {
    pub fn new() -> Self {
        Self {
            counters: [0; 4],
            reloads: [0; 4],
            controls: [0; 4],
            enabled: [false; 4],
            cascaded: [false; 4],
            prescaler: [1; 4],
            irq_enabled: [false; 4],
            tick_counters: [0; 4],
            overflow_flags: [false; 4],
        }
    }

    pub fn tick(&mut self, cycles: u32, bus: &mut super::memory::MemoryBus) {
        for i in 0..4 {
            if !self.enabled[i] {
                continue;
            }

            if self.cascaded[i] && i > 0 {
                continue;
            }

            self.tick_counters[i] += cycles;

            let prescaler = self.prescaler[i];
            while self.tick_counters[i] >= prescaler {
                self.tick_counters[i] -= prescaler;
                self.counters[i] += 1;

                // Timer overflow at 0x10000 (16-bit counter)
                if self.counters[i] >= 0x10000 {
                    self.counters[i] = self.reloads[i];
                    self.overflow_flags[i] = true;

                    // Handle cascade to next timer
                    if i < 3 && self.cascaded[i + 1] {
                        self.counters[i + 1] += 1;
                    }

                    // GBATEK, GBA Interrupt Control: IF bits 3,4,5,6 are
                    // Timer 0,1,2,3 overflow. IE and IME gate whether the CPU
                    // takes the exception, never whether IF is set.
                    if self.irq_enabled[i] {
                        bus.io.request_interrupt(1 << (3 + i));
                    }
                }
            }
        }
    }

    pub fn set_reload(&mut self, timer: usize, value: u16) {
        if timer < 4 {
            self.reloads[timer] = value as u32;
        }
    }

    pub fn set_control(&mut self, timer: usize, value: u16) {
        if timer < 4 {
            self.controls[timer] = value;
            let was_enabled = self.enabled[timer];
            self.enabled[timer] = value & 0x0080 != 0;
            self.cascaded[timer] = value & 0x0004 != 0;
            self.irq_enabled[timer] = value & 0x0040 != 0;

            // Set prescaler from bits 0-1
            let prescaler_bits = (value & 0x0003) as usize;
            self.prescaler[timer] = PRESCALER_TABLE[prescaler_bits];

            // When timer is enabled, reload counter
            if self.enabled[timer] && !was_enabled {
                self.counters[timer] = self.reloads[timer];
                self.tick_counters[timer] = 0;
            }
        }
    }

    pub fn counter(&self, timer: usize) -> u16 {
        if timer < 4 {
            self.counters[timer] as u16
        } else {
            0
        }
    }

    /// Drain overflow flags (returns which timers overflowed).
    pub fn drain_overflows(&mut self) -> [bool; 4] {
        let flags = self.overflow_flags;
        self.overflow_flags = [false; 4];
        flags
    }
}
