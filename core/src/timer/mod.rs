pub struct Timer {
    counters: [u32; 4],
    reloads: [u32; 4],
    controls: [u16; 4],
    enabled: [bool; 4],
    cascaded: [bool; 4],
}

impl Timer {
    pub fn new() -> Self {
        Self {
            counters: [0; 4],
            reloads: [0; 4],
            controls: [0; 4],
            enabled: [false; 4],
            cascaded: [false; 4],
        }
    }

    pub fn tick(&mut self, cycles: u32, bus: &mut super::memory::MemoryBus) {
        for i in 0..4 {
            if !self.enabled[i] {
                continue;
            }

            if self.cascaded[i] && i > 0 {
                // Timer cascades: timer N overflows trigger timer N+1
                continue;
            }

            self.counters[i] += cycles;

            // Timer overflow at 0x10000 (16-bit counter)
            if self.counters[i] >= 0x10000 {
                self.counters[i] -= 0x10000;

                // Handle cascade to next timer
                if i < 3 && self.cascaded[i + 1] {
                    self.counters[i + 1] += 1;
                }

                // TODO: Trigger timer IRQ if enabled
                let _ = bus;
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
            self.enabled[timer] = value & 0x0080 != 0;
            self.cascaded[timer] = value & 0x0004 != 0;

            if self.enabled[timer] {
                self.counters[timer] = self.reloads[timer];
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
}
