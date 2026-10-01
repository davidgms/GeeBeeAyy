---
name: probing-the-core-without-touching-the-repo
description: How to run throwaway probes against core/ during a read-only review - a scratch crate with a path dependency, plus how to locate fields inside a save-state blob
metadata:
  type: feedback
---

On a read-only review ("change no files except your memory"), `temp/` is still
inside the repository. Put probes in the session scratchpad instead: a tiny
binary crate whose `Cargo.toml` has
`geebeeayy-core = { path = "/home/david/projects/GeeBeeAyy/core" }`, built with
`CARGO_TARGET_DIR` pointing into the scratchpad. Everything `pub` on `Gba`
(`cpu`, `bus`, `apu.snapshot()`, `bios::handle_swi`, `bus.eeprom_begin_dma`,
`bus.read16_mut`) is reachable, so most hypotheses can be confirmed in one
`cargo run` with `catch_unwind` around the suspect call.

**Why:** on 2026-10-01 I first redirected `cargo test` output into `temp/` and
had to delete it again; the scratch crate confirmed three findings (forced-blank
panic, FIFO-position panic from a crafted state, prescaler-0 hang) without a
single repo write.

**How to apply:**
- Hangs: run the call on a spawned thread and `recv_timeout` on a channel.
- Save-state fields: `Timer`/`Ppu` internals are `pub(crate)`, so patch the
  bytes. The APU blob is found by searching `state.data` for
  `len.to_le_bytes() ++ gba.apu.snapshot()`; FIFO A's `read_pos/write_pos/count`
  sit at +152/+156/+160 inside it. The timer block starts at byte
  8 + 152 + 24 + 1 + 68 + 115200 (header, CPU banks, PSRs, halted, PPU scalars,
  frame buffer) - recompute if `savestate.rs` changes, SAVE_VERSION was 6.
