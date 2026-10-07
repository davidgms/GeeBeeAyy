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
    /// Every access in every region costs one cycle.
    pub(crate) fn flat() -> Self {
        Self {
            costs: [[1; 4]; REGIONS],
        }
    }

    #[inline]
    pub(crate) fn cost(&self, address: u32, word: bool, seq: bool) -> u32 {
        u32::from(self.costs[region(address)][usize::from(word) << 1 | usize::from(seq)])
    }
}
