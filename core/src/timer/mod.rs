const PRESCALER_TABLE: [u32; 4] = [1, 64, 256, 1024];

pub struct Timer {
    pub(crate) counters: [u32; 4],
    pub(crate) reloads: [u32; 4],
    pub(crate) controls: [u16; 4],
    pub(crate) enabled: [bool; 4],
    pub(crate) cascaded: [bool; 4],
    pub(crate) prescaler: [u32; 4],
    pub(crate) irq_enabled: [bool; 4],
    pub(crate) tick_counters: [u32; 4],
    /// How many times each timer overflowed since the last drain. DMA sound
    /// pops one FIFO byte per overflow, so this has to be a count: a bool
    /// silently collapses the several overflows a single tick can produce at
    /// a short reload, and the FIFO then drains slower than the game fills it.
    pub overflow_flags: [u32; 4],
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
            overflow_flags: [0; 4],
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
                self.increment(i, bus);
            }
        }
    }

    /// Advance one timer by a single count, handling its overflow.
    ///
    /// The cascade used to be a bare `counters[i + 1] += 1` inside `tick`,
    /// with no overflow check of its own - and `tick` skips cascaded timers,
    /// so nothing else ever looked at that counter either. A cascaded timer
    /// therefore counted past 0x10000 forever: never reloading, never setting
    /// an overflow flag for DMA sound, and never raising its IRQ. Going
    /// through the same function recursively gives the cascade target the
    /// identical overflow handling, including cascading on to the next timer.
    fn increment(&mut self, i: usize, bus: &mut super::memory::MemoryBus) {
        self.counters[i] += 1;

        // Timer overflow at 0x10000 (16-bit counter)
        if self.counters[i] < 0x10000 {
            return;
        }
        self.counters[i] = self.reloads[i];
        self.overflow_flags[i] += 1;

        // Handle cascade to next timer. A cascade target only counts up while
        // it is itself enabled.
        if i < 3 && self.cascaded[i + 1] && self.enabled[i + 1] {
            self.increment(i + 1, bus);
        }

        // GBATEK, GBA Interrupt Control: IF bits 3,4,5,6 are Timer 0,1,2,3
        // overflow. IE and IME gate whether the CPU takes the exception,
        // never whether IF is set.
        if self.irq_enabled[i] {
            bus.io.request_interrupt(1 << (3 + i));
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

    /// Drain the per-timer overflow counts since the last call.
    pub fn drain_overflows(&mut self) -> [u32; 4] {
        std::mem::take(&mut self.overflow_flags)
    }

    /// The live counter of each timer, for writing back into `TMxCNT_L`.
    pub fn counters(&self) -> [u16; 4] {
        [
            self.counters[0] as u16,
            self.counters[1] as u16,
            self.counters[2] as u16,
            self.counters[3] as u16,
        ]
    }
}
