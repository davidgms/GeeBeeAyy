//! HLE BIOS SWI regression tests.
//!
//! Every case hand-assembles a real `swi` opcode into IWRAM and steps the CPU
//! over it, so the dispatch path (decoder -> `Cpu::swi` -> `bios::handle_swi`)
//! is exercised rather than the handler alone. No ROM or BIOS image needed.

use geebeeayy_core::cpu::Cpu;
use geebeeayy_core::memory::MemoryBus;

const BASE: u32 = 0x0300_0000;
const DATA: u32 = 0x0300_1000;
const DEST: u32 = 0x0300_2000;

/// ARM `swi n`: the BIOS function number sits in bits 23-16 of the comment
/// field (GBATEK, ARM CPU Exceptions).
fn arm_swi(n: u32) -> u32 {
    0xEF00_0000 | (n << 16)
}

/// Run one ARM `swi n` with r0..r3 preloaded, returning the CPU and the bus.
fn run_swi(n: u32, args: [u32; 4]) -> (Cpu, MemoryBus) {
    let mut bus = MemoryBus::new();
    bus.write32(BASE, arm_swi(n));
    let mut cpu = Cpu::new();
    cpu.registers[15] = BASE;
    cpu.registers[13] = 0x0300_7F00;
    cpu.registers[..4].copy_from_slice(&args);
    cpu.step(&mut bus);
    (cpu, bus)
}

// ---------------------------------------------------------------------------
// SWI dispatch
// ---------------------------------------------------------------------------

#[test]
fn arm_swi_takes_the_function_number_from_bits_23_16() {
    // `swi 0x06` in ARM state is EF060000, not EF000006. Div(10, 3) = 3 rem 1
    // proves the comment field reached the handler.
    let (cpu, _) = run_swi(0x06, [10, 3, 0, 0]);
    assert_eq!(cpu.registers[0], 3, "ARM swi 0x06 did not reach Div");
    assert_eq!(cpu.registers[1], 1);
}

#[test]
fn thumb_swi_takes_the_function_number_from_the_low_byte() {
    let mut bus = MemoryBus::new();
    bus.write16(BASE, 0xDF06); // swi 0x06
    let mut cpu = Cpu::new();
    cpu.registers[15] = BASE;
    cpu.registers[13] = 0x0300_7F00;
    cpu.cpsr |= 0x20; // T bit
    cpu.registers[0] = 10;
    cpu.registers[1] = 3;
    cpu.step(&mut bus);
    assert_eq!(cpu.registers[0], 3);
    assert_eq!(cpu.registers[1], 1);
}

// ---------------------------------------------------------------------------
// SWI 0x09 / 0x0A - ArcTan, ArcTan2
// ---------------------------------------------------------------------------

/// Turn a 16bit BIOS angle into the same units so a tolerance is readable:
/// a full turn is 0x10000, so 1 unit is 360/65536 degrees.
fn angle_of(x: f64, y: f64) -> i32 {
    let mut theta = y.atan2(x);
    if theta < 0.0 {
        theta += std::f64::consts::TAU;
    }
    (theta / std::f64::consts::TAU * 65536.0).round() as i32
}

#[test]
fn arctan_of_zero_is_zero() {
    let (cpu, _) = run_swi(0x09, [0, 0, 0, 0]);
    assert_eq!(cpu.registers[0], 0);
}

#[test]
fn arctan_matches_atan_inside_its_accurate_range() {
    // r0 is 1.14 fixed point, so 0x1000 = 0.25 and 0x2000 = 0.5. GBATEK warns
    // the BIOS series loses accuracy past PI/4 (tan > 1.0), so stay under it.
    for (tan, ratio) in [(0x1000u32, 0.25f64), (0x2000, 0.5)] {
        let (cpu, _) = run_swi(0x09, [tan, 0, 0, 0]);
        let expected = angle_of(1.0, ratio);
        let got = cpu.registers[0] as i32;
        assert!(
            (got - expected).abs() <= 16,
            "ArcTan({tan:#x}) = {got:#x}, expected about {expected:#x}"
        );
    }
}

#[test]
fn arctan_of_a_negative_tangent_is_negative_and_sign_extended() {
    // GBATEK: the result covers -PI/2..PI/2 as C000h..4000h. The BIOS leaves it
    // in r0 as a sign-extended word, so -0.5 gives ~-0x12DB, not 0xED25.
    // The input is the whole of r0 (the suite's hardware table: 0000C000 is
    // +3.0, FFFFC000 is -1.0), so -0.5 has to be sign-extended by the caller.
    let (cpu, _) = run_swi(0x09, [0xFFFF_E000, 0, 0, 0]); // -0.5
    let got = cpu.registers[0] as i32;
    let expected = -angle_of(1.0, 0.5);
    assert!(got < 0, "ArcTan of a negative tangent must be negative");
    assert!(
        (got - expected).abs() <= 16,
        "ArcTan(-0.5) = {got}, expected about {expected}"
    );
}

#[test]
fn arctan2_pins_the_four_axes() {
    // GBATEK: result is 0000h-FFFFh for 0 <= THETA < 2PI.
    for (x, y, expected) in [
        (0x4000u32, 0x0000u32, 0x0000u32),
        (0x0000, 0x4000, 0x4000),
        (0xFFFF_C000, 0x0000, 0x8000),
        (0x0000, 0xFFFF_C000, 0xC000),
    ] {
        let (cpu, _) = run_swi(0x0A, [x, y, 0, 0]);
        assert_eq!(
            cpu.registers[0], expected,
            "ArcTan2(x={x:#x}, y={y:#x}) = {:#x}",
            cpu.registers[0]
        );
    }
}

