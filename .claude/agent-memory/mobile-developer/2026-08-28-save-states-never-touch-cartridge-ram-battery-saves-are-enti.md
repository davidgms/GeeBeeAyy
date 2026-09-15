### 2026-08-28 - Save states never touch cartridge RAM, battery saves are entirely unwired

- **Context**: Same consultation, checking what `SaveState` and the FFI
  actually persist today.
- **Finding**: `core/src/savestate.rs` (`SaveState::create`/`restore`) has
  zero references to `cart`/`Cartridge` - `grep -n "cart" core/src/savestate.rs`
  returns nothing. Save state slots snapshot CPU, PPU, memory and DMA/timer
  state but never SRAM/Flash/EEPROM. Separately, `SaveState::save_to_file`/
  `load_from_file` (lines 213-222) already exist and already write/read a
  versioned (`GBAS`, version 2) buffer via plain `std::fs::write`/
  `std::fs::read` - not atomic, no temp-then-rename - but
  `EmulationViewModel.saveState()` (`android/.../viewmodel/EmulationViewModel.kt:157-159`)
  never calls them; it only calls `engine.saveStateCreate()` and drops the
  handle. `Cartridge::save_data()`/`load_save()` exist in `core/src/cart/mod.rs`
  (lines 218-243) but are not exposed through `core/src/ffi.rs` at all - no
  Kotlin call site can reach battery RAM today.
- **Application**: Two design decisions are outstanding and not yet made by
  anyone: (1) whether a save-state slot should snapshot cart RAM too, since
  right now loading an old slot can leave a battery save that doesn't match
  what was in RAM at that state's moment - a visible bug to a player the
  first time it happens; (2) `rust-engineer` needs new FFI exports for
  `save_data()`/`load_save()` before any Kotlin-side flush/load logic can be
  written at all, and ideally a dirty flag driven from `Cartridge::save_write`
  (line 151) so the frontend isn't diffing the whole SRAM/Flash buffer every
  frame to decide whether to flush.
