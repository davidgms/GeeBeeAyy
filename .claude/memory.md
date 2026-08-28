# Project Memory - GeeBeeAyy!

## Purpose

Project context, discoveries and learnings that are durable and shared. Agents
read this at the start of a task and correct it when it goes stale. Knowledge
that belongs to one agent's craft goes in that agent's own `## Discoveries`
section instead; knowledge about the project goes here or in `docs/`.

Outdated or incorrect entries should be removed, not annotated. A wrong line
here costs more than a missing one.

## Project Scope

Cross-platform Game Boy Advance emulator with a pixel bee theme.

- **`core/`** - Rust emulation core (~5.6k lines). ARM7TDMI interpreter, PPU,
  APU, memory bus, DMA, timers, cartridge saves, save states, HLE BIOS.
- **`android/`** - Kotlin + Jetpack Compose. Primary target.
- **`ios/`** - Swift + SwiftUI. Secondary, and not yet buildable.
- Licence MIT. No ROMs or BIOS images in the repository, ever.

## Tech Stack

- **Core**: Rust 2021, dependencies limited to `log` and `thiserror`
  (plus `jni` and `android_logger` under `cfg(target_os = "android")`).
- **Android**: Kotlin, Jetpack Compose, minSdk 24, targetSdk 34, NDK 27,
  ABIs `arm64-v8a` and `armeabi-v7a`.
- **iOS**: Swift, SwiftUI.
- **Toolchain**: Docker image with the full Android SDK/NDK, or local
  `rustup` + `cargo-ndk`. Development happens on WSL2.

## Key Files

- `core/src/lib.rs` - `Gba::run_frame`, the main emulation loop, DMA sound
  routing and interrupt delivery.
- `core/src/cpu/mod.rs` - `Cpu::step`, PC advance, banked registers, IRQ entry.
- `core/src/cpu/arm.rs` / `thumb.rs` - the two instruction decoders.
- `core/src/ffi.rs` - the only boundary between core and frontends: C ABI plus
  JNI exports.
- `core/src/io.rs` - I/O register constants and the interrupt handler state.
- `core/tests/` - the accuracy gate. See CLAUDE.md, Testing.
- `android/.../engine/GbaEngine.kt` - Kotlin side of the JNI bridge.
- `android/.../engine/AudioOutput.kt` - `AudioTrack` sink; the timing master.
- `android/.../viewmodel/EmulationViewModel.kt` - the emulation loop driver.
- `ROADMAP.md` - what is actually done and what is next. Trust it over the
  feature checklists in `README.md`.

## Hardware references

Answer hardware questions from these rather than from memory:

- GBATEK - https://problemkaputt.de/gbatek.htm - the primary reference.
- TONC - https://www.coranac.com/tonc/text/toc.htm - the programmer's view.
- ARM7TDMI TRM - https://developer.arm.com/documentation/ddi0029/
- mGBA, SkyEmu, NanoBoyAdvance - reference implementations worth reading when
  a behaviour is ambiguous.

## Discoveries

### 2026-08-27 - The core's decoders were structurally broken, and nothing caught it

**Context**: first test suite ever written for `core/`. Four fatal bugs
surfaced within minutes, all of which had been in the repository for its whole
history while `ROADMAP.md` marked the decoders complete.

1. **PC advanced by 8 per ARM instruction (4 per THUMB).** `Cpu::step` set
   `R15 = pc + 8` to satisfy the "R15 reads as PC+8" rule and never walked it
   back, so **every second instruction was skipped**. Fixed with a `branched`
   flag set from `set_reg(15, ...)`.
2. **THUMB dispatch matched 3 bits against 4- and 5-bit arms.**
   `match bits15_13` with arms like `0b1001`, `0b1101`, `0b11110`, which can
   never match a 3-bit value. ALU ops, hi-register/BX, PC-relative load,
   register-offset load/store, push/pop, STMIA/LDMIA, conditional branches and
   BL all silently became NOPs. Fixed by re-keying to `bits15_12`.
3. **ARM halfword transfers were absent.** `LDRH`/`STRH`/`LDRSB`/`LDRSH` hit no
   dispatch arm and fell through to NOP.
4. **MRS/MSR were selected on bit 4 instead of bit 21**, so every `MSR` ran as
   `MRS` and nothing could write CPSR.

**Application**: Rust will not warn about an unreachable integer match arm, so
a dispatch table is exactly the kind of code that rots silently. When touching
either decoder, add the encoding to `core/tests/cpu.rs` first. Treat the
completion claims in `ROADMAP.md` and `README.md` as unverified until a test
covers them.