#[test]
fn arctan2_covers_all_eight_octants() {
    // One point per octant, kept off the diagonals where the BIOS series is
    // documented to be inaccurate.
    let cases: [(f64, f64); 8] = [
        (1.0, 0.5),
        (0.5, 1.0),
        (-0.5, 1.0),
        (-1.0, 0.5),
        (-1.0, -0.5),
        (-0.5, -1.0),
        (0.5, -1.0),
        (1.0, -0.5),
    ];
    for (fx, fy) in cases {
        let x = ((fx * 16384.0) as i32) as u32;
        let y = ((fy * 16384.0) as i32) as u32;
        let (cpu, _) = run_swi(0x0A, [x, y, 0, 0]);
        let got = cpu.registers[0] as i32;
        assert!(
            (0..=0xFFFF).contains(&got),
            "ArcTan2 must stay in 0000h..FFFFh, got {got:#x}"
        );
        let expected = angle_of(fx, fy);
        assert!(
            (got - expected).abs() <= 16,
            "ArcTan2({fx}, {fy}) = {got:#x}, expected about {expected:#x}"
        );
    }
}

// ---------------------------------------------------------------------------
// SWI 0x0B - CpuSet (fill mode was ignored)
// ---------------------------------------------------------------------------

#[test]
fn cpu_set_fill_repeats_the_source_word() {
    let mut bus = MemoryBus::new();
    bus.write32(BASE, arm_swi(0x0B));
    bus.write32(DATA, 0xDEAD_BEEF);
    bus.write32(DATA + 4, 0x1234_5678); // must never be read in fill mode
    let mut cpu = Cpu::new();
    cpu.registers[15] = BASE;
    cpu.registers[0] = DATA;
    cpu.registers[1] = DEST;
    // Bit 24 = fill, bit 26 = 32bit, 4 words.
    cpu.registers[2] = 0x0500_0000 | 4;
    cpu.step(&mut bus);
    for i in 0..4 {
        assert_eq!(bus.read32(DEST + i * 4), 0xDEAD_BEEF, "word {i}");
    }
}

#[test]
fn cpu_set_fill_repeats_the_source_halfword() {
    let mut bus = MemoryBus::new();
    bus.write32(BASE, arm_swi(0x0B));
    bus.write16(DATA, 0xABCD);
    bus.write16(DATA + 2, 0x1111);
    let mut cpu = Cpu::new();
    cpu.registers[15] = BASE;
    cpu.registers[0] = DATA;
    cpu.registers[1] = DEST;
    cpu.registers[2] = 0x0100_0000 | 4; // fill, 16bit, 4 halfwords
    cpu.step(&mut bus);
    for i in 0..4 {
        assert_eq!(bus.read16(DEST + i * 2), 0xABCD, "halfword {i}");
    }
}

// ---------------------------------------------------------------------------
// SWI 0x0C - CpuFastSet
// ---------------------------------------------------------------------------

fn seed_words(bus: &mut MemoryBus, count: u32) {
    for i in 0..count {
        bus.write32(DATA + i * 4, 0x1000_0000 + i);
    }
}

#[test]
fn cpu_fast_set_copies_words() {
    let mut bus = MemoryBus::new();
    bus.write32(BASE, arm_swi(0x0C));
    seed_words(&mut bus, 16);
    let mut cpu = Cpu::new();
    cpu.registers[15] = BASE;
    cpu.registers[0] = DATA;
    cpu.registers[1] = DEST;
    cpu.registers[2] = 16;
    cpu.step(&mut bus);
    for i in 0..16 {
        assert_eq!(bus.read32(DEST + i * 4), 0x1000_0000 + i, "word {i}");
    }
    assert_eq!(
        bus.read32(DEST + 16 * 4),
        0,
        "wrote past the requested length"
    );
}

#[test]
fn cpu_fast_set_rounds_the_word_count_up_to_a_multiple_of_eight() {
    // GBATEK: "Wordcount (GBA: rounded-up to multiple of 8 words)". Asking for
    // 3 words moves 8.
    let mut bus = MemoryBus::new();
    bus.write32(BASE, arm_swi(0x0C));
    seed_words(&mut bus, 16);
    let mut cpu = Cpu::new();
    cpu.registers[15] = BASE;
    cpu.registers[0] = DATA;
    cpu.registers[1] = DEST;
    cpu.registers[2] = 3;
    cpu.step(&mut bus);
    for i in 0..8 {
        assert_eq!(bus.read32(DEST + i * 4), 0x1000_0000 + i, "word {i}");
    }
    assert_eq!(bus.read32(DEST + 8 * 4), 0, "rounded up past 8 words");
}

#[test]
fn cpu_fast_set_fill_repeats_the_source_word() {
    let mut bus = MemoryBus::new();
    bus.write32(BASE, arm_swi(0x0C));
    seed_words(&mut bus, 16);
    let mut cpu = Cpu::new();
    cpu.registers[15] = BASE;
    cpu.registers[0] = DATA;
    cpu.registers[1] = DEST;
    cpu.registers[2] = 0x0100_0000 | 8; // bit 24 = fill by WORD[r0]
    cpu.step(&mut bus);
    for i in 0..8 {
        assert_eq!(bus.read32(DEST + i * 4), 0x1000_0000, "word {i}");
    }
}

// ---------------------------------------------------------------------------
// SWI 0x16 / 0x17 / 0x18 - the diff unfilters
// ---------------------------------------------------------------------------

/// Header: bits 0-3 unit size, bits 4-7 type 8 (DiffFiltered), bits 8-31 the
/// decompressed size in bytes.
fn diff_header(unit_size: u32, bytes: u32) -> u32 {
    (bytes << 8) | 0x80 | unit_size
}

/// 10, 11, 12, 13, 14, 15, 16, 17 filtered as first-value-plus-differences.
const FILTERED8: [u8; 8] = [10, 1, 1, 1, 1, 1, 1, 1];
const UNFILTERED8: [u8; 8] = [10, 11, 12, 13, 14, 15, 16, 17];

