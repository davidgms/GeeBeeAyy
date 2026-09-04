// GBA APU (Audio Processing Unit)
// 4 PSG channels + 2 FIFO channels

/// The GBA system clock.
const GBA_CLOCK: u64 = 16_777_216;

/// Channel 3's "force 75%" (SOUND3CNT_H bit 15) as an internal volume code.
///
/// Kept outside the 0-3 range the two-bit volume field uses so the two cannot
/// collide; it is never a value read from or written to a register.
const FORCE_75_VOLUME_CODE: u8 = 4;

/// The rate the core hands samples to the frontend.
///
/// 48 kHz is what Android's mixer runs at natively. The old rate was
/// `16_777_216 / 964` = 17403 Hz, which no device supports, so every track
/// went through the resampler and `AUDIO_OUTPUT_FLAG_FAST` was refused. The
/// sample clock below is an exact fraction rather than an integer cycle
/// count, so this rate carries no rounding drift.
pub const SAMPLE_RATE: u32 = 48_000;

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
    /// Cycles owed to this channel's phase counter. The phase is clocked by
    /// the system clock, not by output samples - clocking it per sample tied
    /// the pitch to the sample rate and put every PSG channel 64x flat.
    cycle_accum: u32,
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
    cycle_accum: u32,
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
    cycle_accum: u32,
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
    cycle_accum: u32,
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
    /// Empty the FIFO, as SOUNDCNT_H's reset bits do.
    fn reset(&mut self) {
        self.buffer = [0; 32];
        self.read_pos = 0;
        self.write_pos = 0;
        self.count = 0;
    }

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
    /// Sample clock, in units of cycles * SAMPLE_RATE. A sample falls due
    /// each time it reaches GBA_CLOCK, which makes the output rate an exact
    /// fraction of the system clock with no accumulated rounding error.
    sample_accum: u64,
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
    /// Timer that drives FIFO playback (0 or 1)
    pub fifo_timer: u8,
    fifo_b_timer: u8,
    /// The byte each FIFO's timer last popped. DMA sound holds that value
    /// until the next overflow - the FIFO is not read at the output rate.
    fifo_a_latch: i8,
    fifo_b_latch: i8,
    /// Cycles accumulated for FIFO sample output
    fifo_cycles: u32,
    /// Whether FIFO A/B are enabled via SOUNDCNT_H
    pub fifo_a_enabled: bool,
    pub fifo_b_enabled: bool,
}

impl Apu {
    pub fn new() -> Self {
        Self {
            sample_accum: 0,
            sample_buffer: Vec::new(),
            ch1: SoundChannel1 {
                enabled: false,
                sweep_enabled: false,
                sweep_shift: 0,
                sweep_dir: 0,
                sweep_timer: 0,
                sweep_tick: 0,
                duty: 0,
                volume_init: 0,
                volume_cur: 0,
                envelope_dir: 0,
                envelope_period: 0,
                envelope_timer: 0,
                length_counter: 0,
                length_enabled: false,
                freq_divider: 0,
                freq_timer: 0,
                freq_counter: 0,
                cycle_accum: 0,
                sample_idx: 0,
            },
            ch2: SoundChannel2 {
                enabled: false,
                duty: 0,
                volume_init: 0,
                volume_cur: 0,
                envelope_dir: 0,
                envelope_period: 0,
                envelope_timer: 0,
                length_counter: 0,
                length_enabled: false,
                freq_divider: 0,
                freq_timer: 0,
                freq_counter: 0,
                cycle_accum: 0,
                sample_idx: 0,
            },
            ch3: SoundChannel3 {
                enabled: false,
                bank_select: false,
                volume_code: 0,
                length_counter: 0,
                length_enabled: false,
                freq_divider: 0,
                freq_timer: 0,
                freq_counter: 0,
                cycle_accum: 0,
                wave_ram: [0; 32],
                sample_idx: 0,
            },
            ch4: SoundChannel4 {
                enabled: false,
                volume_init: 0,
                volume_cur: 0,
                envelope_dir: 0,
                envelope_period: 0,
                envelope_timer: 0,
                length_counter: 0,
                length_enabled: false,
                shift_freq: 0,
                width_mode: 0,
                div_ratio: 0,
                lfsr: 0x7FFF,
                freq_timer: 0,
                freq_counter: 0,
                cycle_accum: 0,
            },
            fifo_a: FifoChannel::new(),
            fifo_b: FifoChannel::new(),
            sound_on: false,
            sound1_vol: 0,
            sound2_vol: 0,
            master_vol_left: 0,
            master_vol_right: 0,
            sound_out_mix: 0,
            envelope_tick_counter: 0,
            fifo_timer: 0,
            fifo_b_timer: 0,
            fifo_a_latch: 0,
            fifo_b_latch: 0,
            fifo_cycles: 0,
            fifo_a_enabled: false,
            fifo_b_enabled: false,
        }
    }

