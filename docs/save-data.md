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

## Save state format v3

v2 restored a machine that never existed. v3 fixes it:

| Was | Now |
|---|---|
| Timers wrote the counter and restored it **as the reload**; control registers absent entirely, so every timer came back **disabled** - killing DMA sound and every cascade | Counter, reload, control, prescaler, tick counter and the enable/cascade/IRQ flags all round-trip |
| DMA assigned the raw control word, leaving `timing`, `word_count` and the address modes stale. Sound DMA is `timing == 3`, so it was **dead after every restore** | Rebuilt through `Dma::restore_control`, which decodes without starting a transfer |
| The entire `io_regs` file was absent - DISPCNT, scroll registers, everything | Serialised in full |
| The cartridge battery save was absent, so a state did not roll the `.sav` back with it | Serialised alongside |
| `restore` wrote into the live machine as it parsed, so a bad file left a **hybrid of two machines** while returning `Err` | Snapshots first and rolls back on failure, so a rejected load is a no-op |

Splitting `Dma::decode_control` out of `write_control` also fixed a live bug:
`word_count` was computed from `transfer_type` **before** that field was
assigned from the new control word, so it used the previous transfer's width.

The version check rejects older states outright rather than misreading them.

## Still missing

- ~~**Android persistence.**~~ Done: `EmulationViewModel` in `android/` reads
  `<romfile>.sav` next to the ROM before the first frame runs, and debounces
  the flush off the core's dirty flag (2s after the last dirty read, plus an
  unconditional flush on pause/stop/`onCleared`). Falls back to app-private
  storage if the ROM-adjacent write fails. Unverified on a device - no game
  has driven this path yet.
- **Flash chip ID reads.** Command `0x90` sets a state but no read behaviour,
  so a game probing the manufacturer/device ID gets flash contents instead.
  Some games check this before writing and will refuse to save. The gba-suite
  save ROMs pass without it, so it needs its own test.
- ~~EEPROM is not EEPROM.~~ **Implemented 2026-08-28.** It is now the real
  serial protocol at `0x0D000000`, driven bit by bit through DMA's halfword
  path: `11` + 6 or 14 address bits + `0` to set a read address, 68 bits back
  of which the first four are discarded, and `10` + address + 64 data bits +
  `0` to write. Two traps worth knowing if you touch it: **both** commands end
  with a stop bit that must be consumed, or it lands in the next command's
  opcode and desynchronises the stream - which is exactly how the first version
  failed. Address width comes from the detected size, and remains guessed at
  ROM load rather than inferred from the game's first access, so a cart whose
  real EEPROM size differs from our guess gets a file other emulators will not
  read.
- **APU state is still absent from the save state.** Everything else v2 was
  missing is now in v3 (see below), but the audio channels are not, so sound
  restarts from silence after a load rather than continuing mid-note.
