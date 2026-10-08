//! Serial port (SIO) registers and the no-peer normal-mode transfer.
//!
//! The register table is the mGBA suite's `sio-read.c` (MIT,
//! github.com/mgba-emu/suite), whose expected values were recorded on
//! hardware with no link cable attached.

use geebeeayy_core::memory::MemoryBus;
use geebeeayy_core::Gba;

const SIOCNT: u32 = 0x0400_0128;
const RCNT: u32 = 0x0400_0134;

/// (name, address, SIOCNT, RCNT, expected read after writing all ones).
#[rustfmt::skip]
const SIO_READ: &[(&str, u32, u16, u16, u16)] = &[
    ("M: SIOMULTI0", 0x120, 0x2000, 0, 0), ("M: SIOMULTI1", 0x122, 0x2000, 0, 0),
    ("M: SIOMULTI2", 0x124, 0x2000, 0, 0), ("M: SIOMULTI3", 0x126, 0x2000, 0, 0),
    ("M: SIOCNT", 0x128, 0x2000, 0, 0x6F8F), ("M: SIOMLT_SEND", 0x12A, 0x2000, 0, 0xFFFF),
    ("M: RCNT", 0x134, 0x2000, 0, 0x01FF), ("M: INVALID (136)", 0x136, 0x2000, 0, 0),
    ("M: JOYCNT", 0x140, 0x2000, 0, 0x0040), ("M: INVALID (142)", 0x142, 0x2000, 0, 0),
    ("M: JOY_RECV_L", 0x150, 0x2000, 0, 0), ("M: JOY_RECV_H", 0x152, 0x2000, 0, 0),
    ("M: JOY_TRANS_L", 0x154, 0x2000, 0, 0), ("M: JOY_TRANS_H", 0x156, 0x2000, 0, 0),
    ("M: JOYSTAT", 0x15A, 0x2000, 0, 0),

    ("N8: SIODATA32_L", 0x120, 0, 0, 0), ("N8: SIODATA32_H", 0x122, 0, 0, 0),
    ("N8: SIOMULTI2", 0x124, 0, 0, 0), ("N8: SIOMULTI3", 0x126, 0, 0, 0),
    ("N8: SIOCNT", 0x128, 0, 0, 0x4F8F), ("N8: SIODATA8", 0x12A, 0, 0, 0xFFFF),
    ("N8: RCNT", 0x134, 0, 0, 0x01F5), ("N8: INVALID (136)", 0x136, 0, 0, 0),
    ("N8: JOYCNT", 0x140, 0, 0, 0x0040), ("N8: INVALID (142)", 0x142, 0, 0, 0),
    ("N8: JOY_RECV_L", 0x150, 0, 0, 0), ("N8: JOY_RECV_H", 0x152, 0, 0, 0),
    ("N8: JOY_TRANS_L", 0x154, 0, 0, 0), ("N8: JOY_TRANS_H", 0x156, 0, 0, 0),
    ("N8: JOYSTAT", 0x15A, 0, 0, 0),

    ("N32: SIODATA32_L", 0x120, 0x1000, 0, 0xFFFF), ("N32: SIODATA32_H", 0x122, 0x1000, 0, 0xFFFF),
    ("N32: SIOMULTI2", 0x124, 0x1000, 0, 0), ("N32: SIOMULTI3", 0x126, 0x1000, 0, 0),
    ("N32: SIOCNT", 0x128, 0x1000, 0, 0x5F8F), ("N32: SIODATA8", 0x12A, 0x1000, 0, 0xFFFF),
    ("N32: RCNT", 0x134, 0x1000, 0, 0x01F5), ("N32: INVALID (136)", 0x136, 0x1000, 0, 0),
    ("N32: JOYCNT", 0x140, 0x1000, 0, 0x0040), ("N32: INVALID (142)", 0x142, 0x1000, 0, 0),
    ("N32: JOY_RECV_L", 0x150, 0x1000, 0, 0), ("N32: JOY_RECV_H", 0x152, 0x1000, 0, 0),
    ("N32: JOY_TRANS_L", 0x154, 0x1000, 0, 0), ("N32: JOY_TRANS_H", 0x156, 0x1000, 0, 0),
    ("N32: JOYSTAT", 0x15A, 0x1000, 0, 0),

    ("U: SIODATA32_L", 0x120, 0x3000, 0, 0), ("U: SIODATA32_H", 0x122, 0x3000, 0, 0),
    ("U: SIOMULTI2", 0x124, 0x3000, 0, 0), ("U: SIOMULTI3", 0x126, 0x3000, 0, 0),
    ("U: SIOCNT", 0x128, 0x3000, 0, 0x7FAF), ("U: SIODATA8", 0x12A, 0x3000, 0, 0),
    ("U: RCNT", 0x134, 0x3000, 0, 0x01FF), ("U: INVALID (136)", 0x136, 0x3000, 0, 0),
    ("U: JOYCNT", 0x140, 0x3000, 0, 0x0040), ("U: INVALID (142)", 0x142, 0x3000, 0, 0),
    ("U: JOY_RECV_L", 0x150, 0x3000, 0, 0), ("U: JOY_RECV_H", 0x152, 0x3000, 0, 0),
    ("U: JOY_TRANS_L", 0x154, 0x3000, 0, 0), ("U: JOY_TRANS_H", 0x156, 0x3000, 0, 0),
    ("U: JOYSTAT", 0x15A, 0x3000, 0, 0),

    ("G: SIODATA32_L", 0x120, 0, 0x8000, 0), ("G: SIODATA32_H", 0x122, 0, 0x8000, 0),
    ("G: SIOMULTI2", 0x124, 0, 0x8000, 0), ("G: SIOMULTI3", 0x126, 0, 0x8000, 0),
    ("G: SIOCNT", 0x128, 0, 0x8000, 0x4F8F), ("G: SIODATA8", 0x12A, 0, 0x8000, 0xFFFF),
    ("G: RCNT", 0x134, 0, 0x8000, 0x81FF), ("G: INVALID (136)", 0x136, 0, 0x8000, 0),
    ("G: JOYCNT", 0x140, 0, 0x8000, 0x0040), ("G: INVALID (142)", 0x142, 0, 0x8000, 0),
    ("G: JOY_RECV_L", 0x150, 0, 0x8000, 0), ("G: JOY_RECV_H", 0x152, 0, 0x8000, 0),
    ("G: JOY_TRANS_L", 0x154, 0, 0x8000, 0), ("G: JOY_TRANS_H", 0x156, 0, 0x8000, 0),
    ("G: JOYSTAT", 0x15A, 0, 0x8000, 0),

    ("J: SIODATA32_L", 0x120, 0, 0xC000, 0), ("J: SIODATA32_H", 0x122, 0, 0xC000, 0),
    ("J: SIOMULTI2", 0x124, 0, 0xC000, 0), ("J: SIOMULTI3", 0x126, 0, 0xC000, 0),
    ("J: SIOCNT", 0x128, 0, 0xC000, 0x4F8F), ("J: SIODATA8", 0x12A, 0, 0xC000, 0xFFFF),
    ("J: RCNT", 0x134, 0, 0xC000, 0xC1FC), ("J: INVALID (136)", 0x136, 0, 0xC000, 0),
    ("J: JOYCNT", 0x140, 0, 0xC000, 0x0040), ("J: INVALID (142)", 0x142, 0, 0xC000, 0),
    ("J: JOY_RECV_L", 0x150, 0, 0xC000, 0), ("J: JOY_RECV_H", 0x152, 0, 0xC000, 0),
    ("J: JOY_TRANS_L", 0x154, 0, 0xC000, 0), ("J: JOY_TRANS_H", 0x156, 0, 0xC000, 0),
    ("J: JOYSTAT", 0x15A, 0, 0xC000, 0),
];