### 2026-08-27 - Ordering trap in the ARM dispatch chain

`halfword_data_transfer` must be tested **before** the PSR transfer check. A
pre-indexed, down-counting `STRH` carries the same bits `[24:23] = 10` and
`bit [20] = 0` pattern that `MSR` matches on, so putting it later silently
routes stores into the PSR path. The dispatch chain in `core/src/cpu/arm.rs`
is order-dependent throughout; adding an arm means checking what already
claims that bit pattern.

### 2026-08-27 - KEYINPUT defaults to "every button held"

`KEYINPUT` (`core/src/io.rs:108`) is a declared constant referenced nowhere,
and `io_regs` is zero-filled. The register is active-low, so a read returns
`0x0000` = all ten buttons pressed, permanently. Any game that polls input
misbehaves from the first frame. Initialising it to `0x03FF` is half the fix;
the other half is an FFI entry point, since no input path from the frontends
to the core exists at all.

### 2026-08-27 - Fetching GBATEK: use the per-section pages, not the monolith

The combined `gbatek.htm` truncates badly through WebFetch's markdown
conversion. Use the split pages instead, pattern
`https://problemkaputt.de/gbatek-<section-slug>.htm` - for example
`gbatek-arm-cpu-register-set.htm`, `gbatek-arm-cpu-exceptions.htm`,
`gbatek-gba-interrupt-control.htm`, `gbatek-gba-timers.htm`,
`gbatek-gba-keypad-input.htm`. Find a slug with a `site:problemkaputt.de`
search rather than guessing anchors on the combined page.

The ARM7TDMI TRM at `developer.arm.com/documentation/ddi0029*` redirects to a
JS-rendered page that cannot be fetched as static text. GBATEK's *ARM CPU
Exceptions* page reproduces the same exception-entry table and is fetchable,
so it is the practical primary source even though the TRM outranks it.

### 2026-08-28 - `libgeebeeayy_core.so` was never actually committed

CLAUDE.md's own architecture section and `ROADMAP.md` item 0.5 both asserted
the jniLibs `.so` files were "committed" and merely stale. `git ls-files
android/app/src/main/jniLibs` returns nothing: `.gitignore`'s `*.so` rule
(under the "Build artifacts" section) matches them, and they were never
force-added. They exist only as local build output on whatever machine last
ran `./build-mobile.sh` or `./build.sh android`. A fresh clone has empty (in
fact absent - git does not track empty directories) `arm64-v8a`/`armeabi-v7a`
folders, so a Gradle build from a clean checkout packages an APK with no
native library at all, and it `UnsatisfiedLinkError`s on the first JNI call -
not "runs the old bugs" as the stale wording implied. `.github/workflows/ci.yml`
(added 2026-08-28) now builds both ABIs with `cargo-ndk` on every push and
uploads them as artifacts; nothing needs to change about `.gitignore`, since
there was never anything to remove from git in the first place.

### 2026-08-28 - ARM-mode `swi` took the function number from the wrong bits

`core/src/cpu/arm.rs` passed the whole 24-bit comment field to `Cpu::swi`, so
`bios::handle_swi` matched on `0x0C0000` and fell through to `_ => false` for
every ARM-mode BIOS call. The real BIOS handler does `ldrb r?,[lr,#-2]`, which
for an ARM opcode is bits 23-16 and for a THUMB opcode is the low byte - the
comment field width differs but the byte the BIOS reads does not. GBATEK, *ARM
CPU Exceptions*, says the same thing from the other direction: "you could use
only the most significant 8bits of the 24bit ARM comment". THUMB
(`thumb.rs:393`) was already correct, which is why nothing noticed: every game
that calls BIOS functions from THUMB worked and every ARM-state call silently
did nothing. Covered now by `arm_swi_takes_the_function_number_from_bits_23_16`
in `core/tests/bios.rs`.

The same pass found `lz77_decompress` and `rl_decompress` reading the header's
decompressed size into `_decompressed_size` and then never using it: both had a
bare `loop` with no exit, so a single `SWI 0x11` or `0x14` overwrote memory
until it hit an unmapped address. The size lives in bits 8-31 of the header,
not bits 0-23 - bits 0-3 are the unit size and 4-7 the type.

### 2026-08-27 - An agent whose `tools:` list omits Edit cannot satisfy the Memory Protocol

