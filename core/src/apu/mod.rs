// GBA APU (Audio Processing Unit)
// 4 PSG channels + 2 FIFO channels

const CYCLES_PER_SAMPLE: u32 = 964; // 16_777_216 / 17_500 ≈ 964 (for accurate GBA timing)

struct SoundChannel1 {
    enabled: bool,
    sweep_enabled: bool,
    sweep_shift: u8,
    sweep_dir: u8,
    sweep_timer: u8,
    sweep_tick: u8,
    duty: u8,
    volume_init: u8,
    volume_cur: u8,
    envelope_dir: u8,
    envelope_period: u8,
    envelope_timer: u8,
    length_counter: u16,
    length_enabled: bool,
    freq_divider: u16,
    freq_timer: u16,
    freq_counter: u16,
    sample_idx: u32,
}

struct SoundChannel2 {
    enabled: bool,
    duty: u8,
    volume_init: u8,
    volume_cur: u8,
    envelope_dir: u8,
    envelope_period: u8,
    envelope_timer: u8,
    length_counter: u16,
    length_enabled: bool,
    freq_divider: u16,
    freq_timer: u16,
    freq_counter: u16,
    sample_idx: u32,
}

struct SoundChannel3 {
    enabled: bool,
    bank_select: bool,
    volume_code: u8,
    length_counter: u16,
    length_enabled: bool,
    freq_divider: u16,
    freq_timer: u16,
    freq_counter: u16,
    wave_ram: [u8; 32],
    sample_idx: u32,
}

struct SoundChannel4 {
    enabled: bool,
    volume_init: u8,
    volume_cur: u8,
    envelope_dir: u8,
    envelope_period: u8,
    envelope_timer: u8,
    length_counter: u16,
    length_enabled: bool,
    shift_freq: u8,
    width_mode: u8,
    div_ratio: u8,
    lfsr: u16,
    freq_timer: u16,
    freq_counter: u16,
}

struct FifoChannel {
    buffer: [i8; 32],
    read_pos: usize,
    write_pos: usize,
    count: usize,
    timer: u8,
    enabled: bool,
    dma_refill: bool,
}

impl FifoChannel {
    fn new() -> Self {
        Self {
            buffer: [0; 32],
            read_pos: 0,
            write_pos: 0,
            count: 0,
            timer: 0,
            enabled: false,
            dma_refill: false,
        }
    }

    fn push(&mut self, sample: i8) {
        if self.count < 32 {
            self.buffer[self.write_pos] = sample;
            self.write_pos = (self.write_pos + 1) % 32;
            self.count += 1;
        }
    }

    fn pop(&mut self) -> i8 {
        if self.count > 0 {
            let sample = self.buffer[self.read_pos];
            self.read_pos = (self.read_pos + 1) % 32;
            self.count -= 1;
            sample
        } else {
            0
        }
    }

    fn is_empty(&self) -> bool {
        self.count == 0
    }

    fn is_half_empty(&self) -> bool {
        self.count <= 16
    }
}

pub struct Apu {
    cycle_counter: u32,
    sample_buffer: Vec<f32>,
    ch1: SoundChannel1,
    ch2: SoundChannel2,
    ch3: SoundChannel3,
    ch4: SoundChannel4,
    fifo_a: FifoChannel,
    fifo_b: FifoChannel,
    sound_on: bool,
    sound1_vol: u8,
    sound2_vol: u8,
    master_vol_left: u8,
    master_vol_right: u8,
    sound_out_mix: u8,
    envelope_tick_counter: u32,
}

impl Apu {
    pub fn new() -> Self {
        Self {
            cycle_counter: 0,
            sample_buffer: Vec::new(),
            ch1: SoundChannel1 {
                enabled: false, sweep_enabled: false, sweep_shift: 0,
                sweep_dir: 0, sweep_timer: 0, sweep_tick: 0, duty: 0,
                volume_init: 0, volume_cur: 0, envelope_dir: 0,
                envelope_period: 0, envelope_timer: 0,
                length_counter: 0, length_enabled: false,
                freq_divider: 0, freq_timer: 0, freq_counter: 0,
                sample_idx: 0,
            },
            ch2: SoundChannel2 {
                enabled: false, duty: 0, volume_init: 0, volume_cur: 0,
                envelope_dir: 0, envelope_period: 0, envelope_timer: 0,
                length_counter: 0, length_enabled: false,
                freq_divider: 0, freq_timer: 0, freq_counter: 0,
                sample_idx: 0,
            },
            ch3: SoundChannel3 {
                enabled: false, bank_select: false, volume_code: 0,
                length_counter: 0, length_enabled: false,
                freq_divider: 0, freq_timer: 0, freq_counter: 0,
                wave_ram: [0; 32], sample_idx: 0,
            },
            ch4: SoundChannel4 {
                enabled: false, volume_init: 0, volume_cur: 0,
                envelope_dir: 0, envelope_period: 0, envelope_timer: 0,
                length_counter: 0, length_enabled: false,
                shift_freq: 0, width_mode: 0, div_ratio: 0,
                lfsr: 0x7FFF, freq_timer: 0, freq_counter: 0,
            },
            fifo_a: FifoChannel::new(),
            fifo_b: FifoChannel::new(),
            sound_on: false,
            sound1_vol: 0, sound2_vol: 0,
            master_vol_left: 0, master_vol_right: 0,
            sound_out_mix: 0,
            envelope_tick_counter: 0,
        }
    }