    pub fn write_soundcnt_l(&mut self, value: u16) {
        self.master_vol_left = (value & 0x07) as u8;
        self.master_vol_right = ((value >> 8) & 0x07) as u8;
    }

    pub fn write_soundcnt_h(&mut self, value: u16) {
        self.sound_out_mix = (value & 0xFF) as u8;
        // Bit 15 here is "DMA Sound B Reset FIFO", not a master enable
        // (GBATEK, Sound Control Registers). The master enable is SOUNDCNT_X
        // bit 7 - see `write_soundcnt_x`.
        if value & 0x8000 != 0 {
            self.fifo_b.reset();
        }
        if value & 0x0800 != 0 {
            self.fifo_a.reset();
        }
        // GBATEK, SOUNDCNT_H: bit 2 is DMA sound A's volume and bit 3 is B's -
        // they are not in SOUNDCNT_L, where this used to read them from bits
        // 12 and 13, which are two of the PSG channel enables.
        self.sound1_vol = ((value >> 2) & 1) as u8;
        self.sound2_vol = ((value >> 3) & 1) as u8;
        // A channel is enabled if it is routed to either speaker: bits 8 and 9
        // for A, 12 and 13 for B.
        self.fifo_a_enabled = value & 0x0300 != 0;
        self.fifo_b_enabled = value & 0x3000 != 0;
        // Each FIFO picks its own timer: bit 10 for A, bit 14 for B.
        self.fifo_timer = ((value >> 10) & 1) as u8;
        self.fifo_b_timer = ((value >> 14) & 1) as u8;
    }

    /// SOUNDCNT_X (0x04000084). GBATEK: "Bit 7 R/W PSG/FIFO Master Enable
    /// (0=Disable, 1=Enable)". This register was not routed to the APU at all,
    /// so the master enable never took effect and `sound_on` was being derived
    /// from the wrong bit of the wrong register.
    pub fn write_soundcnt_x(&mut self, value: u16) {
        self.sound_on = value & 0x0080 != 0;
    }

    /// Write a 16-bit sound register, addressed by its offset from 0x04000000.
    ///
    /// Decoding happens at 16-bit granularity on purpose. The previous
    /// byte-level decoders had `SOUND1CNT_H`'s split off by one - `0x62` read
    /// the envelope fields that GBATEK places in `0x63`, `0x63` read a length
    /// that belongs in `0x62`, and `0x65` had no handler at all, which is where
    /// the trigger bit lives. No PSG channel could ever be started.
    pub fn write_register(&mut self, reg: u32, value: u16) {
        match reg {
            // SOUND1CNT_L - sweep
            0x60 => {
                self.ch1.sweep_shift = (value & 0x07) as u8;
                self.ch1.sweep_dir = ((value >> 3) & 1) as u8;
                self.ch1.sweep_timer = ((value >> 4) & 0x07) as u8;
                self.ch1.sweep_enabled = self.ch1.sweep_timer != 0;
            }
            // SOUND1CNT_H - length, duty, envelope
            0x62 => self.write_duty_envelope(0, value),
            // SOUND1CNT_X - frequency and control
            0x64 => self.write_frequency_control(0, value),

            // SOUND2CNT_L / SOUND2CNT_H, same layout minus the sweep
            0x68 => self.write_duty_envelope(1, value),
            0x6C => self.write_frequency_control(1, value),

            // SOUND3CNT_L - wave RAM bank and enable
            0x70 => {
                self.ch3.bank_select = value & 0x0040 != 0;
                self.ch3.enabled = value & 0x0080 != 0;
            }
            // SOUND3CNT_H - length and volume
            0x72 => {
                self.ch3.length_counter = value & 0xFF;
                // GBATEK, SOUND3CNT_H: bits 13-14 are the volume (0 = mute,
                // 1 = 100%, 2 = 50%, 3 = 25%) and bit 15 is a separate
                // "force 75%" that overrides them. Masking three bits folded
                // the override into the volume field, so a game asking for
                // 75% produced codes 4-7, which the mixer treated as mute.
                self.ch3.volume_code = if value & 0x8000 != 0 {
                    FORCE_75_VOLUME_CODE
                } else {
                    ((value >> 13) & 0x03) as u8
                };
            }
            // SOUND3CNT_X - frequency and control
            0x74 => self.write_frequency_control(2, value),

            // SOUND4CNT_L - length and envelope
            0x78 => {
                self.ch4.length_counter = value & 0x3F;
                self.ch4.envelope_period = ((value >> 8) & 0x07) as u8;
                self.ch4.envelope_dir = ((value >> 11) & 1) as u8;
                self.ch4.volume_init = ((value >> 12) & 0x0F) as u8;
                self.ch4.volume_cur = self.ch4.volume_init;
                self.ch4.envelope_timer = 0;
            }
            // SOUND4CNT_H - noise parameters and control
            0x7C => {
                self.ch4.div_ratio = (value & 0x07) as u8;
                self.ch4.width_mode = ((value >> 3) & 1) as u8;
                self.ch4.shift_freq = ((value >> 4) & 0x0F) as u8;
                self.ch4.length_enabled = value & 0x4000 != 0;
                if value & 0x8000 != 0 {
                    self.ch4.enabled = true;
                    self.ch4.lfsr = 0x7FFF;
                    self.ch4.freq_counter = 0;
                }
            }

            0x80 => self.write_soundcnt_l(value),
            0x82 => self.write_soundcnt_h(value),
            0x84 => self.write_soundcnt_x(value),

            // Wave RAM, which the routing table used to drop entirely.
            0x90..=0x9F => {
                let idx = (reg - 0x90) as usize;
                if idx + 1 < self.ch3.wave_ram.len() {
                    self.ch3.wave_ram[idx] = (value & 0xFF) as u8;
                    self.ch3.wave_ram[idx + 1] = (value >> 8) as u8;
                }
            }
            _ => {}
        }
    }