#[test]
fn diff8bit_unfilter_wram_sums_the_differences() {
    let mut bus = MemoryBus::new();
    bus.write32(BASE, arm_swi(0x16));
    bus.write32(DATA, diff_header(1, 8));
    for (i, &b) in FILTERED8.iter().enumerate() {
        bus.write8(DATA + 4 + i as u32, b);
    }
    let mut cpu = Cpu::new();
    cpu.registers[15] = BASE;
    cpu.registers[0] = DATA;
    cpu.registers[1] = DEST;
    cpu.step(&mut bus);
    for (i, &b) in UNFILTERED8.iter().enumerate() {
        assert_eq!(bus.read8(DEST + i as u32), b, "byte {i}");
    }
    assert_eq!(bus.read8(DEST + 8), 0, "wrote past the header size");
}

#[test]
fn diff8bit_unfilter_wram_wraps_at_a_byte() {
    // The accumulator is 8bit: 0xF0 + 0x20 must come out as 0x10, not 0x110.
    let mut bus = MemoryBus::new();
    bus.write32(BASE, arm_swi(0x16));
    bus.write32(DATA, diff_header(1, 2));
    bus.write8(DATA + 4, 0xF0);
    bus.write8(DATA + 5, 0x20);
    let mut cpu = Cpu::new();
    cpu.registers[15] = BASE;
    cpu.registers[0] = DATA;
    cpu.registers[1] = DEST;
    cpu.step(&mut bus);
    assert_eq!(bus.read8(DEST), 0xF0);
    assert_eq!(bus.read8(DEST + 1), 0x10);
}

#[test]
fn diff8bit_unfilter_vram_writes_halfword_pairs() {
    let mut bus = MemoryBus::new();
    bus.write32(BASE, arm_swi(0x17));
    bus.write32(DATA, diff_header(1, 8));
    for (i, &b) in FILTERED8.iter().enumerate() {
        bus.write8(DATA + 4 + i as u32, b);
    }
    let mut cpu = Cpu::new();
    cpu.registers[15] = BASE;
    cpu.registers[0] = DATA;
    cpu.registers[1] = 0x0600_0000; // VRAM, which rejects byte stores
    cpu.step(&mut bus);
    for i in 0..4u32 {
        let expected =
            UNFILTERED8[i as usize * 2] as u16 | ((UNFILTERED8[i as usize * 2 + 1] as u16) << 8);
        assert_eq!(bus.read16(0x0600_0000 + i * 2), expected, "halfword {i}");
    }
}

#[test]
fn diff16bit_unfilter_sums_halfword_differences() {
    let mut bus = MemoryBus::new();
    bus.write32(BASE, arm_swi(0x18));
    // Header size stays in bytes: 4 halfwords = 8 bytes.
    bus.write32(DATA, diff_header(2, 8));
    for (i, &h) in [0x1000u16, 0x0001, 0xFFFF, 0x0100].iter().enumerate() {
        bus.write16(DATA + 4 + i as u32 * 2, h);
    }
    let mut cpu = Cpu::new();
    cpu.registers[15] = BASE;
    cpu.registers[0] = DATA;
    cpu.registers[1] = DEST;
    cpu.step(&mut bus);
    // 0x1000, +1 = 0x1001, -1 = 0x1000, +0x100 = 0x1100.
    for (i, &h) in [0x1000u16, 0x1001, 0x1000, 0x1100].iter().enumerate() {
        assert_eq!(bus.read16(DEST + i as u32 * 2), h, "halfword {i}");
    }
    assert_eq!(bus.read16(DEST + 8), 0, "wrote past the header size");
}

// ---------------------------------------------------------------------------
// SWI 0x11 / 0x14 - the decompressors, which used to loop forever
// ---------------------------------------------------------------------------

/// Header: bits 0-3 reserved, bits 4-7 compressed type, bits 8-31 the
/// decompressed size in bytes.
fn comp_header(kind: u32, bytes: u32) -> u32 {
    (bytes << 8) | (kind << 4)
}

#[test]
fn lz77_uncomp_stops_at_the_header_size() {
    // Four literals then a back-reference of 4 bytes at disp 4: "ABCDABCD".
    let mut bus = MemoryBus::new();
    bus.write32(BASE, arm_swi(0x11));
    bus.write32(DATA, comp_header(1, 8));
    bus.write8(DATA + 4, 0b0000_1000); // literal x4, then one block
    for (i, b) in b"ABCD".iter().enumerate() {
        bus.write8(DATA + 5 + i as u32, *b);
    }
    // Block: length-3 = 1 in the high nibble (4 bytes), disp-1 = 3.
    bus.write8(DATA + 9, 0x10);
    bus.write8(DATA + 10, 0x03);
    let mut cpu = Cpu::new();
    cpu.registers[15] = BASE;
    cpu.registers[0] = DATA;
    cpu.registers[1] = DEST;
    cpu.step(&mut bus);
    for (i, b) in b"ABCDABCD".iter().enumerate() {
        assert_eq!(bus.read8(DEST + i as u32), *b, "byte {i}");
    }
    assert_eq!(bus.read8(DEST + 8), 0, "kept writing past the header size");
}

#[test]
fn lz77_uncomp_displacement_carries_into_the_high_nibble() {
    // disp-1 = 0x1FF, so the real displacement is 0x200. `x | y + 1` binds the
    // +1 to the low byte alone and yields 0x100 instead.
    let mut bus = MemoryBus::new();
    bus.write32(BASE, arm_swi(0x11));
    // 0x200 bytes of filler, then a back-reference reaching 0x200 back.
    bus.write32(DATA, comp_header(1, 0x203));
    let mut src = DATA + 4;
    let mut written = 0u32;
    while written < 0x200 {
        bus.write8(src, 0x00); // eight literals
        src += 1;
        for i in 0..8u32 {
            bus.write8(src + i, if written + i == 0 { 0x5A } else { 0x11 });
        }
        src += 8;
        written += 8;
    }
    bus.write8(src, 0b1000_0000); // one block, then padding literals
    bus.write8(src + 1, 0x01); // length-3 = 0 -> 3 bytes, disp MSBs = 1
    bus.write8(src + 2, 0xFF); // disp LSBs -> disp-1 = 0x1FF
    let mut cpu = Cpu::new();
    cpu.registers[15] = BASE;
    cpu.registers[0] = DATA;
    cpu.registers[1] = DEST;
    cpu.step(&mut bus);
    assert_eq!(
        bus.read8(DEST + 0x200),
        0x5A,
        "the back-reference landed at the wrong displacement"
    );
}