    pub fn write_soundcnt_l(&mut self, value: u16) {
        self.master_vol_left = (value & 0x07) as u8;
        self.master_vol_right = ((value >> 8) & 0x07) as u8;
        self.sound1_vol = ((value >> 12) & 1) as u8;
        self.sound2_vol = ((value >> 13) & 1) as u8;
    }

    pub fn write_soundcnt_h(&mut self, value: u16) {
        self.sound_out_mix = (value & 0xFF) as u8;
        self.sound_on = value & 0x8000 != 0;
    }

    pub fn write_sound1_reg(&mut self, reg: u32, value: u8) {
        match reg {
            0x60 => { // SOUND1CNT_L - Sweep
                self.ch1.sweep_shift = value & 0x07;
                self.ch1.sweep_dir = (value >> 3) & 1;
                self.ch1.sweep_timer = (value >> 4) & 0x07;
                self.ch1.sweep_enabled = value != 0;
            }
            0x62 => { // SOUND1CNT_H - Duty/Volume/Envelope
                self.ch1.duty = (value >> 6) & 3;
                self.ch1.volume_init = (value >> 4) & 0x0F;
                self.ch1.volume_cur = self.ch1.volume_init;
                self.ch1.envelope_dir = (value >> 3) & 1;
                self.ch1.envelope_period = value & 0x07;
                self.ch1.envelope_timer = 0;
            }
            0x63 => { // SOUND1CNT_X low
                self.ch1.length_counter = value as u16;
            }
            0x64 => { // SOUND1CNT_X high
                self.ch1.freq_divider = ((value as u16) << 8) | (self.ch1.freq_divider & 0xFF);
                if value & 0x80 != 0 {
                    self.ch1.enabled = true;
                    self.ch1.freq_counter = 0;
                    self.ch1.sample_idx = 0;
                }
            }
            _ => {}
        }
    }

    pub fn write_sound2_reg(&mut self, reg: u32, value: u8) {
        match reg {
            0x68 => {
                self.ch2.duty = (value >> 6) & 3;
                self.ch2.volume_init = (value >> 4) & 0x0F;
                self.ch2.volume_cur = self.ch2.volume_init;
                self.ch2.envelope_dir = (value >> 3) & 1;
                self.ch2.envelope_period = value & 0x07;
            }
            0x6C => {
                self.ch2.length_counter = value as u16;
            }
            0x6E => {
                self.ch2.freq_divider = ((value as u16) << 8) | (self.ch2.freq_divider & 0xFF);
                if value & 0x80 != 0 {
                    self.ch2.enabled = true;
                    self.ch2.freq_counter = 0;
                    self.ch2.sample_idx = 0;
                }
            }
            _ => {}
        }
    }

    pub fn write_sound3_reg(&mut self, reg: u32, value: u8) {
        match reg {
            0x70 => {
                self.ch3.bank_select = value & 0x20 != 0;
                self.ch3.volume_code = (value >> 5) & 3;
            }
            0x72 => {
                self.ch3.length_counter = value as u16;
            }
            0x74 => {
                self.ch3.freq_divider = ((value as u16) & 0x7F) | (self.ch3.freq_divider & 0xFF80);
                if value & 0x80 != 0 {
                    self.ch3.enabled = true;
                    self.ch3.freq_counter = 0;
                    self.ch3.sample_idx = 0;
                }
            }
            _ => {}
        }
    }

    pub fn write_sound4_reg(&mut self, reg: u32, value: u8) {
        match reg {
            0x78 => {
                self.ch4.volume_init = (value >> 4) & 0x0F;
                self.ch4.volume_cur = self.ch4.volume_init;
                self.ch4.envelope_dir = (value >> 3) & 1;
                self.ch4.envelope_period = value & 0x07;
            }
            0x7A => {
                self.ch4.length_counter = value as u16;
            }
            0x7C => {
                self.ch4.shift_freq = (value >> 4) & 0x0F;
                self.ch4.width_mode = (value >> 3) & 1;
                self.ch4.div_ratio = value & 0x07;
            }
            0x7E => {
                if value & 0x80 != 0 {
                    self.ch4.enabled = true;
                    self.ch4.lfsr = 0x7FFF;
                    self.ch4.freq_counter = 0;
                }
            }
            _ => {}
        }
    }

