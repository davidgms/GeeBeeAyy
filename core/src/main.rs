use geebee_core::Gba;
use std::env;
use std::fs;
use std::io::Write;
use std::path::Path;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: geebee <rom_file> [options]");
        eprintln!();
        eprintln!("Options:");
        eprintln!("  --frames N        Run N frames (default: 10)");
        eprintln!("  --fast N          Run N frames fast-forward");
        eprintln!("  --save-state FILE Save state to file after running");
        eprintln!("  --load-state FILE Load state from file before running");
        eprintln!("  --no-ppm          Don't save PPM frames");
        eprintln!("  --no-wav          Don't save WAV audio");
        std::process::exit(1);
    }

    let rom_path = &args[1];
    let mut num_frames: u32 = 10;
    let mut fast_forward: u32 = 0;
    let mut save_state_path: Option<String> = None;
    let mut load_state_path: Option<String> = None;
    let mut save_ppm = true;
    let mut save_wav = true;

    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "--frames" => {
                i += 1;
                num_frames = args[i].parse().expect("Invalid frame count");
            }
            "--fast" => {
                i += 1;
                fast_forward = args[i].parse().expect("Invalid fast forward count");
            }
            "--save-state" => {
                i += 1;
                save_state_path = Some(args[i].clone());
            }
            "--load-state" => {
                i += 1;
                load_state_path = Some(args[i].clone());
            }
            "--no-ppm" => save_ppm = false,
            "--no-wav" => save_wav = false,
            _ => eprintln!("Unknown option: {}", args[i]),
        }
        i += 1;
    }

    if !Path::new(rom_path).exists() {
        eprintln!("Error: ROM file '{}' not found", rom_path);
        std::process::exit(1);
    }

    let rom_data = fs::read(rom_path).expect("Failed to read ROM file");
    println!("Loaded ROM: {} ({} bytes)", rom_path, rom_data.len());

    let mut gba = Gba::new();
    gba.load_rom(&rom_data).expect("Failed to load ROM");

    // Load state if requested
    if let Some(ref path) = load_state_path {
        println!("Loading state from {}...", path);
        let state = geebee_core::savestate::SaveState::load_from_file(path)
            .expect("Failed to load save state");
        gba.load_state(&state).expect("Failed to restore state");
    }

    let mut all_samples: Vec<f32> = Vec::new();

    // Fast forward
    if fast_forward > 0 {
        println!("Fast-forwarding {} frames...", fast_forward);
        gba.run_frames(fast_forward);
    }

    // Normal run
    println!("Running {} frames...", num_frames);
    for frame in 0..num_frames {
        gba.run_frame();

        // Collect audio
        let samples = gba.apu_samples();
        all_samples.extend_from_slice(samples);
        gba.clear_audio_buffer();

        if save_ppm {
            let frame_data = gba.frame_buffer();
            let ppm_path = format!("frame_{:04}.ppm", frame);
            save_ppm_file(&ppm_path, frame_data);
            println!("Saved {}", ppm_path);
        }
    }

    // Save state if requested
    if let Some(ref path) = save_state_path {
        println!("Saving state to {}...", path);
        let state = gba.save_state();
        state.save_to_file(path).expect("Failed to save state");
    }

    // Save audio
    if save_wav && !all_samples.is_empty() {
        let wav_path = "output.wav";
        save_wav_file(wav_path, &all_samples, 44100);
        println!("Saved {} ({} samples, {:.2}s)", wav_path, all_samples.len(),
                 all_samples.len() as f64 / 44100.0);
    }

    println!("Done! {} frames rendered.", num_frames);
}

fn save_ppm_file(path: &str, data: &[u8; 240 * 160 * 3]) {
    let header = format!("P6\n240 160\n255\n");
    let mut output = header.into_bytes();
    output.extend_from_slice(data);
    fs::write(path, &output).expect("Failed to write PPM file");
}

fn save_wav_file(path: &str, samples: &[f32], sample_rate: u32) {
    let num_samples = samples.len() as u32;
    let num_channels: u16 = 1;
    let bits_per_sample: u16 = 16;
    let byte_rate = sample_rate * num_channels as u32 * bits_per_sample as u32 / 8;
    let block_align = num_channels * bits_per_sample / 8;
    let data_size = num_samples * bits_per_sample as u32 / 8;

    let mut file = fs::File::create(path).expect("Failed to create WAV file");

    file.write_all(b"RIFF").unwrap();
    file.write_all(&(36 + data_size).to_le_bytes()).unwrap();
    file.write_all(b"WAVE").unwrap();

    file.write_all(b"fmt ").unwrap();
    file.write_all(&16u32.to_le_bytes()).unwrap();
    file.write_all(&1u16.to_le_bytes()).unwrap();
    file.write_all(&num_channels.to_le_bytes()).unwrap();
    file.write_all(&sample_rate.to_le_bytes()).unwrap();
    file.write_all(&byte_rate.to_le_bytes()).unwrap();
    file.write_all(&block_align.to_le_bytes()).unwrap();
    file.write_all(&bits_per_sample.to_le_bytes()).unwrap();

    file.write_all(b"data").unwrap();
    file.write_all(&data_size.to_le_bytes()).unwrap();

    for &sample in samples {
        let clamped = sample.clamp(-1.0, 1.0);
        let pcm = (clamped * 32767.0) as i16;
        file.write_all(&pcm.to_le_bytes()).unwrap();
    }
}