#[test]
fn rl_uncomp_expands_runs_and_literals_in_byte_units() {
    // GBATEK: RLUnComp reads and writes bytes; there is no 16bit variant of the
    // data, only of the destination writes.
    let mut bus = MemoryBus::new();
    bus.write32(BASE, arm_swi(0x14));
    bus.write32(DATA, comp_header(3, 7));
    bus.write8(DATA + 4, 0x82); // compressed, 2 + 3 = 5 copies
    bus.write8(DATA + 5, 0x7E);
    bus.write8(DATA + 6, 0x01); // uncompressed, 1 + 1 = 2 bytes
    bus.write8(DATA + 7, 0xAA);
    bus.write8(DATA + 8, 0xBB);
    let mut cpu = Cpu::new();
    cpu.registers[15] = BASE;
    cpu.registers[0] = DATA;
    cpu.registers[1] = DEST;
    cpu.step(&mut bus);
    for i in 0..5 {
        assert_eq!(bus.read8(DEST + i), 0x7E, "run byte {i}");
    }
    assert_eq!(bus.read8(DEST + 5), 0xAA);
    assert_eq!(bus.read8(DEST + 6), 0xBB);
    assert_eq!(bus.read8(DEST + 7), 0, "kept writing past the header size");
}

/// `LZ77UnCompVram` (SWI 0x12) exists because VRAM ignores byte stores - a
/// `STRB` there writes the byte into *both* halves of the halfword. The
/// decompressor wrote its output a byte at a time, so every halfword landed as
/// two copies of its second byte and the whole block came out as noise.
#[test]
fn lz77_decompresses_into_vram_in_halfwords() {
    let mut bus = MemoryBus::new();
    bus.write32(BASE, arm_swi(0x12));

    // Header: type 1, four decompressed bytes. Then one flag byte with every
    // unit uncompressed, followed by the literals.
    bus.write32(DATA, 0x0000_0410);
    bus.write8(DATA + 4, 0x00);
    for (i, b) in [0xAAu8, 0xBB, 0xCC, 0xDD].iter().enumerate() {
        bus.write8(DATA + 5 + i as u32, *b);
    }

    let mut cpu = Cpu::new();
    cpu.registers[15] = BASE;
    cpu.registers[13] = 0x0300_7F00;
    cpu.registers[0] = DATA;
    cpu.registers[1] = 0x0600_0000;
    cpu.step(&mut bus);

    assert_eq!(bus.read16(0x0600_0000), 0xBBAA, "first halfword");
    assert_eq!(bus.read16(0x0600_0002), 0xDDCC, "second halfword");
}

/// The same for the run-length decompressor's VRAM variant (SWI 0x15).
#[test]
fn rl_decompresses_into_vram_in_halfwords() {
    let mut bus = MemoryBus::new();
    bus.write32(BASE, arm_swi(0x15));

    // Header: type 3, four bytes out. One uncompressed run of four literals.
    bus.write32(DATA, 0x0000_0430);
    bus.write8(DATA + 4, 0x03); // uncompressed, N+1 = 4
    for (i, b) in [0x11u8, 0x22, 0x33, 0x44].iter().enumerate() {
        bus.write8(DATA + 5 + i as u32, *b);
    }

    let mut cpu = Cpu::new();
    cpu.registers[15] = BASE;
    cpu.registers[13] = 0x0300_7F00;
    cpu.registers[0] = DATA;
    cpu.registers[1] = 0x0600_0000;
    cpu.step(&mut bus);

    assert_eq!(bus.read16(0x0600_0000), 0x2211, "first halfword");
    assert_eq!(bus.read16(0x0600_0002), 0x4433, "second halfword");
}

/// The decompression header's size field is 24 bits, so a corrupt ROM can ask
/// for 16 MB. Honouring that means a 16 MB allocation and millions of
/// iterations inside a single SWI; no GBA destination is larger than EWRAM's
/// 256 KB, so the size is clamped there.
///
/// This is a **cost** guard, not a correctness one: uncapped the call still
/// returns, just around 70x slower (0.71 s against 0.01 s when this was
/// measured), which inside one BIOS call is a visible freeze. The assertion is
/// only that it completes without panicking - wall-clock assertions are
/// flaky, so the number lives in this comment instead.
#[test]
fn a_corrupt_decompression_header_cannot_ask_for_sixteen_megabytes() {
    let mut bus = MemoryBus::new();
    bus.write32(BASE, arm_swi(0x11));

    // Type 1, and the largest size the field can hold.
    bus.write32(DATA, 0xFFFF_FF10);
    // A flag byte of zero means eight literal bytes, so the stream never ends
    // on its own - only the size cap stops it.
    for i in 0..64u32 {
        bus.write8(DATA + 4 + i, 0x00);
    }

    let mut cpu = Cpu::new();
    cpu.registers[15] = BASE;
    cpu.registers[13] = 0x0300_7F00;
    cpu.registers[0] = DATA;
    cpu.registers[1] = DEST;
    // Completing at all is the assertion: uncapped this allocates 16 MB and
    // spends millions of iterations before returning.
    cpu.step(&mut bus);
}

// ---------------------------------------------------------------------------
// SWI 0x06 / 0x07: Div overflow
// ---------------------------------------------------------------------------