/// The suite's `_runTest`, in the same order and on one machine: select the
/// mode, write all ones (keeping the mode bits of SIOCNT/RCNT), read back,
/// then clean up as the suite does before the next row.
#[test]
fn sio_registers_read_back_like_the_mgba_suite_hardware_table() {
    let mut bus = MemoryBus::new();
    let mut failures = Vec::new();
    for &(name, offset, siocnt, rcnt, expected) in SIO_READ {
        let address = 0x0400_0000 + offset;
        bus.write16(RCNT, rcnt);
        bus.write16(SIOCNT, siocnt);
        let value = match address {
            SIOCNT => (bus.read16(SIOCNT) & 0x3000) | 0xCFFF,
            RCNT => (bus.read16(RCNT) & 0xC000) | 0x3FFF,
            _ => 0xFFFF,
        };
        bus.write16(address, value);
        let got = bus.read16(address);
        if got != expected {
            failures.push(format!("{name}: got {got:04X}, want {expected:04X}"));
        }
        if address != RCNT {
            bus.write16(address, 0);
        }
        bus.write16(SIOCNT, 0);
        bus.write16(RCNT, 0x8000);
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// A machine running `b .` forever, so stepping only advances the clock.
fn spinning_gba() -> Gba {
    let mut rom = vec![0u8; 0x200];
    rom[..4].copy_from_slice(&0xEAFF_FFFEu32.to_le_bytes());
    let mut gba = Gba::new();
    gba.load_rom(&rom).expect("ROM should load");
    gba
}

/// Start a transfer with `siocnt` (start bit added) and step until the start
/// bit clears or `limit` cycles pass; the cycles it took, if it finished.
fn transfer(gba: &mut Gba, siocnt: u16, limit: u64) -> Option<u64> {
    gba.bus.write32(0x0400_0120, 0x1234_5678);
    gba.bus.write16(0x0400_012A, 0x00AA);
    gba.bus.write16(SIOCNT, siocnt);
    gba.bus.write16(SIOCNT, siocnt | 0x80);
    // The start applies at the end of the step that wrote it.
    gba.step();
    let start = gba.cycles;
    while gba.cycles - start < limit {
        gba.step();
        if gba.bus.read16(SIOCNT) & 0x80 == 0 {
            return Some(gba.cycles - start);
        }
    }
    None
}

/// GBATEK, SIO Normal Mode: with the internal clock a transfer shifts 8 or 32
/// bits at 256 KHz (64 cycles a bit) or 2 MHz (8 cycles), then clears the
/// start bit and requests IRQ 7 if SIOCNT bit 14 asks. With no cable SI is
/// high, so the received data is all ones. The start bit used to stay set
/// forever, so a game polling or waiting for the serial IRQ hung.
#[test]
fn a_normal_mode_internal_clock_transfer_finishes_after_its_bits() {
    for (siocnt, cycles, bits32) in [
        (0x4001, 8 * 64, false),
        (0x4003, 8 * 8, false),
        (0x5001, 32 * 64, true),
        (0x5003, 32 * 8, true),
    ] {
        let mut gba = spinning_gba();
        let took = transfer(&mut gba, siocnt, 10_000)
            .unwrap_or_else(|| panic!("SIOCNT {siocnt:04X}: the transfer never finished"));
        // `b .` from ROM is one step of 20 cycles at WAITCNT 0 (S fetch 6,
        // refill N 8 + S 6), so the end is seen up to a step late.
        assert!(
            (cycles..=cycles + 20).contains(&took),
            "SIOCNT {siocnt:04X}: finished after {took} cycles, want {cycles}"
        );
        assert_ne!(gba.bus.read16(0x0400_0202) & 0x80, 0, "no serial IRQ");
        if bits32 {
            assert_eq!(gba.bus.read32(0x0400_0120), 0xFFFF_FFFF);
        } else {
            assert_eq!(gba.bus.read16(0x0400_012A) & 0xFF, 0xFF);
        }
    }
}

/// An external clock, or multiplayer and UART, need a peer: with no cable the
/// transfer never ends (the mGBA suite's multiplayer timing rows time out on
/// hardware), and nothing is raised.
#[test]
fn a_transfer_that_needs_a_peer_never_finishes() {
    for siocnt in [0x4000, 0x5002, 0x6003, 0x7003] {
        let mut gba = spinning_gba();
        assert_eq!(
            transfer(&mut gba, siocnt, 100_000),
            None,
            "SIOCNT {siocnt:04X} finished without a peer"
        );
        assert_eq!(gba.bus.read16(0x0400_0202) & 0x80, 0);
    }
}

/// Clearing the start bit abandons a transfer: no IRQ arrives later.
#[test]
fn clearing_the_start_bit_cancels_the_transfer() {
    let mut gba = spinning_gba();
    gba.bus.write16(SIOCNT, 0x4081);
    gba.step();
    gba.bus.write16(SIOCNT, 0x4001);
    for _ in 0..1000 {
        gba.step();
    }
    assert_eq!(gba.bus.read16(0x0400_0202) & 0x80, 0);
}

/// The BIOS `Halt` (SWI 02h) has a return path after the wake that the HLE
/// does not execute, and the mGBA suite's SIO timing rows measure it: a
/// 2 MHz 8-bit transfer is 64 cycles, and the caller resumes 30 cycles of
/// BIOS epilogue later. IME is off here, so no handler runs in between.
#[test]
fn a_halt_swi_pays_the_bios_return_path_after_the_wake() {
    let mut rom = vec![0u8; 0x200];
    rom[..4].copy_from_slice(&0xEF02_0000u32.to_le_bytes()); // swi 0x020000
    rom[4..8].copy_from_slice(&0xEAFF_FFFEu32.to_le_bytes()); // b .
    let mut gba = Gba::new();
    gba.load_rom(&rom).expect("ROM should load");
    gba.bus.write16(0x0400_0200, 0x0080); // IE: serial
    gba.bus.write16(SIOCNT, 0x4083); // 2 MHz, 8 bits, IRQ, start

    gba.step(); // the SWI; the transfer starts as it ends
    let halted_at = gba.cycles;
    while gba.cpu.halted || gba.bus.io.halt {
        gba.step();
    }
    assert_eq!(gba.cycles - halted_at, 64, "the wake is the transfer's end");
    let first = gba.step();
    let plain = gba.step();
    assert_eq!(first, plain + 30, "the first instruction after Halt");
}
