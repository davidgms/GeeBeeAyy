//! Serial port (SIO) registers and the no-peer normal-mode transfer.
//!
//! The register table is the mGBA suite's `sio-read.c` (MIT,
//! github.com/mgba-emu/suite), whose expected values were recorded on
//! hardware with no link cable attached.

use geebeeayy_core::memory::MemoryBus;

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