#[test]
fn div_of_int_min_by_minus_one_does_not_panic() {
    // Rust's i32 `/` panics on INT_MIN / -1 in release as well as debug, which
    // aborts the whole process through JNI. The BIOS wraps and returns
    // 0x80000000 with a remainder of 0.
    let (cpu, _) = run_swi(0x06, [0x8000_0000, 0xFFFF_FFFF, 0, 0]);
    assert_eq!(cpu.registers[0], 0x8000_0000);
    assert_eq!(cpu.registers[1], 0);
}

#[test]
fn div_arm_of_int_min_by_minus_one_does_not_panic() {
    // SWI 0x07 takes its operands the other way round: r0 is the denominator.
    let (cpu, _) = run_swi(0x07, [0xFFFF_FFFF, 0x8000_0000, 0, 0]);
    assert_eq!(cpu.registers[0], 0x8000_0000);
    assert_eq!(cpu.registers[1], 0);
}

// ---------------------------------------------------------------------------
// SWI 0x00: SoftReset
// ---------------------------------------------------------------------------

/// Run a THUMB `swi 0x00` from Supervisor mode with every register dirty and
/// `flag` at 0x03007FFA, so the reset has to switch mode, state and bank.
fn soft_reset(flag: u8) -> (Cpu, MemoryBus) {
    let mut bus = MemoryBus::new();
    bus.write16(BASE, 0xDF00); // swi 0x00
    bus.write32(0x0300_7DFC, 0xCAFE_F00D); // just below the cleared block
    bus.write32(0x0300_7E00, 0xDEAD_BEEF);
    bus.write32(0x0300_7FF8, 0xDEAD_BEEF);
    bus.write8(0x0300_7FFA, flag);
    let mut cpu = Cpu::new();
    cpu.set_cpsr(0x13 | 0x20); // Supervisor, THUMB
    for r in 0..15 {
        cpu.registers[r] = 0x1111_1111 * (r as u32 + 1);
    }
    cpu.registers[15] = BASE;
    cpu.irq_registers = [0x1234, 0x5678];
    cpu.spsr_svc = 0x1F;
    cpu.spsr_irq = 0x1F;
    cpu.step(&mut bus);
    (cpu, bus)
}

/// GBATEK, BIOS Reset Functions: SoftReset clears 0x3007E00-0x3007FFF, sets
/// SP_svc/SP_irq/SP_sys to 0x3007FE0/0x3007FA0/0x3007F00, zeroes R0-R12,
/// LR_svc, SPSR_svc, LR_irq and SPSR_irq, enters System mode and jumps to
/// 0x08000000 through `BX R14`. It used to return without doing anything.
#[test]
fn soft_reset_reinitialises_the_machine_and_jumps_to_rom() {
    let (cpu, bus) = soft_reset(0);
    assert_eq!(cpu.registers[15], 0x0800_0000, "SoftReset must jump to ROM");
    assert_eq!(cpu.registers[14], 0x0800_0000, "the jump goes through LR");
    assert_eq!(cpu.cpsr & 0x1F, 0x1F, "SoftReset must leave System mode");
    assert_eq!(cpu.cpsr & 0x20, 0, "SoftReset must leave ARM state");
    assert_eq!(cpu.registers[..13], [0; 13], "R0-R12 must be zero");
    assert_eq!(cpu.registers[13], 0x0300_7F00, "SP_sys");
    assert_eq!(cpu.svc_registers, [0x0300_7FE0, 0], "SP_svc and LR_svc");
    assert_eq!(cpu.irq_registers, [0x0300_7FA0, 0], "SP_irq and LR_irq");
    assert_eq!((cpu.spsr_svc, cpu.spsr_irq), (0, 0));
    for addr in (0x0300_7E00..0x0300_8000).step_by(4) {
        assert_eq!(bus.read32(addr), 0, "{addr:08X} must be cleared");
    }
    assert_eq!(
        bus.read32(0x0300_7DFC),
        0xCAFE_F00D,
        "only 0x200 bytes clear"
    );
}

/// A non-zero byte at 0x3007FFA selects the multiboot entry in EWRAM. It is
/// read before the block holding it is cleared.
#[test]
fn soft_reset_with_the_ram_flag_jumps_to_ewram() {
    let (cpu, bus) = soft_reset(1);
    assert_eq!(cpu.registers[15], 0x0200_0000);
    assert_eq!(
        bus.read8(0x0300_7FFA),
        0,
        "the flag is cleared with the rest"
    );
}

