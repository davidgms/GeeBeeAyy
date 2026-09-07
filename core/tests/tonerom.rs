use geebeeayy_core::Gba;

/// The tone ROM must actually make the APU emit samples. Before the register
/// map was rewritten no PSG channel could be triggered at all.
#[test]
fn the_tone_rom_produces_audio() {
    let path = format!("{}/../temp/roms/tone.gba", env!("CARGO_MANIFEST_DIR"));
    let Ok(rom) = std::fs::read(&path) else {
        return;
    };
    let mut gba = Gba::new();
    gba.load_rom(&rom).unwrap();
    gba.run_frame();
    gba.clear_audio_buffer();
    gba.run_frame();

    let s = gba.apu_samples();
    assert!(!s.is_empty(), "the APU produced no samples at all");
    let nonzero = s.iter().filter(|&&v| v != 0.0).count();
    assert!(
        nonzero > s.len() / 10,
        "expected a tone, got {nonzero}/{} non-zero samples",
        s.len()
    );
}

/// The PSG channels' pitch used to be tied to the output sample rate: their
/// phase advanced once per emitted sample rather than per system-clock cycle,
/// so the tone ROM's 128 Hz square came out at 2 Hz - 64x flat. Nobody heard
/// it because no game tested so far uses the PSG for music.
#[test]
fn a_psg_square_plays_the_frequency_the_rom_asked_for() {
    let path = format!("{}/../temp/roms/tone.gba", env!("CARGO_MANIFEST_DIR"));
    let Ok(rom) = std::fs::read(&path) else {
        return;
    };
    let mut gba = Gba::new();
    gba.load_rom(&rom).unwrap();

    // Settle, then collect a second of audio.
    for _ in 0..30 {
        gba.run_frame();
    }
    gba.clear_audio_buffer();
    let mut samples = Vec::new();
    for _ in 0..60 {
        gba.run_frame();
        samples.extend_from_slice(gba.apu_samples());
        gba.clear_audio_buffer();
    }

    // GBATEK: a square channel's tone is 131072/(2048-n) Hz.
    let n = (gba.bus.read16(0x0400_0064) & 0x07FF) as f64;
    let want = 131_072.0 / (2048.0 - n);

    let crossings = samples
        .windows(2)
        .filter(|w| (w[0] < 0.0) != (w[1] < 0.0))
        .count() as f64;
    let seconds = samples.len() as f64 / geebeeayy_core::apu::SAMPLE_RATE as f64;
    let got = crossings / 2.0 / seconds;

    assert!(
        (got - want).abs() / want < 0.05,
        "channel 1 asked for {want:.1} Hz and played {got:.1} Hz"
    );
}

/// The core hands the frontend 48 kHz, which is what Android's mixer runs at
/// natively. At the old 17403 Hz every track went through the resampler and
/// the low-latency path was refused outright.
#[test]
fn the_core_emits_samples_at_48_khz() {
    assert_eq!(geebeeayy_core::apu::SAMPLE_RATE, 48_000);

    let mut gba = Gba::new();
    gba.load_rom(&vec![0u8; 0x200]).expect("ROM should load");
    gba.clear_audio_buffer();
    let mut total = 0usize;
    for _ in 0..60 {
        gba.run_frame();
        total += gba.apu_samples().len();
        gba.clear_audio_buffer();
    }
    // Sixty frames is a shade over a second of emulated time.
    assert!(
        (47_000..49_500).contains(&total),
        "expected about 48000 samples a second, got {total}"
    );
}
