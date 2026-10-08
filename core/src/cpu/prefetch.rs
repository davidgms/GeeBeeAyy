//! The GamePak prefetch buffer (WAITCNT bit 14).
//!
//! GBATEK, GBA GamePak Prefetch: with the buffer enabled the GBA reads
//! opcodes from GamePak ROM ahead of the CPU while the CPU is not using the
//! GamePak bus, up to eight halfwords, and an opcode fetch the buffer already
//! holds costs no wait states. Only opcodes fetched from ROM go through it.
//!
//! That much is GBATEK. The rest of this model is fitted to the mGBA suite's
//! timing table (hardware values, `src/timing.c`), and GBATEK does not state
//! any of it:
//!
//! - The buffer fetches one halfword per sequential access time (S16) of the
//!   code's wait-state region, on every cycle the CPU leaves the GamePak bus
//!   alone: internal cycles, data accesses to other regions, and the cycle
//!   in which the CPU reads an opcode out of the buffer.
//! - An opcode fetch the buffer holds costs 1 cycle ("0 waits", GBATEK). An
//!   ARM opcode is one 32-bit read: 1 cycle if its second half is buffered
//!   by the end of that cycle, otherwise plus what that half's fetch has
//!   left. A halfword the buffer is still fetching costs the cycles that
//!   fetch has left.
//! - Anything else is an ordinary access, after which the buffer starts over
//!   behind it, empty. A pipeline refill is two ordinary accesses.
//! - A data access to the GamePak (ROM or SRAM) empties and stops the buffer
//!   until the next opcode fetch misses, and costs one more cycle if the
//!   halfword the buffer is fetching has exactly one cycle left. A DMA on
//!   the GamePak pays the same cycle but leaves the buffer as it was.

/// Whether `address` is on the GamePak bus: ROM in its three wait-state
/// mirrors, and SRAM.
#[inline]
pub(crate) fn gamepak(address: u32) -> bool {
    (0x08..0x10).contains(&(address >> 24))
}

#[derive(Clone, Copy, Default)]
pub(crate) struct Prefetch {
    /// Fetching: the CPU runs from ROM with the buffer enabled and has not
    /// touched GamePak data since the last opcode fetch that missed.
    pub(crate) active: bool,
    /// The halfword being fetched now; the buffered ones end just before it.
    next: u32,
    /// Halfwords buffered, 0 to 8.
    count: u32,
    /// Cycles already spent fetching `next`.
    progress: u32,
    /// Cycles per halfword: the region's sequential 16-bit access time.
    s16: u32,
}

impl Prefetch {
    /// Start fetching at `next`, empty.
    pub(crate) fn restart(&mut self, next: u32, s16: u32) {
        *self = Self {
            active: true,
            next,
            count: 0,
            progress: 0,
            s16,
        };
    }

    /// `cycles` with the GamePak bus free.
    #[inline]
    pub(crate) fn advance(&mut self, cycles: u32) {
        if !self.active || self.count == 8 {
            return;
        }
        let mut progress = self.progress + cycles;
        while progress >= self.s16 {
            progress -= self.s16;
            self.count += 1;
            self.next = self.next.wrapping_add(2);
            if self.count == 8 {
                progress = 0;
                break;
            }
        }
        self.progress = progress;
    }

    /// An opcode fetch of one halfword: its cycles if the buffer has it or
    /// is fetching it, `None` if it misses.
    fn half(&mut self, address: u32) -> Option<u32> {
        if self.count > 0 {
            if address != self.next.wrapping_sub(2 * self.count) {
                return None;
            }
            self.count -= 1;
            self.advance(1);
            return Some(1);
        }
        if address != self.next {
            return None;
        }
        let wait = self.s16.saturating_sub(self.progress).max(1);
        self.progress = 0;
        self.next = self.next.wrapping_add(2);
        Some(wait)
    }

    /// An opcode fetch, one halfword or (ARM) two: its cycles, or `None` if
    /// the buffer does not have it. Only called while `active`.
    #[inline]
    pub(crate) fn fetch(&mut self, address: u32, word: bool) -> Option<u32> {
        if word && self.count > 0 && address == self.next.wrapping_sub(2 * self.count) {
            // A buffered ARM opcode is one 0-wait 32-bit read, as long as its
            // second half is in the buffer by the end of that cycle.
            self.count -= 1;
            self.advance(1);
            if self.count > 0 {
                self.count -= 1;
                return Some(1);
            }
            return Some(1 + self.half(address.wrapping_add(2)).unwrap_or(0));
        }
        let first = self.half(address)?;
        if !word {
            return Some(first);
        }
        // After the first halfword the second is always in flight.
        Some(first + self.half(address.wrapping_add(2)).unwrap_or(1))
    }

    /// What another master taking the GamePak bus waits for the buffer: one
    /// cycle when the halfword it is fetching has exactly one cycle left
    /// (fitted, see the module notes).
    pub(crate) fn handover(&self) -> u32 {
        u32::from(self.active && self.count < 8 && self.progress + 1 == self.s16)
    }

    /// A data access to the GamePak takes the bus: the buffer empties and
    /// stops. Returns the cycles that costs the access, `handover`.
    pub(crate) fn stop(&mut self) -> u32 {
        let wait = self.handover();
        self.active = false;
        self.count = 0;
        wait
    }
}