    /// Push a byte to FIFO A (Direct Sound A)
    pub fn write_fifo_a(&mut self, value: i8) {
        self.fifo_a.push(value);
    }

    /// Push a byte to FIFO B (Direct Sound B)
    pub fn write_fifo_b(&mut self, value: i8) {
        self.fifo_b.push(value);
    }

    /// Check if FIFO A is half-empty (for DMA refill)
    pub fn fifo_a_half_empty(&self) -> bool {
        self.fifo_a.is_half_empty()
    }

    /// Check if FIFO B is half-empty (for DMA refill)
    pub fn fifo_b_half_empty(&self) -> bool {
        self.fifo_b.is_half_empty()
    }

    fn duty_wave(duty: u8, idx: u32) -> f32 {
        let phase = idx % 8;
        match duty {
            0 => if phase < 1 { 1.0 } else { -1.0 },  // 12.5%
            1 => if phase < 2 { 1.0 } else { -1.0 },  // 25%
            2 => if phase < 4 { 1.0 } else { -1.0 },  // 50%
            3 => if phase < 6 { 1.0 } else { -1.0 },  // 75%
            _ => 0.0,
        }
    }

    pub fn tick(&mut self, cycles: u32) {
        if !self.sound_on {
            self.cycle_counter += cycles;
            while self.cycle_counter >= CYCLES_PER_SAMPLE {
                self.cycle_counter -= CYCLES_PER_SAMPLE;
                self.sample_buffer.push(0.0);
            }
            return;
        }

        self.cycle_counter += cycles;

        // Envelope timer: tick every ~8 scanlines (59.73 Hz)
        // 1232 cycles/scanline * 8 = 9856 cycles
        self.envelope_tick_counter += cycles;
        if self.envelope_tick_counter >= 9856 {
            self.envelope_tick_counter -= 9856;
            self.tick_envelopes();
            self.tick_sweep();
            self.tick_length_counters();
        }

        while self.cycle_counter >= CYCLES_PER_SAMPLE {
            self.cycle_counter -= CYCLES_PER_SAMPLE;

            let mut sample = 0.0f32;

            // Channel 1: Square wave with sweep
            if self.ch1.enabled {
                self.ch1.freq_counter += 1;
                let period = 2048u16.saturating_sub(self.ch1.freq_divider);
                if period > 0 && self.ch1.freq_counter >= period {
                    self.ch1.freq_counter = 0;
                    self.ch1.sample_idx += 1;
                }
                let duty_sample = Self::duty_wave(self.ch1.duty, self.ch1.sample_idx);
                sample += duty_sample * (self.ch1.volume_cur as f32 / 15.0) * 0.25;
            }

            // Channel 2: Square wave
            if self.ch2.enabled {
                self.ch2.freq_counter += 1;
                let period = 2048u16.saturating_sub(self.ch2.freq_divider);
                if period > 0 && self.ch2.freq_counter >= period {
                    self.ch2.freq_counter = 0;
                    self.ch2.sample_idx += 1;
                }
                let duty_sample = Self::duty_wave(self.ch2.duty, self.ch2.sample_idx);
                sample += duty_sample * (self.ch2.volume_cur as f32 / 15.0) * 0.25;
            }

            // Channel 3: Wave
            if self.ch3.enabled {
                self.ch3.freq_counter += 1;
                let period = 2048u16.saturating_sub(self.ch3.freq_divider);
                if period > 0 && self.ch3.freq_counter >= period {
                    self.ch3.freq_counter = 0;
                    self.ch3.sample_idx += 1;
                }
                let wave_idx = (self.ch3.sample_idx % 32) as usize;
                let wave_byte = self.ch3.wave_ram[wave_idx / 2];
                let nibble = if wave_idx % 2 == 0 { wave_byte >> 4 } else { wave_byte & 0x0F };
                let vol_shift = match self.ch3.volume_code {
                    0 => 4,
                    1 => 3,
                    2 => 2,
                    3 => 1,
                    _ => 4,
                };
                sample += ((nibble as f32 / 15.0) - 0.5) * (1.0 / vol_shift as f32);
            }

            // Channel 4: Noise
            if self.ch4.enabled {
                self.ch4.freq_counter += 1;
                let freq = if self.ch4.div_ratio == 0 { 8u16 } else { self.ch4.div_ratio as u16 * 16 };
                let period = freq << self.ch4.shift_freq;
                if period > 0 && self.ch4.freq_counter >= period {
                    self.ch4.freq_counter = 0;
                    // LFSR
                    let xor_bit = (self.ch4.lfsr & 1) ^ ((self.ch4.lfsr >> 1) & 1);
                    self.ch4.lfsr = (self.ch4.lfsr >> 1) | (xor_bit << 14);
                    if self.ch4.width_mode == 1 {
                        self.ch4.lfsr = (self.ch4.lfsr & !0x40) | (xor_bit << 6);
                    }
                }
                let noise_sample = if self.ch4.lfsr & 1 == 0 { 1.0 } else { -1.0 };
                sample += noise_sample * (self.ch4.volume_cur as f32 / 15.0) * 0.15;
            }

            // FIFO A (Direct Sound A)
            if !self.fifo_a.is_empty() {
                let fifo_a_sample = self.fifo_a.pop() as f32 / 128.0;
                let vol_a = if self.sound1_vol == 1 { 2.0 } else { 1.0 };
                sample += fifo_a_sample * vol_a * 0.5;
            }

            // FIFO B (Direct Sound B)
            if !self.fifo_b.is_empty() {
                let fifo_b_sample = self.fifo_b.pop() as f32 / 128.0;
                let vol_b = if self.sound2_vol == 1 { 2.0 } else { 1.0 };
                sample += fifo_b_sample * vol_b * 0.5;
            }

            self.sample_buffer.push(sample.clamp(-1.0, 1.0));
        }
    }

