pub struct Apu {
    cycle_counter: u32,
    sample_buffer: Vec<f32>,
    sample_rate: u32,
}

impl Apu {
    pub fn new() -> Self {
        Self {
            cycle_counter: 0,
            sample_buffer: Vec::new(),
            sample_rate: 44100,
        }
    }

    pub fn tick(&mut self, cycles: u32) {
        self.cycle_counter += cycles;

        // Generate samples at 44.1kHz
        // GBA CPU runs at 16.78 MHz
        // 16_777_216 / 44_100 = ~380 cycles per sample
        if self.cycle_counter >= 380 {
            self.cycle_counter -= 380;
            // TODO: Mix all audio channels and produce sample
            self.sample_buffer.push(0.0);
        }
    }

    pub fn samples(&self) -> &[f32] {
        &self.sample_buffer
    }

    pub fn clear_buffer(&mut self) {
        self.sample_buffer.clear();
    }
}