`search-specialist` and `accessibility-tester` shipped with read-only tool
lists (`Read, Grep, Glob, ...`), so the `SubagentStop` hook nudged them on
every run for a write they were structurally incapable of making. Both now
carry `Edit`. When registering an agent that has a `## Discoveries` section,
check that its frontmatter grants `Edit` or `Write` - the hook and the tool
list have to agree or the agent loops.

### 2026-08-28 - gba-suite found eleven bugs the hand-written tests could not

**Context**: `core/tests/gba_suite.rs` runs jsmolka's ROMs from `temp/roms/`.
All three (`arm`, `thumb`, `memory`) pass as of 2026-08-28. Getting there took
eleven fixes beyond the four the first hand-written suite caught.

The pattern worth remembering: **every one was a decode or dispatch bug that
hand-written tests missed because the test and the code shared the same wrong
assumption.** The worst example was `arm_ldr_str_word`, which asserted a
store/load round trip and passed while both halves used the wrong address -
an inverted offset-mode flag sent them to the same wrong place. A test that
checks a round trip without checking the address proves nothing.

Categories found: inverted flag bits (`I` in single transfer, `U` in multiply
long), dispatch ranges that excluded valid encodings (`SWPB`, `SMULL`,
register-specified shifts), missing addressing modes (the `P` bit, `IB`/`DA`
block starts), missing special cases (empty register list, base-in-list,
user-bank `^` transfers, `Rd == Rn` writeback), unconditional flag writes on
non-S operations, and a memory map with no mirroring at all.

**Application**: when touching either decoder or the bus, run
`cargo test --release --test gba_suite` before claiming anything works. Assert
on addresses and side effects, not just on returned values. And treat any
`unreachable!()` in a decode path as a latent panic - THUMB Format 2 sat there
for the project's whole history.

### 2026-08-28 - write16 and write32 decompose into byte writes

`MemoryBus::write16`/`write32` are implemented as repeated 8-bit stores. That
made adding the GBA's 8-bit video rules (OAM ignores byte writes; palette and
BG VRAM duplicate the byte across the halfword; OBJ VRAM ignores them)
dangerous, because those rules must apply **only** to genuine byte stores.
The fix was a private `store8` that the wider writes use, with the public
`write8` layering the video rules on top. If you add another size-dependent
rule to the bus, check which of the two paths it belongs on.

### 2026-08-28 - The cartridge save memory is complete and completely unreachable

`Cartridge::save_read`, `save_write`, `save_data` and `load_save`
(`core/src/cart/mod.rs:132,151,218,230`) have **zero callers anywhere in the
repository**. `MemoryBus` has no `Cartridge` field at all and keeps its own
second copy of the ROM, filled by `Gba::load_rom` (`core/src/lib.rs:51`) - so a
32 MB cart is resident twice. There is no bus arm for `0x0E000000`, so every
in-game save write is silently discarded and every read returns 0. The Flash
command state machine at `cart/mod.rs:161-207` has never executed a single
transition.

`ROADMAP.md` claimed "SRAM/Flash/EEPROM save" and "emulated but never written
to disk". Both were wrong in the same direction. That is the fifth time a
completion claim in this repo has turned out to describe code that exists but
is never reached - the same shape as the THUMB dispatch and the ARM SWI
comment field.

**Application**: when a feature is listed as done, grep for a *caller* before
believing it. `pub fn` existing proves nothing.

### 2026-08-28 - Save state v2 restores a machine that never existed

Found during a consultation, not yet fixed. `core/src/savestate.rs`:

- **Timers come back disabled and mistuned.** `create` writes
  `timer.counter(i)` (`:78`) and `restore` feeds it to `set_reload(i, ...)`
  (`:180`) - counter and reload swapped. `control`, `enabled`, `cascaded`,
  `prescaler` and `irq_enabled` are not serialised at all.
- **DMA derived state is stale.** `restore` assigns `channels[i].control`
  directly (`:189`) instead of going through `Dma::write_control`, which is
  what decodes `timing`, `word_count` and the address modes. Sound DMA is
  `timing == 3`, so it is dead after a restore.
- **Absent entirely**: the whole `io_regs` array, all APU state, and the cart's
  save memory.
- **`restore` writes into the live `Gba` as it parses** (`:126-207`), so a
  truncated file leaves a half-restored machine and returns `Err`. The frontend
  reports failure and keeps running a hybrid of two states, which surfaces as a
  game bug minutes later.

`integration.rs:50` passes because it asserts only on `registers[0]`; it would
pass against a `restore` that did nothing else. A real round-trip test runs N
frames, snapshots, runs M more, restores, runs N more, and compares the frame
buffer.
