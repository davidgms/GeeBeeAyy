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
    /// Cycles a just-started timer waits before its first count; see
    /// `delay_start`. Transient, so not part of a save state.
    pub(crate) start_delay: [u32; 4],
}

impl Default for Timer {
    fn default() -> Self {
        Self::new()
    }
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
            start_delay: [0; 4],
        }
    }

    /// Advance every running timer by `cycles`.
    ///
    /// Returns how many cycles into this slice the first timer IRQ was
    /// raised, so the caller can time the CPU's IRQ latency from the overflow
    /// itself rather than from the end of the instruction it fell in.
    pub fn tick(&mut self, cycles: u32, bus: &mut super::memory::MemoryBus) -> Option<u32> {
        let mut first_irq: Option<u32> = None;
        for i in 0..4 {
            if !self.enabled[i] {
                continue;
            }

            if self.cascaded[i] && i > 0 {
                continue;
            }

            let skip = self.start_delay[i].min(cycles);
            self.start_delay[i] -= skip;
            let mut elapsed = skip;
            let prescaler = self.prescaler[i];
            loop {
                // Saturating: a prescaler lowered on a running timer can leave
                // the accumulator past it, and then the next count is due now.
                let next = prescaler.saturating_sub(self.tick_counters[i]).max(1);
                if elapsed + next > cycles {
                    self.tick_counters[i] += cycles - elapsed;
                    break;
                }
                elapsed += next;
                self.tick_counters[i] = 0;
                if self.increment(i, bus) && first_irq.is_none() {
                    first_irq = Some(elapsed);
                }
            }
        }
        first_irq
    }

    /// Hold a timer that was just started off its first count for `cycles`.
    ///
    /// Hardware does not count the cycle right after the store that set the
    /// enable bit. The mGBA suite's timer-IRQ test pins this down: a timer
    /// started at 0xFFFF and one started at 0xFFFE both read 0 two
    /// instructions later, which only works if the 0xFFFF one overflowed once
    /// with the old reload before the `strh` that changed it took effect.
    pub fn delay_start(&mut self, timer: usize, cycles: u32) {
        if timer < 4 {
            self.start_delay[timer] = cycles;
        }
    }

    /// The counter as it will read one cycle from now.
    ///
    /// A load samples an I/O register in its second cycle, not its first, and
    /// this interpreter executes the whole instruction before ticking the
    /// timers. Publishing the counter one count ahead is what a load sees.
    fn counter_after_one_cycle(&self, i: usize) -> u16 {
        let counts = self.enabled[i]
            && !(self.cascaded[i] && i > 0)
            && self.start_delay[i] == 0
            && self.tick_counters[i] + 1 >= self.prescaler[i];
        match (counts, self.counters[i] + 1) {
            (false, _) => self.counters[i] as u16,
            (true, 0x10000) => self.reloads[i] as u16,
            (true, next) => next as u16,
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
    ///
    /// Returns whether an IRQ was raised, by this timer or a cascade target.
    fn increment(&mut self, i: usize, bus: &mut super::memory::MemoryBus) -> bool {
        self.counters[i] += 1;

        // Timer overflow at 0x10000 (16-bit counter)
        if self.counters[i] < 0x10000 {
            return false;
        }
        self.counters[i] = self.reloads[i];
        self.overflow_flags[i] += 1;

        // Handle cascade to next timer. A cascade target only counts up while
        // it is itself enabled.
        let mut irq = false;
        if i < 3 && self.cascaded[i + 1] && self.enabled[i + 1] {
            irq = self.increment(i + 1, bus);
        }

        // GBATEK, GBA Interrupt Control: IF bits 3,4,5,6 are Timer 0,1,2,3
        // overflow. IE and IME gate whether the CPU takes the exception,
        // never whether IF is set.
        if self.irq_enabled[i] {
            bus.io.request_interrupt(1 << (3 + i));
            irq = true;
        }
        irq
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

    /// The counter of each timer as a load reads it, for writing back into
    /// `TMxCNT_L`; see `counter_after_one_cycle`.
    pub fn counters(&self) -> [u16; 4] {
        std::array::from_fn(|i| self.counter_after_one_cycle(i))
    }
}
