use geebee_core::Gba;
use std::env;
use std::fs;
use std::io::Write;
use std::path::Path;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: geebee <rom_file> [frames]");
        eprintln!("  rom_file: Path to GBA ROM (.gba)");
        eprintln!("  frames:   Number of frames to run (default: 10)");
        std::process::exit(1);
    }

    let rom_path = &args[1];
    let num_frames = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(10);

    if !Path::new(rom_path).exists() {
        eprintln!("Error: ROM file '{}' not found", rom_path);
        std::process::exit(1);
    }

    let rom_data = fs::read(rom_path).expect("Failed to read ROM file");
    println!("Loaded ROM: {} ({} bytes)", rom_path, rom_data.len());

    let mut gba = Gba::new();
    gba.load_rom(&rom_data).expect("Failed to load ROM");

    let mut all_samples: Vec<f32> = Vec::new();

    println!("Running {} frames...", num_frames);

    for frame in 0..num_frames {
        gba.run_frame();

        // Collect audio samples
        let samples = gba.apu_samples();
        all_samples.extend_from_slice(samples);
        gba.clear_audio_buffer();

        // Save frame as PPM image
        let frame_data = gba.frame_buffer();
        let ppm_path = format!("frame_{:04}.ppm", frame);
        save_ppm(&ppm_path, frame_data);
        println!("Saved {}", ppm_path);
    }

    // Write audio as WAV
    let wav_path = "output.wav";
    save_wav(wav_path, &all_samples, 44100);
    println!("Saved {} ({} samples, {:.2}s)", wav_path, all_samples.len(),
             all_samples.len() as f64 / 44100.0);

    println!("Done! {} frames rendered.", num_frames);
}

fn save_ppm(path: &str, data: &[u8; 240 * 160 * 3]) {
    let header = format!("P6\n240 160\n255\n");
    let mut output = header.into_bytes();
    output.extend_from_slice(data);
    fs::write(path, &output).expect("Failed to write PPM file");
}

fn save_wav(path: &str, samples: &[f32], sample_rate: u32) {
    let num_samples = samples.len() as u32;
    let num_channels: u16 = 1;
    let bits_per_sample: u16 = 16;
    let byte_rate = sample_rate * num_channels as u32 * bits_per_sample as u32 / 8;
    let block_align = num_channels * bits_per_sample / 8;
    let data_size = num_samples * bits_per_sample as u32 / 8;

    let mut file = fs::File::create(path).expect("Failed to create WAV file");

    // RIFF header
    file.write_all(b"RIFF").unwrap();
    file.write_all(&(36 + data_size).to_le_bytes()).unwrap();
    file.write_all(b"WAVE").unwrap();

    // fmt chunk
    file.write_all(b"fmt ").unwrap();
    file.write_all(&16u32.to_le_bytes()).unwrap(); // chunk size
    file.write_all(&1u16.to_le_bytes()).unwrap();   // PCM format
    file.write_all(&num_channels.to_le_bytes()).unwrap();
    file.write_all(&sample_rate.to_le_bytes()).unwrap();
    file.write_all(&byte_rate.to_le_bytes()).unwrap();
    file.write_all(&block_align.to_le_bytes()).unwrap();
    file.write_all(&bits_per_sample.to_le_bytes()).unwrap();

    // data chunk
    file.write_all(b"data").unwrap();
    file.write_all(&data_size.to_le_bytes()).unwrap();

    for &sample in samples {
        let clamped = sample.clamp(-1.0, 1.0);
        let pcm = (clamped * 32767.0) as i16;
        file.write_all(&pcm.to_le_bytes()).unwrap();
    }
}
