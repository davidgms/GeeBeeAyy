use geebee_core::Gba;
use std::env;
use std::fs;
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

    println!("Running {} frames...", num_frames);

    for frame in 0..num_frames {
        gba.run_frame();

        // Save frame as PPM image
        let frame_data = gba.frame_buffer();
        let ppm_path = format!("frame_{:04}.ppm", frame);
        save_ppm(&ppm_path, frame_data);
        println!("Saved {}", ppm_path);
    }

    println!("Done! {} frames rendered.", num_frames);
}

fn save_ppm(path: &str, data: &[u8; 240 * 160 * 3]) {
    let header = format!("P6\n240 160\n255\n");
    let mut output = header.into_bytes();
    output.extend_from_slice(data);
    fs::write(path, &output).expect("Failed to write PPM file");
}
