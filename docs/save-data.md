# Save data: battery saves and save states

How the two kinds of save work, why the boundary sits where it does, and what
is still missing. Written 2026-08-28, after wiring the cartridge into the bus.

## The two kinds, and why they are not the same thing

- **Battery save** - what the game itself writes when you save in-game. It is
  the cartridge's SRAM, Flash or EEPROM chip. The game owns the format; we only
  store bytes. Losing one loses the player's actual progress.
- **Save state** - a snapshot of the whole emulator. Ours, not the game's, and
  tied to our internal layout, so it breaks whenever that layout changes.

They fail differently and should be stored differently. A corrupted save state
costs one slot. A corrupted battery save costs the playthrough.

## What was actually wrong

`Cartridge::save_read`, `save_write`, `save_data` and `load_save` existed and
were complete, and **had zero callers**. `MemoryBus` had no `Cartridge` field
and no arm for `0x0E000000`, so every in-game save write was silently
discarded and every read returned 0. The Flash command state machine had never
executed a single transition.

This was not a persistence gap. The save hardware was unreachable.

## The fix, and the design decisions in it

### The cartridge lives in the bus

`MemoryBus` owns the `Cartridge`. It is the only object reachable from
`read8`/`store8`, which see a `&mut MemoryBus` and nothing else, so anything
the memory path must reach has to live there. `Gba` keeps thin accessors
(`save_data`, `load_save`, `take_save_dirty`, `cartridge`).

### The save region has an 8-bit databus

GBATEK, *GBA Cart Backup SRAM/FRAM*: the region is at `0x0E000000` and "the
databus is restricted to 8 bits, it should be accessed by LDRB, LDRSB, and
STRB opcodes only".

That is not a footnote, it changes observable behaviour:

- A **halfword read** returns the single byte replicated. `STRB 1` then `LDRH`
  gives `0x0101`, not `0xFF01`.
- A **halfword write** delivers only the byte selected by the accessed
  address, so `STRH 0xAABB` writes `0xBB` at an even address and `0xAA` at an
  odd one.

Because of that, **access alignment is forced inside `MemoryBus`, not by the
callers**. The CPU used to mask the address before calling `write16`, which
threw away the low bit the save region needs. Any new caller should pass the
raw address and let the bus decide.

### Flash erase needs two unlock sequences

The full chip-erase command is `AA, 55, 80, AA, 55, 10`. The state machine
originally took the command straight after `0x80`, which swallowed the second
`0xAA` and meant the erase never ran - a game that formats its save before
writing would find the old contents still there. Sector erase (`0x30`) has the
same shape.

### The dirty flag is set at the mutation sites

`Cartridge::save_dirty` is set where a byte actually changes - the SRAM store,
the Flash byte-program, and the erase paths - not at the top of `save_write`.
The Flash unlock bytes `0xAA`/`0x55` change no data, and treating them as
dirty would trigger a file write every time a game so much as touches Flash.

**Ordering contract**: check `take_save_dirty()` **then** read the bytes.
Reversed, a write landing between the read and the clear is lost. Today every
call happens on the emulation thread between frames so there is no race, but
the contract has to hold the moment anyone moves the flush elsewhere.

## The FFI surface

Four functions, plus JNI mirrors. Deliberately small.

| Function | Purpose |
|---|---|
| `geebeeayy_save_size` | Bytes in the battery save, 0 if the cart has no save chip |
| `geebeeayy_save_read` | Copy the save out into a caller-owned buffer |
| `geebeeayy_save_write` | Restore a save |
| `geebeeayy_save_take_dirty` | Did save memory change since last asked; clears |

There is deliberately **no `save_type` function**. The size uniquely identifies
the type (512 / 32K / 64K / 8K / 128K are all distinct) and the frontend's real
questions are "how many bytes" and "is there a save at all". Add one later if a
UI genuinely wants to print "Flash 128K".

Save states use the same shape, so the frontend has one pattern rather than
two:

| Function | Purpose |
|---|---|
| `geebeeayy_state_size` | Bytes a state taken now would occupy |
| `geebeeayy_state_read` | Copy a state out; 0 if the buffer is too small |
| `geebeeayy_state_write` | Restore a state; -1 if rejected |

This **replaced** an opaque-handle API (`save_state_create` / `load_state` /
`save_state_destroy`) that handed Kotlin a `Long` it could only give back or
free. States were not exportable to a file at all under that design, which is
why "save states are not wired to a UI" understated the problem.

A `-1` from `state_write` means the emulator is **partially restored**, not
untouched: `SaveState::restore` writes into the live machine as it parses.
Treat it as "reload the ROM", not "carry on". Fixing that is part of v3.

There is deliberately **no path-taking FFI**. Android hands out content URIs
and file descriptors, not paths the core can `fs::write`. Storage policy is the
frontend's job; bytes in, bytes out is the only honest boundary.

The core also does **not** auto-flush inside `run_frame`. It has no clock, no
filesystem and no lifecycle. `take_save_dirty` returning true is information;
deciding whether that means "write now", "write in two seconds" or "write on
`onPause`" belongs to the frontend.

## Verified by

`core/tests/saves.rs`, plus these ROMs from `jsmolka/gba-tests` driven through
`core/tests/gba_suite.rs`:

| ROM | Status |
|---|---|
| `sram.gba` | passes |
| `flash64.gba` | passes |
| `flash128.gba` | passes |
| `none.gba` | passes |
| `bios.gba` | ignored - its first check reads the BIOS ROM, which HLE cannot reproduce |

ROMs are never committed. Fetch them into `temp/roms/`; the tests skip when
they are absent.

## Still missing

- **Android persistence.** The core exposes the bytes and the dirty flag;
  nothing writes them to disk yet.
- **Flash chip ID reads.** Command `0x90` sets a state but no read behaviour,
  so a game probing the manufacturer/device ID gets flash contents instead.
  Some games check this before writing and will refuse to save. The gba-suite
  save ROMs pass without it, so it needs its own test.
- **EEPROM is not EEPROM.** Ours is byte-addressed RAM. Real EEPROM is a serial
  device: the game DMAs a bit stream to `0x0D000000` and DMAs bits back. That
  is a different device, not a simplification, and the region is currently
  claimed by ROM reads. Needs a GBATEK read before anyone writes code.
- **Save state v3.** v2 restores a machine that never existed - timers come
  back disabled with counter and reload swapped, DMA derived state is stale so
  sound DMA dies, and `io_regs`, the APU and the cart save are absent from the
  format entirely. `restore` also writes into the live `Gba` as it parses, so a
  truncated file leaves a hybrid of two states while reporting failure. See
  `.claude/memory.md`.
