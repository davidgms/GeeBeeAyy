use geebeeayy_core::Gba;

/// The tone ROM must actually make the APU emit samples. Before the register
/// map was rewritten no PSG channel could be triggered at all.
#[test]
fn the_tone_rom_produces_audio() {
    let path = format!("{}/../temp/roms/tone.gba", env!("CARGO_MANIFEST_DIR"));
    let Ok(rom) = std::fs::read(&path) else { return };
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
