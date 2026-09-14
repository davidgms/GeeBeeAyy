### 2026-08-28 - `cart/`'s save memory is dead code: the bus never reaches it
- **Context**: consultation on persisting battery saves and save states.
- **Finding**: `Cartridge::save_read`/`save_write`/`save_data`/`load_save`
  (`core/src/cart/mod.rs:132,151,218,230`) have **zero callers** outside the
  file. `MemoryBus` has no `Cartridge` field at all (`core/src/memory/mod.rs:1-19`)
  and keeps its own second copy of the ROM (`bus.rom`, filled by
  `Gba::load_rom` at `core/src/lib.rs:51-52` alongside `Cartridge::from_bytes`).
  `read8` has no arm for `0x0E00_0000..=0x0EFF_FFFF`, so it falls to `_ => 0`
  (`memory/mod.rs:165`); `store8` likewise falls to `_ => {}` (`:269`). Every
  in-game save write is silently discarded and every read returns 0. The
  Flash state machine has never executed a single transition.
- **Application**: "SRAM/Flash/EEPROM emulated" in ROADMAP.md:25 is false. Any
  battery-save FFI work must first wire the cart into the bus and land a
  `core/tests/` case that stores to 0x0E000000 and reads it back - that test
  fails today. Do not build the FFI on top of the existing accessors and
  assume they work.