    /// Shared by channels 1 and 2: length, duty and the volume envelope.
    /// GBATEK: bits 0-5 length, 6-7 duty, 8-10 envelope step, 11 direction,
    /// 12-15 initial volume.
    fn write_duty_envelope(&mut self, channel: usize, value: u16) {
        let length = value & 0x3F;
        let duty = ((value >> 6) & 3) as u8;
        let period = ((value >> 8) & 0x07) as u8;
        let dir = ((value >> 11) & 1) as u8;
        let volume = ((value >> 12) & 0x0F) as u8;
        if channel == 0 {
            self.ch1.length_counter = length;
            self.ch1.duty = duty;
            self.ch1.envelope_period = period;
            self.ch1.envelope_dir = dir;
            self.ch1.volume_init = volume;
            self.ch1.volume_cur = volume;
            self.ch1.envelope_timer = 0;
        } else {
            self.ch2.length_counter = length;
            self.ch2.duty = duty;
            self.ch2.envelope_period = period;
            self.ch2.envelope_dir = dir;
            self.ch2.volume_init = volume;
            self.ch2.volume_cur = volume;
            self.ch2.envelope_timer = 0;
        }
    }

    /// Shared by channels 1, 2 and 3: frequency in bits 0-10, the length flag
    /// in bit 14 and the restart trigger in bit 15.
    fn write_frequency_control(&mut self, channel: usize, value: u16) {
        let freq = value & 0x07FF;
        let length_enabled = value & 0x4000 != 0;
        let trigger = value & 0x8000 != 0;
        match channel {
            0 => {
                self.ch1.freq_divider = freq;
                self.ch1.length_enabled = length_enabled;
                if trigger {
                    self.ch1.enabled = true;
                    self.ch1.freq_counter = 0;
                    self.ch1.sample_idx = 0;
                    self.ch1.volume_cur = self.ch1.volume_init;
                }
            }
            1 => {
                self.ch2.freq_divider = freq;
                self.ch2.length_enabled = length_enabled;
                if trigger {
                    self.ch2.enabled = true;
                    self.ch2.freq_counter = 0;
                    self.ch2.sample_idx = 0;
                    self.ch2.volume_cur = self.ch2.volume_init;
                }
            }
            _ => {
                self.ch3.freq_divider = freq;
                self.ch3.length_enabled = length_enabled;
                if trigger {
                    self.ch3.enabled = true;
                    self.ch3.freq_counter = 0;
                    self.ch3.sample_idx = 0;
                }
            }
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

    /// A timer overflowed. This - and nothing else - advances DMA sound: each
    /// FIFO pops one byte when its selected timer overflows, and the mixer
    /// holds that byte until the next one.
    ///
    /// This used to be an empty stub while the mixer popped the FIFOs itself,
    /// once per output sample. That drained them far faster than the game's
    /// timer refilled them, so the sound came out as a burst every few frames
    /// with silence in between - a thump, not music - and it played at the
    /// output rate instead of the rate the game asked for.
    pub fn on_timer_overflow(&mut self, timer: u8) {
        if !self.sound_on {
            return;
        }
        if timer == self.fifo_timer && !self.fifo_a.is_empty() {
            self.fifo_a_latch = self.fifo_a.pop();
        }
        if timer == self.fifo_b_timer && !self.fifo_b.is_empty() {
            self.fifo_b_latch = self.fifo_b.pop();
        }
    }

    /// Advance every channel's phase by `cycles` of the system clock.
    ///
    /// GBATEK, GBA Sound Channels: a square channel's tone is
    /// `131072/(2048-n)` Hz and its duty waveform has eight phases, so one
    /// phase lasts `16 * (2048-n)` cycles. Channel 3 walks 32 wave samples at
    /// `2097152/(2048-n)` Hz, one every `8 * (2048-n)` cycles. Channel 4's
    /// generator runs at `524288/r/2^(s+1)` Hz with `r=0` meaning 0.5, so one
    /// step is `32*r*2^(s+1)` cycles - `16*2^(s+1)` when `r` is zero.
    fn tick_channels(&mut self, cycles: u32) {
        let square = |divider: u16| -> u32 { 16 * (2048u32 - divider.min(2047) as u32) };

        if self.ch1.enabled {
            let period = square(self.ch1.freq_divider);
            self.ch1.cycle_accum += cycles;
            while self.ch1.cycle_accum >= period {
                self.ch1.cycle_accum -= period;
                self.ch1.sample_idx = self.ch1.sample_idx.wrapping_add(1);
            }
        }
        if self.ch2.enabled {
            let period = square(self.ch2.freq_divider);
            self.ch2.cycle_accum += cycles;
            while self.ch2.cycle_accum >= period {
                self.ch2.cycle_accum -= period;
                self.ch2.sample_idx = self.ch2.sample_idx.wrapping_add(1);
            }
        }
        if self.ch3.enabled {
            let period = 8 * (2048u32 - self.ch3.freq_divider.min(2047) as u32);
            self.ch3.cycle_accum += cycles;
            while self.ch3.cycle_accum >= period {
                self.ch3.cycle_accum -= period;
                self.ch3.sample_idx = self.ch3.sample_idx.wrapping_add(1);
            }
        }
        if self.ch4.enabled {
            let step = 1u32 << (self.ch4.shift_freq.min(15) + 1);
            let period = if self.ch4.div_ratio == 0 {
                16 * step
            } else {
                32 * self.ch4.div_ratio as u32 * step
            };
            self.ch4.cycle_accum += cycles;
            while self.ch4.cycle_accum >= period {
                self.ch4.cycle_accum -= period;
                let xor_bit = (self.ch4.lfsr & 1) ^ ((self.ch4.lfsr >> 1) & 1);
                self.ch4.lfsr = (self.ch4.lfsr >> 1) | (xor_bit << 14);
                if self.ch4.width_mode == 1 {
                    self.ch4.lfsr = (self.ch4.lfsr & !0x40) | (xor_bit << 6);
                }
            }
        }
    }

    fn duty_wave(duty: u8, idx: u32) -> f32 {
        let phase = idx % 8;
        match duty {
            0 => {
                if phase < 1 {
                    1.0
                } else {
                    -1.0
                }
            } // 12.5%
            1 => {
                if phase < 2 {
                    1.0
                } else {
                    -1.0
                }
            } // 25%
            2 => {
                if phase < 4 {
                    1.0
                } else {
                    -1.0
                }
            } // 50%
            3 => {
                if phase < 6 {
                    1.0
                } else {
                    -1.0
                }
            } // 75%
            _ => 0.0,
        }
    }

    pub fn tick(&mut self, cycles: u32) {
        if !self.sound_on {
            self.sample_accum += cycles as u64 * SAMPLE_RATE as u64;
            while self.sample_accum >= GBA_CLOCK {
                self.sample_accum -= GBA_CLOCK;
                self.sample_buffer.push(0.0);
            }
            return;
        }

        // The channel phases run off the system clock, so their pitch does not
        // move when the output rate does.
        self.tick_channels(cycles);

        // Envelope timer: tick every ~8 scanlines (59.73 Hz)
        // 1232 cycles/scanline * 8 = 9856 cycles
        self.envelope_tick_counter += cycles;
        if self.envelope_tick_counter >= 9856 {
            self.envelope_tick_counter -= 9856;
            self.tick_envelopes();
            self.tick_sweep();
            self.tick_length_counters();
        }

        self.sample_accum += cycles as u64 * SAMPLE_RATE as u64;
        while self.sample_accum >= GBA_CLOCK {
            self.sample_accum -= GBA_CLOCK;

            let mut sample = 0.0f32;

            // Channel 1: Square wave with sweep
            if self.ch1.enabled {
                let duty_sample = Self::duty_wave(self.ch1.duty, self.ch1.sample_idx);
                sample += duty_sample * (self.ch1.volume_cur as f32 / 15.0) * 0.25;
            }

            // Channel 2: Square wave
            if self.ch2.enabled {
                let duty_sample = Self::duty_wave(self.ch2.duty, self.ch2.sample_idx);
                sample += duty_sample * (self.ch2.volume_cur as f32 / 15.0) * 0.25;
            }

            // Channel 3: Wave
            if self.ch3.enabled {
                let wave_idx = (self.ch3.sample_idx % 32) as usize;
                let wave_byte = self.ch3.wave_ram[wave_idx / 2];
                let nibble = if wave_idx % 2 == 0 {
                    wave_byte >> 4
                } else {
                    wave_byte & 0x0F
                };
                // A gain, not a shift: code 0 is silence, and the bit-15
                // override is 75%, which no power-of-two divisor expresses.
                // The old table mapped code 0 to a quarter volume instead of
                // to nothing, and had 1 and 3 the wrong way round.
                let gain = match self.ch3.volume_code {
                    1 => 1.0,
                    2 => 0.5,
                    3 => 0.25,
                    FORCE_75_VOLUME_CODE => 0.75,
                    _ => 0.0,
                };
                sample += ((nibble as f32 / 15.0) - 0.5) * gain;
            }

            // Channel 4: Noise
            if self.ch4.enabled {
                let noise_sample = if self.ch4.lfsr & 1 == 0 { 1.0 } else { -1.0 };
                sample += noise_sample * (self.ch4.volume_cur as f32 / 15.0) * 0.15;
            }

            // FIFO A (Direct Sound A). The held latch, not a fresh pop.
            if self.fifo_a_enabled {
                let vol_a = if self.sound1_vol == 1 { 1.0 } else { 0.5 };
                sample += (self.fifo_a_latch as f32 / 128.0) * vol_a;
            }

            // FIFO B (Direct Sound B)
            if self.fifo_b_enabled {
                let vol_b = if self.sound2_vol == 1 { 1.0 } else { 0.5 };
                sample += (self.fifo_b_latch as f32 / 128.0) * vol_b;
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

// --- Save state serialisation ----------------------------------------------
//
// Generated as matching pairs so the write and read orders cannot drift apart -
// the only real risk across 78 fields of flat scalars. `sample_buffer` is
// deliberately absent: it is drained to the frontend every frame, so
// snapshotting it would replay stale audio on load.

/// Split `n` bytes off the front of the cursor, or `None` if short.
fn take<'a>(cur: &mut &'a [u8], n: usize) -> Option<&'a [u8]> {
    if cur.len() < n {
        return None;
    }
    let (head, rest) = cur.split_at(n);
    *cur = rest;
    Some(head)
}

impl SoundChannel1 {
    fn write_state(&self, buf: &mut Vec<u8>) {
        buf.push(self.enabled as u8);
        buf.push(self.sweep_enabled as u8);
        buf.extend_from_slice(&self.sweep_shift.to_le_bytes());
        buf.extend_from_slice(&self.sweep_dir.to_le_bytes());
        buf.extend_from_slice(&self.sweep_timer.to_le_bytes());
        buf.extend_from_slice(&self.sweep_tick.to_le_bytes());
        buf.extend_from_slice(&self.duty.to_le_bytes());
        buf.extend_from_slice(&self.volume_init.to_le_bytes());
        buf.extend_from_slice(&self.volume_cur.to_le_bytes());
        buf.extend_from_slice(&self.envelope_dir.to_le_bytes());
        buf.extend_from_slice(&self.envelope_period.to_le_bytes());
        buf.extend_from_slice(&self.envelope_timer.to_le_bytes());
        buf.extend_from_slice(&self.length_counter.to_le_bytes());
        buf.push(self.length_enabled as u8);
        buf.extend_from_slice(&self.freq_divider.to_le_bytes());
        buf.extend_from_slice(&self.freq_timer.to_le_bytes());
        buf.extend_from_slice(&self.freq_counter.to_le_bytes());
        buf.extend_from_slice(&self.sample_idx.to_le_bytes());
    }

    fn read_state(&mut self, cur: &mut &[u8]) -> Option<()> {
        self.enabled = take(cur, 1)?[0] != 0;
        self.sweep_enabled = take(cur, 1)?[0] != 0;
        self.sweep_shift = u8::from_le_bytes(take(cur, 1)?.try_into().ok()?);
        self.sweep_dir = u8::from_le_bytes(take(cur, 1)?.try_into().ok()?);
        self.sweep_timer = u8::from_le_bytes(take(cur, 1)?.try_into().ok()?);
        self.sweep_tick = u8::from_le_bytes(take(cur, 1)?.try_into().ok()?);
        self.duty = u8::from_le_bytes(take(cur, 1)?.try_into().ok()?);
        self.volume_init = u8::from_le_bytes(take(cur, 1)?.try_into().ok()?);
        self.volume_cur = u8::from_le_bytes(take(cur, 1)?.try_into().ok()?);
        self.envelope_dir = u8::from_le_bytes(take(cur, 1)?.try_into().ok()?);
        self.envelope_period = u8::from_le_bytes(take(cur, 1)?.try_into().ok()?);
        self.envelope_timer = u8::from_le_bytes(take(cur, 1)?.try_into().ok()?);
        self.length_counter = u16::from_le_bytes(take(cur, 2)?.try_into().ok()?);
        self.length_enabled = take(cur, 1)?[0] != 0;
        self.freq_divider = u16::from_le_bytes(take(cur, 2)?.try_into().ok()?);
        self.freq_timer = u16::from_le_bytes(take(cur, 2)?.try_into().ok()?);
        self.freq_counter = u16::from_le_bytes(take(cur, 2)?.try_into().ok()?);
        self.sample_idx = u32::from_le_bytes(take(cur, 4)?.try_into().ok()?);
        Some(())
    }
}

impl SoundChannel2 {
    fn write_state(&self, buf: &mut Vec<u8>) {
        buf.push(self.enabled as u8);
        buf.extend_from_slice(&self.duty.to_le_bytes());
        buf.extend_from_slice(&self.volume_init.to_le_bytes());
        buf.extend_from_slice(&self.volume_cur.to_le_bytes());
        buf.extend_from_slice(&self.envelope_dir.to_le_bytes());
        buf.extend_from_slice(&self.envelope_period.to_le_bytes());
        buf.extend_from_slice(&self.envelope_timer.to_le_bytes());
        buf.extend_from_slice(&self.length_counter.to_le_bytes());
        buf.push(self.length_enabled as u8);
        buf.extend_from_slice(&self.freq_divider.to_le_bytes());
        buf.extend_from_slice(&self.freq_timer.to_le_bytes());
        buf.extend_from_slice(&self.freq_counter.to_le_bytes());
        buf.extend_from_slice(&self.sample_idx.to_le_bytes());
    }

    fn read_state(&mut self, cur: &mut &[u8]) -> Option<()> {
        self.enabled = take(cur, 1)?[0] != 0;
        self.duty = u8::from_le_bytes(take(cur, 1)?.try_into().ok()?);
        self.volume_init = u8::from_le_bytes(take(cur, 1)?.try_into().ok()?);
        self.volume_cur = u8::from_le_bytes(take(cur, 1)?.try_into().ok()?);
        self.envelope_dir = u8::from_le_bytes(take(cur, 1)?.try_into().ok()?);
        self.envelope_period = u8::from_le_bytes(take(cur, 1)?.try_into().ok()?);
        self.envelope_timer = u8::from_le_bytes(take(cur, 1)?.try_into().ok()?);
        self.length_counter = u16::from_le_bytes(take(cur, 2)?.try_into().ok()?);
        self.length_enabled = take(cur, 1)?[0] != 0;
        self.freq_divider = u16::from_le_bytes(take(cur, 2)?.try_into().ok()?);
        self.freq_timer = u16::from_le_bytes(take(cur, 2)?.try_into().ok()?);
        self.freq_counter = u16::from_le_bytes(take(cur, 2)?.try_into().ok()?);
        self.sample_idx = u32::from_le_bytes(take(cur, 4)?.try_into().ok()?);
        Some(())
    }
}

impl SoundChannel3 {
    fn write_state(&self, buf: &mut Vec<u8>) {
        buf.push(self.enabled as u8);
        buf.push(self.bank_select as u8);
        buf.extend_from_slice(&self.volume_code.to_le_bytes());
        buf.extend_from_slice(&self.length_counter.to_le_bytes());
        buf.push(self.length_enabled as u8);
        buf.extend_from_slice(&self.freq_divider.to_le_bytes());
        buf.extend_from_slice(&self.freq_timer.to_le_bytes());
        buf.extend_from_slice(&self.freq_counter.to_le_bytes());
        buf.extend_from_slice(&self.wave_ram);
        buf.extend_from_slice(&self.sample_idx.to_le_bytes());
    }

    fn read_state(&mut self, cur: &mut &[u8]) -> Option<()> {
        self.enabled = take(cur, 1)?[0] != 0;
        self.bank_select = take(cur, 1)?[0] != 0;
        self.volume_code = u8::from_le_bytes(take(cur, 1)?.try_into().ok()?);
        self.length_counter = u16::from_le_bytes(take(cur, 2)?.try_into().ok()?);
        self.length_enabled = take(cur, 1)?[0] != 0;
        self.freq_divider = u16::from_le_bytes(take(cur, 2)?.try_into().ok()?);
        self.freq_timer = u16::from_le_bytes(take(cur, 2)?.try_into().ok()?);
        self.freq_counter = u16::from_le_bytes(take(cur, 2)?.try_into().ok()?);
        self.wave_ram.copy_from_slice(take(cur, 32)?);
        self.sample_idx = u32::from_le_bytes(take(cur, 4)?.try_into().ok()?);
        Some(())
    }
}

impl SoundChannel4 {
    fn write_state(&self, buf: &mut Vec<u8>) {
        buf.push(self.enabled as u8);
        buf.extend_from_slice(&self.volume_init.to_le_bytes());
        buf.extend_from_slice(&self.volume_cur.to_le_bytes());
        buf.extend_from_slice(&self.envelope_dir.to_le_bytes());
        buf.extend_from_slice(&self.envelope_period.to_le_bytes());
        buf.extend_from_slice(&self.envelope_timer.to_le_bytes());
        buf.extend_from_slice(&self.length_counter.to_le_bytes());
        buf.push(self.length_enabled as u8);
        buf.extend_from_slice(&self.shift_freq.to_le_bytes());
        buf.extend_from_slice(&self.width_mode.to_le_bytes());
        buf.extend_from_slice(&self.div_ratio.to_le_bytes());
        buf.extend_from_slice(&self.lfsr.to_le_bytes());
        buf.extend_from_slice(&self.freq_timer.to_le_bytes());
        buf.extend_from_slice(&self.freq_counter.to_le_bytes());
    }

    fn read_state(&mut self, cur: &mut &[u8]) -> Option<()> {
        self.enabled = take(cur, 1)?[0] != 0;
        self.volume_init = u8::from_le_bytes(take(cur, 1)?.try_into().ok()?);
        self.volume_cur = u8::from_le_bytes(take(cur, 1)?.try_into().ok()?);
        self.envelope_dir = u8::from_le_bytes(take(cur, 1)?.try_into().ok()?);
        self.envelope_period = u8::from_le_bytes(take(cur, 1)?.try_into().ok()?);
        self.envelope_timer = u8::from_le_bytes(take(cur, 1)?.try_into().ok()?);
        self.length_counter = u16::from_le_bytes(take(cur, 2)?.try_into().ok()?);
        self.length_enabled = take(cur, 1)?[0] != 0;
        self.shift_freq = u8::from_le_bytes(take(cur, 1)?.try_into().ok()?);
        self.width_mode = u8::from_le_bytes(take(cur, 1)?.try_into().ok()?);
        self.div_ratio = u8::from_le_bytes(take(cur, 1)?.try_into().ok()?);
        self.lfsr = u16::from_le_bytes(take(cur, 2)?.try_into().ok()?);
        self.freq_timer = u16::from_le_bytes(take(cur, 2)?.try_into().ok()?);
        self.freq_counter = u16::from_le_bytes(take(cur, 2)?.try_into().ok()?);
        Some(())
    }
}

impl FifoChannel {
    fn write_state(&self, buf: &mut Vec<u8>) {
        buf.extend(self.buffer.iter().map(|&v| v as u8));
        buf.extend_from_slice(&(self.read_pos as u32).to_le_bytes());
        buf.extend_from_slice(&(self.write_pos as u32).to_le_bytes());
        buf.extend_from_slice(&(self.count as u32).to_le_bytes());
        buf.extend_from_slice(&self.timer.to_le_bytes());
        buf.push(self.enabled as u8);
        buf.push(self.dma_refill as u8);
    }

    fn read_state(&mut self, cur: &mut &[u8]) -> Option<()> {
        for (i, &b) in take(cur, 32)?.iter().enumerate() {
            self.buffer[i] = b as i8;
        }
        self.read_pos = u32::from_le_bytes(take(cur, 4)?.try_into().ok()?) as usize;
        self.write_pos = u32::from_le_bytes(take(cur, 4)?.try_into().ok()?) as usize;
        self.count = u32::from_le_bytes(take(cur, 4)?.try_into().ok()?) as usize;
        self.timer = u8::from_le_bytes(take(cur, 1)?.try_into().ok()?);
        self.enabled = take(cur, 1)?[0] != 0;
        self.dma_refill = take(cur, 1)?[0] != 0;
        Some(())
    }
}

impl Apu {
    fn write_state(&self, buf: &mut Vec<u8>) {
        buf.extend_from_slice(&self.sample_accum.to_le_bytes());
        self.ch1.write_state(buf);
        self.ch2.write_state(buf);
        self.ch3.write_state(buf);
        self.ch4.write_state(buf);
        self.fifo_a.write_state(buf);
        self.fifo_b.write_state(buf);
        buf.push(self.sound_on as u8);
        buf.extend_from_slice(&self.sound1_vol.to_le_bytes());
        buf.extend_from_slice(&self.sound2_vol.to_le_bytes());
        buf.extend_from_slice(&self.master_vol_left.to_le_bytes());
        buf.extend_from_slice(&self.master_vol_right.to_le_bytes());
        buf.extend_from_slice(&self.sound_out_mix.to_le_bytes());
        buf.extend_from_slice(&self.envelope_tick_counter.to_le_bytes());
        buf.extend_from_slice(&self.fifo_timer.to_le_bytes());
        buf.extend_from_slice(&self.fifo_b_timer.to_le_bytes());
        buf.extend_from_slice(&self.fifo_a_latch.to_le_bytes());
        buf.extend_from_slice(&self.fifo_b_latch.to_le_bytes());
        buf.extend_from_slice(&self.fifo_cycles.to_le_bytes());
        buf.push(self.fifo_a_enabled as u8);
        buf.push(self.fifo_b_enabled as u8);
    }

    fn read_state(&mut self, cur: &mut &[u8]) -> Option<()> {
        self.sample_accum = u64::from_le_bytes(take(cur, 8)?.try_into().ok()?);
        self.ch1.read_state(cur)?;
        self.ch2.read_state(cur)?;
        self.ch3.read_state(cur)?;
        self.ch4.read_state(cur)?;
        self.fifo_a.read_state(cur)?;
        self.fifo_b.read_state(cur)?;
        self.sound_on = take(cur, 1)?[0] != 0;
        self.sound1_vol = u8::from_le_bytes(take(cur, 1)?.try_into().ok()?);
        self.sound2_vol = u8::from_le_bytes(take(cur, 1)?.try_into().ok()?);
        self.master_vol_left = u8::from_le_bytes(take(cur, 1)?.try_into().ok()?);
        self.master_vol_right = u8::from_le_bytes(take(cur, 1)?.try_into().ok()?);
        self.sound_out_mix = u8::from_le_bytes(take(cur, 1)?.try_into().ok()?);
        self.envelope_tick_counter = u32::from_le_bytes(take(cur, 4)?.try_into().ok()?);
        self.fifo_timer = u8::from_le_bytes(take(cur, 1)?.try_into().ok()?);
        self.fifo_b_timer = u8::from_le_bytes(take(cur, 1)?.try_into().ok()?);
        self.fifo_a_latch = i8::from_le_bytes(take(cur, 1)?.try_into().ok()?);
        self.fifo_b_latch = i8::from_le_bytes(take(cur, 1)?.try_into().ok()?);
        self.fifo_cycles = u32::from_le_bytes(take(cur, 4)?.try_into().ok()?);
        self.fifo_a_enabled = take(cur, 1)?[0] != 0;
        self.fifo_b_enabled = take(cur, 1)?[0] != 0;
        Some(())
    }
}

impl Apu {
    /// Serialise the whole APU for a save state.
    pub fn snapshot(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        self.write_state(&mut buf);
        buf
    }

    /// Restore from [`Apu::snapshot`]. Returns false if the data is short.
    pub fn restore(&mut self, data: &[u8]) -> bool {
        let mut cur = data;
        self.read_state(&mut cur).is_some()
    }
}
