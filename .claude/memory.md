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

### 2026-08-27 - An agent whose `tools:` list omits Edit cannot satisfy the Memory Protocol

`search-specialist` and `accessibility-tester` shipped with read-only tool
lists (`Read, Grep, Glob, ...`), so the `SubagentStop` hook nudged them on
every run for a write they were structurally incapable of making. Both now
carry `Edit`. When registering an agent that has a `## Discoveries` section,
check that its frontmatter grants `Edit` or `Write` - the hook and the tool
list have to agree or the agent loops.