    fn tick_envelopes(&mut self) {
        // Channel 1 envelope
        if self.ch1.enabled && self.ch1.envelope_period > 0 {
            self.ch1.envelope_timer += 1;
            if self.ch1.envelope_timer >= self.ch1.envelope_period {
                self.ch1.envelope_timer = 0;
                if self.ch1.envelope_dir == 1 && self.ch1.volume_cur < 15 {
                    self.ch1.volume_cur += 1;
                } else if self.ch1.envelope_dir == 0 && self.ch1.volume_cur > 0 {
                    self.ch1.volume_cur -= 1;
                }
            }
        }
        // Channel 2 envelope
        if self.ch2.enabled && self.ch2.envelope_period > 0 {
            self.ch2.envelope_timer += 1;
            if self.ch2.envelope_timer >= self.ch2.envelope_period {
                self.ch2.envelope_timer = 0;
                if self.ch2.envelope_dir == 1 && self.ch2.volume_cur < 15 {
                    self.ch2.volume_cur += 1;
                } else if self.ch2.envelope_dir == 0 && self.ch2.volume_cur > 0 {
                    self.ch2.volume_cur -= 1;
                }
            }
        }
        // Channel 4 envelope
        if self.ch4.enabled && self.ch4.envelope_period > 0 {
            self.ch4.envelope_timer += 1;
            if self.ch4.envelope_timer >= self.ch4.envelope_period {
                self.ch4.envelope_timer = 0;
                if self.ch4.envelope_dir == 1 && self.ch4.volume_cur < 15 {
                    self.ch4.volume_cur += 1;
                } else if self.ch4.envelope_dir == 0 && self.ch4.volume_cur > 0 {
                    self.ch4.volume_cur -= 1;
                }
            }
        }
    }

    fn tick_sweep(&mut self) {
        // Channel 1 sweep
        if self.ch1.enabled && self.ch1.sweep_enabled && self.ch1.sweep_timer > 0 {
            self.ch1.sweep_tick += 1;
            if self.ch1.sweep_tick >= self.ch1.sweep_timer {
                self.ch1.sweep_tick = 0;
                let delta = self.ch1.freq_divider >> self.ch1.sweep_shift;
                if self.ch1.sweep_dir == 0 {
                    // Increase frequency
                    self.ch1.freq_divider = self.ch1.freq_divider.wrapping_add(delta);
                    if self.ch1.freq_divider > 0x7FF {
                        self.ch1.enabled = false;
                    }
                } else {
                    // Decrease frequency
                    self.ch1.freq_divider = self.ch1.freq_divider.wrapping_sub(delta);
                }
            }
        }
    }

    fn tick_length_counters(&mut self) {
        if self.ch1.enabled && self.ch1.length_enabled {
            self.ch1.length_counter += 1;
            if self.ch1.length_counter >= 64 {
                self.ch1.enabled = false;
            }
        }
        if self.ch2.enabled && self.ch2.length_enabled {
            self.ch2.length_counter += 1;
            if self.ch2.length_counter >= 64 {
                self.ch2.enabled = false;
            }
        }
        if self.ch3.enabled && self.ch3.length_enabled {
            self.ch3.length_counter += 1;
            if self.ch3.length_counter >= 256 {
                self.ch3.enabled = false;
            }
        }
        if self.ch4.enabled && self.ch4.length_enabled {
            self.ch4.length_counter += 1;
            if self.ch4.length_counter >= 64 {
                self.ch4.enabled = false;
            }
        }
    }

    pub fn samples(&self) -> &[f32] {
        &self.sample_buffer
    }

    pub fn clear_buffer(&mut self) {
        self.sample_buffer.clear();
    }
}
