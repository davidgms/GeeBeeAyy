### 2026-08-28 - Save-state restore is destructive on failure and drops derived state
- **Context**: same consultation; auditing `core/src/savestate.rs` v2.
- **Finding**: three separate defects, none covered by
  `save_state_round_trip_preserves_registers` (`core/tests/integration.rs:50`),
  which only checks `registers[0]` on a state made by the same build.
  1. `restore` writes straight into `gba` as it parses (`savestate.rs:126-207`),
     so a truncated file returns `Err(Io)` with the machine already half
     overwritten. `geebeeayy_load_state` reports -1 (`ffi.rs:238-241`) and the
     frontend keeps running a corrupted emulator.
  2. Timers: `create` writes `counter(i)` (`:78`), `restore` feeds it to
     `set_reload(i, ..)` (`:180`). Counter/reload swapped, and `controls`,
     `enabled`, `prescaler`, `cascaded`, `irq_enabled` are never serialised -
     every timer comes back disabled.
  3. DMA: `restore` assigns `control` directly (`:189`) instead of going
     through `Dma::write_control` (`dma.rs:84`), so `timing`, `word_count`,
     `transfer_type` and the fixed/reload flags keep stale values. Sound DMA
     (`timing == 3`) is dead after a restore.
  Also absent from the format entirely: the whole `io_regs[0x400]` array, all
  APU state, and the cart's save memory.
- **Application**: parse a state into a staging struct and only commit on
  success, or snapshot-and-rollback. And when adding a field to a save state,
  restore it through the same setter the bus uses, never by assigning the raw
  register - the derived fields are the ones that break silently.
