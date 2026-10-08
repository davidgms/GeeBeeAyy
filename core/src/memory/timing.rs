//! What each CPU bus access costs, by memory region.
//!
//! The CPU looks up the cost of every access it makes - opcode fetches,
//! data loads and stores - with `MemoryBus::access_cycles`, adds its
//! internal cycles, and `Cpu::step` returns the sum. The plain
//! `read*`/`write*` methods cost nothing, so the HLE BIOS and the DMA unit,
//! which also use them, stay free.

/// One entry per value of address bits 24-31, so the lookup needs no clamp
/// or bounds check: 0x00-0x0F are the GBA's regions, the rest unmapped.
/// Inline in the bus rather than boxed: the extra pointer load measured
/// ~3% per frame on Mario Tennis.
const REGIONS: usize = 256;

#[inline]
fn region(address: u32) -> usize {
    (address >> 24) as usize
}

/// Cycles per access for each region, indexed `[region][word * 2 + seq]`:
/// non-sequential and sequential, 16-bit (8-bit accesses cost the same) and
/// 32-bit.
#[derive(Clone)]
pub(crate) struct WaitTable {
    costs: [[u8; 4]; REGIONS],
}

impl WaitTable {
    /// The access times WAITCNT selects, with the fixed ones of the other
    /// regions (GBATEK, GBA Memory Map and GBA System Control):
    ///
    /// | Region | Bus | 8/16-bit | 32-bit |
    /// |---|---|---|---|
    /// | BIOS, IWRAM, I/O, OAM | 32 | 1 | 1 |
    /// | EWRAM | 16 | 3 | 6 |
    /// | Palette, VRAM | 16 | 1 | 2 |
    /// | GamePak ROM WS0/1/2 | 16 | N or S | N + S, or 2 S |
    /// | GamePak SRAM | 8 | 1 + wait | 1 + wait |
    ///
    /// Each ROM access is 1 + the wait WAITCNT gives its wait-state region:
    /// first (N) access 4, 3, 2 or 8 waits, second (S) access 2 or 1 for WS0,
    /// 4 or 1 for WS1, 8 or 1 for WS2. A 32-bit ROM access is two 16-bit
    /// ones, the second always sequential. The extra cycle VRAM, palette and
    /// OAM cost while the PPU uses them is not modelled.
    pub(crate) fn from_waitcnt(waitcnt: u16) -> Self {
        const FIRST: [u8; 4] = [4, 3, 2, 8];
        let field = |shift: u16| usize::from((waitcnt >> shift) & 3);
        let bit = |shift: u16| waitcnt >> shift & 1 != 0;
        let mut costs = [[1u8; 4]; REGIONS];
        costs[0x02] = [3, 3, 6, 6];
        costs[0x05] = [1, 1, 2, 2];
        costs[0x06] = [1, 1, 2, 2];
        let rom = |n: u8, s: u8| [1 + n, 1 + s, 2 + n + s, 2 + 2 * s];
        let ws0 = rom(FIRST[field(2)], if bit(4) { 1 } else { 2 });
        let ws1 = rom(FIRST[field(5)], if bit(7) { 1 } else { 4 });
        let ws2 = rom(FIRST[field(8)], if bit(10) { 1 } else { 8 });
        let sram = 1 + FIRST[field(0)];
        for (region, cost) in [
            (0x08, ws0),
            (0x09, ws0),
            (0x0A, ws1),
            (0x0B, ws1),
            (0x0C, ws2),
            (0x0D, ws2),
            (0x0E, [sram; 4]),
            (0x0F, [sram; 4]),
        ] {
            costs[region] = cost;
        }
        Self { costs }
    }

    #[inline]
    pub(crate) fn cost(&self, address: u32, word: bool, seq: bool) -> u32 {
        u32::from(self.costs[region(address)][usize::from(word) << 1 | usize::from(seq)])
    }
}