/// The mGBA suite's BIOS-math table (github.com/mgba-emu/suite,
/// `src/bios-math.c`, MIT), recorded on hardware: `(name, swi, r0, r1)` in,
/// `(r0, r1, r3)` out. r2 is the caller's and the suite zeroes r3 first, so
/// r1 and r3 are what the BIOS leaves behind: ArcTan's `a` and `b` series
/// terms, ArcTan2's `a` and a BIOS address (0x170), Div's |quotient|.
#[rustfmt::skip]
const MGBA_SUITE_MATH: &[(&str, u32, u32, u32, u32, u32, u32)] = &[
    ("ArcTan 00000000", 0x9, 0, 0, 0, 0, 0xA2F9),
    ("ArcTan 00000001", 0x9, 1, 0, 0, 0, 0xA2F9),
    ("ArcTan 00003FFF", 0x9, 0x3FFF, 0, 0x1FFF, 0xFFFFC002, 0x8001),
    ("ArcTan 00004000", 0x9, 0x4000, 0, 0x2000, 0xFFFFC000, 0x8000),
    ("ArcTan 00004001", 0x9, 0x4001, 0, 0x1FFF, 0xFFFFBFFE, 0x7FFE),
    ("ArcTan 00007FFF", 0x9, 0x7FFF, 0, 0x16D8, 0xFFFF0004, 0x22DB6),
    ("ArcTan 00008000", 0x9, 0x8000, 0, 0x16A2, 0xFFFF0000, 0x22D45),
    ("ArcTan 00008001", 0x9, 0x8001, 0, 0x1005, 0xFFFEFFFC, 0x22006),
    ("ArcTan 0000BFFF", 0x9, 0xBFFF, 0, 0x1F64, 0x0001C006, 0x17F33),
    ("ArcTan 0000C000", 0x9, 0xC000, 0, 0xFFFFC360, 0x0001C000, 0x10480),
    ("ArcTan 0000C001", 0x9, 0xC001, 0, 0xFFFFD550, 0x0001BFFA, 0x271BD),
    ("ArcTan 0000FFFF", 0x9, 0xFFFF, 0, 0xFFFFA2FE, 8, 0xA2FF),
    ("ArcTan FFFF0000", 0x9, 0xFFFF0000, 0, 0x5D07, 0, 0xA2F9),
    ("ArcTan FFFF0001", 0x9, 0xFFFF0001, 0, 0x5D01, 8, 0xA2FF),
    ("ArcTan FFFF3FFF", 0x9, 0xFFFF3FFF, 0, 0x2AAF, 0x0001BFFA, 0x271BD),
    ("ArcTan FFFF4000", 0x9, 0xFFFF4000, 0, 0x3CA0, 0x0001C000, 0x10480),
    ("ArcTan FFFF4001", 0x9, 0xFFFF4001, 0, 0xFFFFE09B, 0x0001C006, 0x17F33),
    ("ArcTan FFFF7FFF", 0x9, 0xFFFF7FFF, 0, 0xFFFFEFFA, 0xFFFEFFFC, 0x22006),
    ("ArcTan FFFF8000", 0x9, 0xFFFF8000, 0, 0xFFFFE95D, 0xFFFF0000, 0x22D45),
    ("ArcTan FFFF8001", 0x9, 0xFFFF8001, 0, 0xFFFFE927, 0xFFFF0004, 0x22DB6),
    ("ArcTan FFFFBFFF", 0x9, 0xFFFFBFFF, 0, 0xFFFFE000, 0xFFFFBFFE, 0x7FFE),
    ("ArcTan FFFFC000", 0x9, 0xFFFFC000, 0, 0xFFFFE000, 0xFFFFC000, 0x8000),
    ("ArcTan FFFFC001", 0x9, 0xFFFFC001, 0, 0xFFFFE000, 0xFFFFC002, 0x8001),
    ("ArcTan FFFFFFFF", 0x9, 0xFFFFFFFF, 0, 0xFFFFFFFF, 0, 0xA2F9),
    ("ArcTan2 00000000,00000000", 0xA, 0, 0, 0, 0, 0x170),
    ("ArcTan2 00000001,00000000", 0xA, 1, 0, 0, 0, 0x170),
    ("ArcTan2 00003FFF,00000000", 0xA, 0x3FFF, 0, 0, 0, 0x170),
    ("ArcTan2 00004000,00000000", 0xA, 0x4000, 0, 0, 0, 0x170),
    ("ArcTan2 00004001,00000000", 0xA, 0x4001, 0, 0, 0, 0x170),
    ("ArcTan2 00007FFF,00000000", 0xA, 0x7FFF, 0, 0, 0, 0x170),
    ("ArcTan2 00008000,00000000", 0xA, 0x8000, 0, 0, 0, 0x170),
    ("ArcTan2 00008001,00000000", 0xA, 0x8001, 0, 0, 0, 0x170),
    ("ArcTan2 0000BFFF,00000000", 0xA, 0xBFFF, 0, 0, 0, 0x170),
    ("ArcTan2 0000C000,00000000", 0xA, 0xC000, 0, 0, 0, 0x170),
    ("ArcTan2 0000C001,00000000", 0xA, 0xC001, 0, 0, 0, 0x170),
    ("ArcTan2 0000FFFF,00000000", 0xA, 0xFFFF, 0, 0, 0, 0x170),
    ("ArcTan2 FFFF0000,00000000", 0xA, 0xFFFF0000, 0, 0x8000, 0, 0x170),
    ("ArcTan2 FFFF0001,00000000", 0xA, 0xFFFF0001, 0, 0x8000, 0, 0x170),
    ("ArcTan2 FFFF3FFF,00000000", 0xA, 0xFFFF3FFF, 0, 0x8000, 0, 0x170),
    ("ArcTan2 FFFF4000,00000000", 0xA, 0xFFFF4000, 0, 0x8000, 0, 0x170),
    ("ArcTan2 FFFF4001,00000000", 0xA, 0xFFFF4001, 0, 0x8000, 0, 0x170),
    ("ArcTan2 FFFF7FFF,00000000", 0xA, 0xFFFF7FFF, 0, 0x8000, 0, 0x170),
    ("ArcTan2 FFFF8000,00000000", 0xA, 0xFFFF8000, 0, 0x8000, 0, 0x170),
    ("ArcTan2 FFFF8001,00000000", 0xA, 0xFFFF8001, 0, 0x8000, 0, 0x170),
    ("ArcTan2 FFFFBFFF,00000000", 0xA, 0xFFFFBFFF, 0, 0x8000, 0, 0x170),
    ("ArcTan2 FFFFC000,00000000", 0xA, 0xFFFFC000, 0, 0x8000, 0, 0x170),
    ("ArcTan2 FFFFC001,00000000", 0xA, 0xFFFFC001, 0, 0x8000, 0, 0x170),
    ("ArcTan2 FFFFFFFF,00000000", 0xA, 0xFFFFFFFF, 0, 0x8000, 0, 0x170),
    ("ArcTan2 00000000,00000001", 0xA, 0, 1, 0x4000, 1, 0x170),
    ("ArcTan2 00000001,00000001", 0xA, 1, 1, 0x2000, 0xFFFFC000, 0x170),
    ("ArcTan2 00004000,00000001", 0xA, 0x4000, 1, 0, 0, 0x170),
    ("ArcTan2 00008000,00000001", 0xA, 0x8000, 1, 0, 0, 0x170),
    ("ArcTan2 0000C000,00000001", 0xA, 0xC000, 1, 0, 0, 0x170),
    ("ArcTan2 0000FFFF,00000001", 0xA, 0xFFFF, 1, 0, 0, 0x170),
    ("ArcTan2 FFFF0000,00000001", 0xA, 0xFFFF0000, 1, 0x8000, 0, 0x170),
    ("ArcTan2 FFFF4000,00000001", 0xA, 0xFFFF4000, 1, 0x8000, 0, 0x170),
    ("ArcTan2 FFFF8000,00000001", 0xA, 0xFFFF8000, 1, 0x8000, 0, 0x170),
    ("ArcTan2 FFFFC000,00000001", 0xA, 0xFFFFC000, 1, 0x7FFF, 0, 0x170),
    ("ArcTan2 FFFFFFFF,00000001", 0xA, 0xFFFFFFFF, 1, 0x6000, 0xFFFFC000, 0x170),
    ("ArcTan2 00000000,00004000", 0xA, 0, 0x4000, 0x4000, 0x4000, 0x170),
    ("ArcTan2 00000001,00004000", 0xA, 1, 0x4000, 0x4000, 0, 0x170),
    ("ArcTan2 00004000,00004000", 0xA, 0x4000, 0x4000, 0x2000, 0xFFFFC000, 0x170),
    ("ArcTan2 00008000,00004000", 0xA, 0x8000, 0x4000, 0x12E4, 0xFFFFF000, 0x170),
    ("ArcTan2 0000C000,00004000", 0xA, 0xC000, 0x4000, 0xD1B, 0xFFFFF8E4, 0x170),
    ("ArcTan2 0000FFFF,00004000", 0xA, 0xFFFF, 0x4000, 0x9FB, 0xFFFFFC00, 0x170),
    ("ArcTan2 FFFF0000,00004000", 0xA, 0xFFFF0000, 0x4000, 0x7604, 0xFFFFFC00, 0x170),
    ("ArcTan2 FFFF0001,00004000", 0xA, 0xFFFF0001, 0x4000, 0x7604, 0xFFFFFC00, 0x170),
    ("ArcTan2 FFFF4000,00004000", 0xA, 0xFFFF4000, 0x4000, 0x72E4, 0xFFFFF8E4, 0x170),
    ("ArcTan2 FFFF8000,00004000", 0xA, 0xFFFF8000, 0x4000, 0x6D1C, 0xFFFFF000, 0x170),
    ("ArcTan2 FFFFC000,00004000", 0xA, 0xFFFFC000, 0x4000, 0x6000, 0xFFFFC000, 0x170),
    ("ArcTan2 FFFFFFFF,00004000", 0xA, 0xFFFFFFFF, 0x4000, 0x4001, 0, 0x170),
    ("ArcTan2 00000000,00008000", 0xA, 0, 0x8000, 0x4000, 0x8000, 0x170),
    ("ArcTan2 00000001,00008000", 0xA, 1, 0x8000, 0x4000, 0, 0x170),
    ("ArcTan2 00004000,00008000", 0xA, 0x4000, 0x8000, 0x2D1C, 0xFFFFF000, 0x170),
    ("ArcTan2 00008000,00008000", 0xA, 0x8000, 0x8000, 0x2000, 0xFFFFC000, 0x170),
    ("ArcTan2 0000C000,00008000", 0xA, 0xC000, 0x8000, 0x17F4, 0xFFFFE390, 0x170),
    ("ArcTan2 0000FFFF,00008000", 0xA, 0xFFFF, 0x8000, 0x12E4, 0xFFFFF000, 0x170),
    ("ArcTan2 FFFF0000,00008000", 0xA, 0xFFFF0000, 0x8000, 0x6D1C, 0xFFFFF000, 0x170),
    ("ArcTan2 FFFF0001,00008000", 0xA, 0xFFFF0001, 0x8000, 0x6D1C, 0xFFFFF000, 0x170),
    ("ArcTan2 FFFF4000,00008000", 0xA, 0xFFFF4000, 0x8000, 0x680B, 0xFFFFE390, 0x170),
    ("ArcTan2 FFFF8000,00008000", 0xA, 0xFFFF8000, 0x8000, 0x6000, 0xFFFFC000, 0x170),
    ("ArcTan2 FFFFC000,00008000", 0xA, 0xFFFFC000, 0x8000, 0x52E4, 0xFFFFF000, 0x170),
    ("ArcTan2 FFFFFFFF,00008000", 0xA, 0xFFFFFFFF, 0x8000, 0x4000, 0, 0x170),
    ("ArcTan2 00000000,0000C000", 0xA, 0, 0xC000, 0x4000, 0xC000, 0x170),
    ("ArcTan2 00000001,0000C000", 0xA, 1, 0xC000, 0x4000, 0, 0x170),
    ("ArcTan2 00004000,0000C000", 0xA, 0x4000, 0xC000, 0x32E5, 0xFFFFF8E4, 0x170),
    ("ArcTan2 00008000,0000C000", 0xA, 0x8000, 0xC000, 0x280C, 0xFFFFE390, 0x170),
    ("ArcTan2 0000C000,0000C000", 0xA, 0xC000, 0xC000, 0x2000, 0xFFFFC000, 0x170),
    ("ArcTan2 0000FFFF,0000C000", 0xA, 0xFFFF, 0xC000, 0x1A37, 0xFFFFDC00, 0x170),
    ("ArcTan2 FFFF0000,0000C000", 0xA, 0xFFFF0000, 0xC000, 0x65C8, 0xFFFFDC00, 0x170),
    ("ArcTan2 FFFF0001,0000C000", 0xA, 0xFFFF0001, 0xC000, 0x65C8, 0xFFFFDC00, 0x170),
    ("ArcTan2 FFFF4000,0000C000", 0xA, 0xFFFF4000, 0xC000, 0x6000, 0xFFFFC000, 0x170),
    ("ArcTan2 FFFF8000,0000C000", 0xA, 0xFFFF8000, 0xC000, 0x57F5, 0xFFFFE390, 0x170),
    ("ArcTan2 FFFFC000,0000C000", 0xA, 0xFFFFC000, 0xC000, 0x4D1C, 0xFFFFF8E4, 0x170),
    ("ArcTan2 FFFFFFFF,0000C000", 0xA, 0xFFFFFFFF, 0xC000, 0x4000, 0, 0x170),
    ("ArcTan2 00000000,00010000", 0xA, 0, 0x10000, 0x4000, 0x10000, 0x170),
    ("ArcTan2 00000001,00010000", 0xA, 1, 0x10000, 0x4000, 0, 0x170),
    ("ArcTan2 00004000,00010000", 0xA, 0x4000, 0x10000, 0x3605, 0xFFFFFC00, 0x170),
    ("ArcTan2 00008000,00010000", 0xA, 0x8000, 0x10000, 0x2D1C, 0xFFFFF000, 0x170),
    ("ArcTan2 0000C000,00010000", 0xA, 0xC000, 0x10000, 0x25C9, 0xFFFFDC00, 0x170),
    ("ArcTan2 0000FFFF,00010000", 0xA, 0xFFFF, 0x10000, 0x2001, 0xFFFFC002, 0x170),
    ("ArcTan2 FFFF0000,00010000", 0xA, 0xFFFF0000, 0x10000, 0x6000, 0xFFFFC000, 0x170),
    ("ArcTan2 FFFF0001,00010000", 0xA, 0xFFFF0001, 0x10000, 0x6000, 0xFFFFC002, 0x170),
    ("ArcTan2 FFFF4000,00010000", 0xA, 0xFFFF4000, 0x10000, 0x5A38, 0xFFFFDC00, 0x170),
    ("ArcTan2 FFFF8000,00010000", 0xA, 0xFFFF8000, 0x10000, 0x52E4, 0xFFFFF000, 0x170),
    ("ArcTan2 FFFFC000,00010000", 0xA, 0xFFFFC000, 0x10000, 0x49FC, 0xFFFFFC00, 0x170),
    ("ArcTan2 FFFFFFFF,00010000", 0xA, 0xFFFFFFFF, 0x10000, 0x4000, 0, 0x170),
    ("ArcTan2 00000000,FFFF0000", 0xA, 0, 0xFFFF0000, 0xC000, 0xFFFF0000, 0x170),
    ("ArcTan2 00000001,FFFF0000", 0xA, 1, 0xFFFF0000, 0xC000, 0, 0x170),
    ("ArcTan2 00004000,FFFF0000", 0xA, 0x4000, 0xFFFF0000, 0xC9FC, 0xFFFFFC00, 0x170),
    ("ArcTan2 00008000,FFFF0000", 0xA, 0x8000, 0xFFFF0000, 0xD2E4, 0xFFFFF000, 0x170),
    ("ArcTan2 0000C000,FFFF0000", 0xA, 0xC000, 0xFFFF0000, 0xDA38, 0xFFFFDC00, 0x170),
    ("ArcTan2 0000FFFF,FFFF0000", 0xA, 0xFFFF, 0xFFFF0000, 0xE000, 0xFFFFC002, 0x170),
    ("ArcTan2 FFFF0000,FFFF0000", 0xA, 0xFFFF0000, 0xFFFF0000, 0xA000, 0xFFFFC000, 0x170),
    ("ArcTan2 FFFF0001,FFFF0000", 0xA, 0xFFFF0001, 0xFFFF0000, 0xA001, 0xFFFFC002, 0x170),
    ("ArcTan2 FFFF4000,FFFF0000", 0xA, 0xFFFF4000, 0xFFFF0000, 0xA5C9, 0xFFFFDC00, 0x170),
    ("ArcTan2 FFFF8000,FFFF0000", 0xA, 0xFFFF8000, 0xFFFF0000, 0xAD1C, 0xFFFFF000, 0x170),
    ("ArcTan2 FFFFC000,FFFF0000", 0xA, 0xFFFFC000, 0xFFFF0000, 0xB605, 0xFFFFFC00, 0x170),
    ("ArcTan2 FFFFFFFF,FFFF0000", 0xA, 0xFFFFFFFF, 0xFFFF0000, 0xC000, 0, 0x170),
    ("Div 00000000/00000000", 0x6, 0x00000000, 0x00000000, 1, 0, 1),
    ("Div 00000001/00000000", 0x6, 0x00000001, 0x00000000, 1, 1, 1),
    ("Div FFFFFFFF/00000000", 0x6, 0xFFFFFFFF, 0x00000000, 0xFFFFFFFF, 0xFFFFFFFF, 1),
    ("Div 80000000/FFFFFFFF", 0x6, 0x80000000, 0xFFFFFFFF, 0x80000000, 0, 0x80000000),
];

#[test]
fn bios_math_matches_the_mgba_suite_hardware_table() {
    let mut failures = Vec::new();
    for &(name, swi, in0, in1, r0, r1, r3) in MGBA_SUITE_MATH {
        let (cpu, _) = run_swi(swi, [in0, in1, 0, 0]);
        let got = (cpu.registers[0], cpu.registers[1], cpu.registers[3]);
        if got != (r0, r1, r3) {
            failures.push(format!(
                "{name}: got r0={:08X} r1={:08X} r3={:08X}, hardware {r0:08X} {r1:08X} {r3:08X}",
                got.0, got.1, got.2
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} rows differ:
{}",
        failures.len(),
        MGBA_SUITE_MATH.len(),
        failures.join(
            "
"
        )
    );
}
