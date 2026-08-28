//! jsmolka's gba-suite, the hardware accuracy gate.
//!
//! Each ROM runs its checks and then parks in a `b .` self-branch. R12 holds
//! the number of the first failing test, or 0 if every test passed.
//!
//! The ROMs are **not** committed - no ROM or BIOS image ever is. Fetch them
//! into `temp/roms/` and these tests start running:
//!
//! ```text
//! mkdir -p temp/roms && cd temp/roms
//! for r in arm thumb memory; do
//!   curl -sSLO "https://raw.githubusercontent.com/jsmolka/gba-suite/master/$r/$r.gba"
//! done
//! ```
//!
//! Without them each test prints a skip notice and passes, so the suite stays
//! green for a contributor who has not fetched them.

use geebeeayy_core::Gba;

/// Enough to reach the self-branch on every suite; a real hang trips it.
const MAX_STEPS: u64 = 30_000_000;

fn run_suite(name: &str) {
    let path = format!("{}/../temp/roms/{}.gba", env!("CARGO_MANIFEST_DIR"), name);
    let Ok(rom) = std::fs::read(&path) else {
        eprintln!("skipping gba-suite '{name}': {path} not present");
        return;
    };

    let mut gba = Gba::new();
    gba.load_rom(&rom).expect("gba-suite ROM should load");

    let mut steps = 0u64;
    let finished = loop {
        if steps >= MAX_STEPS {
            break false;
        }
        let pc = gba.cpu.registers[15];
        // Drive the whole machine, not just the CPU: these ROMs poll DISPSTAT
        // for VBlank, so a CPU-only step loops forever.
        gba.step();
        steps += 1;
        // The suite signals "done" by branching to itself.
        if gba.cpu.registers[15] == pc {
            break true;
        }
    };

    assert!(
        finished,
        "gba-suite '{name}' never reached its end loop within {MAX_STEPS} steps \
         (PC = {:08X}); the CPU is stuck or looping",
        gba.cpu.registers[15]
    );

    let failed = gba.cpu.registers[12];
    assert_eq!(
        failed, 0,
        "gba-suite '{name}' failed at test {failed} (R12), after {steps} steps"
    );
}

#[test]
fn gba_suite_arm() {
    run_suite("arm");
}

#[test]
fn gba_suite_thumb() {
    run_suite("thumb");
}

#[test]
fn gba_suite_memory() {
    run_suite("memory");
}
