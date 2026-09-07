//! Rewind: a bounded ring of save states.
//!
//! The core does not decide *when* to snapshot, for the same reason it does not
//! decide when to flush a battery save: it has no clock and no lifecycle. The
//! frontend calls [`Rewind::push`] on whatever cadence it wants and
//! [`Rewind::pop`] when the user rewinds.
//!
//! **Snapshots are not small.** Measured at 512,128 bytes - 500 KiB - for a
//! cart with 32 KB of SRAM, and around 600 KiB with a 128 KB Flash save. It is
//! dominated by EWRAM (256 KB), the frame buffer (115 KB), VRAM (96 KB) and the
//! cartridge save.
//!
//! Budget accordingly. A ring of 20 costs roughly 10 MB, so a frontend wanting
//! ten seconds of rewind should snapshot every half second rather than every
//! frame. [`Rewind::memory_bytes`] reports the live cost, so size against the
//! device rather than guessing.

use crate::savestate::{SaveState, SaveStateError};
use crate::Gba;
use std::collections::VecDeque;

pub struct Rewind {
    states: VecDeque<Vec<u8>>,
    capacity: usize,
}

impl Rewind {
    /// A ring holding at most `capacity` snapshots. Pushing past that discards
    /// the oldest, so the buffer never grows without bound. A capacity of zero
    /// disables rewind, which is the default until a frontend asks for it.
    pub fn new(capacity: usize) -> Self {
        Self {
            states: VecDeque::with_capacity(capacity.min(64)),
            capacity,
        }
    }

    /// Snapshot the machine now.
    pub fn push(&mut self, gba: &Gba) {
        if self.capacity == 0 {
            return;
        }
        while self.states.len() >= self.capacity {
            self.states.pop_front();
        }
        self.states.push_back(gba.save_state().data);
    }

    /// Restore the most recent snapshot and drop it, so repeated calls walk
    /// backwards. Returns `Ok(false)` when the ring is empty.
    ///
    /// A failed restore rolls back - see [`SaveState::restore`] - so a corrupt
    /// entry leaves the machine as it was rather than half-rewound.
    pub fn pop(&mut self, gba: &mut Gba) -> Result<bool, SaveStateError> {
        let Some(data) = self.states.pop_back() else {
            return Ok(false);
        };
        SaveState { data }.restore(gba)?;
        Ok(true)
    }

    /// How many snapshots this ring will hold before discarding the oldest.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Snapshots currently held.
    pub fn len(&self) -> usize {
        self.states.len()
    }

    pub fn is_empty(&self) -> bool {
        self.states.is_empty()
    }

    /// Bytes the ring is holding right now. Worth logging on a device: this is
    /// the number that decides whether a rewind depth is affordable.
    pub fn memory_bytes(&self) -> usize {
        self.states.iter().map(|s| s.len()).sum()
    }

    /// Drop every snapshot, for a ROM change or a save-state load.
    pub fn clear(&mut self) {
        self.states.clear();
    }
}
