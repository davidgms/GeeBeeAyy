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
- **Android**: Kotlin, Jetpack Compose, minSdk 26, targetSdk 34, NDK 27,
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

### 2026-08-28 - Save state v3 fixed the round-trip bugs v2 had

Superseded: the v2 bugs this entry used to describe (swapped timer
counter/reload, stale DMA-derived state, `io_regs`/APU/cart-save missing
entirely, destructive restore on a truncated file) are fixed in
`core/src/savestate.rs` v3 - see `ROADMAP.md` and `docs/save-data.md`. The
JNI/C-ABI surface also changed shape at the same time: the opaque
create/load/destroy handle triplet is gone, replaced by a byte-based
`state_read`/`state_write` (C ABI) and `nativeStateRead`/`nativeStateWrite`
(JNI) pair mirroring the battery-save shape. A `false`/`-1` result from a
write still means the session is unreliable (restore mutates the live
machine as it parses), so callers must stop rather than continue on a
rejected load - that part did not change.

### 2026-08-28 - `cargo check --target aarch64-linux-android` compiles the JNI block

The JNI exports in `core/src/ffi.rs` sit behind `#[cfg(target_os = "android")]`,
so a normal `cargo build`/`cargo test` never type-checks them - broken JNI code
passes the whole local gate and only fails in CI or on a device. The Rust
Android targets are already installed here, and `cargo check --target
aarch64-linux-android` compiles that block **without needing the NDK** (a full
build would need the linker; a check does not).

It caught two real errors on its first use: `JNIEnv` must be taken as `mut env`
for `new_byte_array` and `get_array_elements`. Run it after touching `ffi.rs`.
See `docs/save-data.md` for the save FFI it was catching errors in.

### 2026-08-28 - `android/app/src/main/cpp/jni_bridge.c` is dead code, not a second implementation

Grepping the Android tree for JNI method names to check for callers of a
removed FFI function turns up `android/app/src/main/cpp/jni_bridge.c`, a
hand-written C file that redeclares the `geebeeayy_*` C ABI and re-implements
`Java_com_geebeeayy_app_engine_GbaEngine_*`. It looks like an alternative to
the JNI exports `core/src/ffi.rs` already provides directly (the `jni` crate
implements the same `Java_com_geebeeayy_app_engine_GbaEngine_*` symbols
itself). It is not built: `android/app/build.gradle.kts` has no
`externalNativeBuild`/CMake block, so nothing ever compiles this file into
the APK. It is also stale proof of that - it still calls the removed
`geebeeayy_save_state_create`/`geebeeayy_load_state`/
`geebeeayy_save_state_destroy` and has no `nativeSetKeys`, save, or
save-state-bytes exports at all, meaning nobody has touched it through two
rounds of FFI changes. Do not treat it as something to keep in sync; it is
either dead weight worth deleting or an intentional dual-build path nobody
has documented. `mobile-developer` owns the Gradle/NDK wiring decision.

### 2026-08-28 - minSdk is 26, not 24

`android/app/build.gradle.kts:13` sets `minSdk = 26`. `README.md` said 24, and
that number was copied into `CLAUDE.md`, this file and two agent personas
without anyone checking the build file - so agents were told to guard API 26+
calls that need no guard. All corrected on 2026-08-28.

Same failure shape as the "committed .so" and "cart saves are emulated"
claims: **a number repeated from prose rather than read from the file that
defines it.** For Android facts, `build.gradle.kts` is the source of truth,
not the README.

### 2026-08-28 - Mode 0 never read tile data: a VRAM offset used as an address

`Ppu::get_bg_pixel` returned `char_base = ((cnt >> 2) & 3) * 0x4000` - a VRAM
*offset* - and the caller passed it straight to `bus.read8()`. Every tile-data
read therefore landed around `0x4000` (unmapped) instead of `0x06004000`, so
every pixel decoded to colour 0 and was skipped. The screen-entry read in the
same function *did* add `0x0600_0000`, which is what made it hard to spot.

It was invisible because `render_scanline` pre-filled each line with **white**
rather than the backdrop, so a background that drew nothing looked like a
deliberately bright screen. Two bugs hiding each other.

Also fixed while there: screen-entry bits 10/11 (horizontal and vertical tile
flip) were ignored, and bits 12-15 (the 4bpp palette bank) were computed as a
`_palette_base` that was never used.

**Application**: in `ppu/`, be explicit about whether a value is a VRAM offset
or a bus address - the two differ by `0x06000000` and only one of them faults
visibly. And never clear a scanline to a colour that could be mistaken for
real output; clear to the backdrop or to something obviously wrong.

### 2026-08-28 - The APU register map is off by one byte, so no channel starts

Found while writing a save-state test that needed the APU to make a sound and
could not get one out of it.

`Apu::write_sound1_reg` decodes `SOUND1CNT_H` as if the whole 16-bit register
were in the low byte: `0x62` sets duty (correct), and also envelope volume,
direction and period, which GBATEK puts in the high byte `0x63`. `0x63` is then
decoded as a length counter, which actually lives in `0x62` bits 5-0. `0x64` is
treated as the high byte of `SOUND1CNT_X` and shifted left 8, when it is the
low byte - and **`0x65` has no handler**, which is where the trigger bit sits.
No PSG channel can be started. The other three channels follow the same shape.

`Gba::apu_sound_write` compounds it by routing only a subset of the offsets the
bus captures - roughly half the sound registers, plus all of wave RAM, never
reach the APU.

Two adjacent bugs already fixed: `SOUNDCNT_X` (0x84, the PSG/FIFO master
enable, bit 7) was not routed at all, and `sound_on` was derived from
SOUNDCNT_H bit 15 which GBATEK defines as "DMA Sound B Reset FIFO".

**Fixed on 2026-08-28** by decoding at 16-bit register granularity instead of
per byte, and by folding every captured byte offset onto its containing
register rather than matching a hand-listed subset. `core/tests/saves.rs` now
has the first tests in this project's history that get audio out of the APU.

**Application**: byte-level register decoding is where this class of bug
breeds - a 16-bit layout flattened into the low byte looks plausible and is
wrong in a way no test catches until you ask for output. Decode whole
registers. And note that downstream audio work (the `AudioTrack` path, buffer
sizing, latency) has still never been exercised by real game audio.

### 2026-08-30 - A press shorter than one frame used to be dropped

`EmulationViewModel` samples `keyState` once per frame and hands it to
`engine.setKeys`. A synthetic `adb input tap` presses for **3 ms**, so the
loop never observed it - the log showed `setKey pressed=true` and
`pressed=false` 3 ms apart with no `loop sees` in between. A human touch lasts
50-200 ms so real play was unaffected, but the design silently lost any press
shorter than ~16 ms.

Fixed with a `transientPresses` AtomicInteger that latches every press and is
drained by the loop, so a press reaches the core for at least one frame
regardless of how brief it was.

**Application**: when a consumer polls state on a fixed cadence, decide
explicitly what happens to events shorter than the period. Polling `keyState`
looked obviously correct and was not.

### 2026-08-30 - Debugging on device: trust the log over the screen

Chasing an input bug that did not exist cost several rounds because the
verification was a single sampled pixel of an emulated frame, and the sample
point kept landing in the letterbox. A one-line `log::warn!` inside the JNI
function settled it immediately and unambiguously: `nativeSetKeys keys=0x8 ->
KEYINPUT=0x03F7`, 72 frames running.

Two traps also worth remembering. A hand-written test ROM is itself unverified
code: mine read `[r0, #0x130]` for KEYINPUT, but **halfword loads carry only an
8-bit immediate offset**, so it silently encoded as `[r0, #0x10]` and read a
write-only register. And `android_logger` is capped at `LevelFilter::Warn` in
this project, so `log::info!`/`debug!` from Rust never appear in logcat.

### 2026-08-30 - Audio reaches the device, but not the low-latency path

A tone ROM (`temp/roms/tone.gba`, rebuilt by `core/tests/tonerom.rs`'s comment
block) drives PSG channel 1, and `dumpsys audio` reports our track as
`state:started` on a real device. That closes the last completely unverified
subsystem.

But logcat carries `createTrack_l(0): AUDIO_OUTPUT_FLAG_FAST denied by server`.
The track is created correctly - `sampleRate 17403, format 0x5 (PCM_FLOAT),
channelMask 0x1, frameCount 1400` - and the rate is the problem: Android grants
the fast path only when the track's rate matches the device's native output
rate, essentially always 48000 Hz. Ours is `16777216 / 964 = 17403`, so the
track is resampled through the normal mixer and
`PERFORMANCE_MODE_LOW_LATENCY` has no effect.

**Application**: the roadmap's "input latency under 45 ms" target depends on
this, not on the input path. Resampling to 48000 in the core needs fractional
cycle accumulation, since 16777216/48000 is not an integer.

Also worth knowing: the `ACDB-LOADER ... set parameters failed` errors in
logcat are MIUI's own audio calibration, present for any app, not ours.

### 2026-08-30 - DMA was never wired to the bus: the sixth "complete but unreachable" feature

`Dma::write_sad`/`write_dad`/`write_count`/`write_control` (`core/src/dma.rs`)
had **no caller outside `dma.rs` and `core/tests/interrupts.rs`** for the
project's whole history. The bus stored DMA register values into `io_regs` and
`Dma::channels[]` never learned about them, so no game-initiated transfer ever
ran. Yggdra Union hung at `0x08093030` polling DMA3CNT_H bit 15 for a
completion that could not happen.

Fixed on 2026-08-30. `MemoryBus::store8` records the channel whose DMAxCNT_H
high byte was written into a private `dma_writes` queue, and `Gba::step` drains
it through `Gba::apply_dma_writes`, which reads SAD/DAD/CNT_L back out of
`io_regs` and calls the existing setters. The bus cannot apply the write itself:
`write_control` runs an immediate transfer inline and needs `&mut MemoryBus`,
the borrow the store already holds. Same shape as the `sound_writes` queue.

Making DMA live exposed three more bugs in code that had simply never executed:
`decode_control` read the source and destination address-control fields
backwards (GBATEK: bits 5-6 are **dest**, 7-8 are **source**); `write_count`
truncated the "0 means maximum length" expansion back to 0 with an `as u16`, so
every maximum-length transfer was a no-op; and `do_transfer` reloaded the
*source* on repeat, which flattens a per-scanline HDMA table to one value.
HBlank DMA is now also gated to visible scanlines in `ppu/mod.rs`, and DMA
sound routes by destination register rather than assuming DMA1 feeds FIFO A.

Covered by `core/tests/dma.rs` (5 cases, all failing before the change).

**Application**: that is now six times. The rule stands and should be applied
before touching anything the roadmap calls done - grep for a *caller*, not for
a `pub fn`. Still unaudited by that standard: video-capture DMA (timing 3 on
DMA3) is a `TODO` in `Dma::on_vcounter` and does nothing.

### 2026-08-30 - Yggdra Union boots: four bugs between the DMA fix and the title screen

Superseded the earlier "derails in its task switcher" note - that derail was a
symptom of the first bug below, not a task-switcher problem.

The four, in the order they were found and each with a test that fails without
the fix:

1. **THUMB Format 18 (`B label`) never sign-extended its 11-bit offset.**
   `core/src/cpu/thumb.rs` widened `instruction & 0x7FF` through `as i16`,
   which is always positive, so **every backward unconditional branch landed
   0x1000 above the branch instead of below it**. Yggdra's `__divsi3`
   normalisation loop fell out of its own function into the C++ throw path,
   which then hit `SWI 0xAB` (ARM semihosting `SYS_EXIT`) forever. Fixing it
   also un-blacked `waimanu`, `jumpingbarnabe` and `powerpig`, which had all
   rendered nothing since the project started. `jsmolka/gba-tests`' `thumb.gba`
   passes with the bug present - the suite does not cover it.

2. **The PPU rebuilt DISPSTAT every tick from cached enable bits** refreshed
   only once per scanline, so a DISPSTAT write made mid-scanline was erased a
   few cycles later. Bit 2 was also hard-wired to the VBlank range instead of
   being the VCounter match against bits 8-15, and the VCounter IRQ was never
   raised. Bits 3-15 are the game's and are now read back and preserved on
   every tick.

3. **A halted step advanced a whole scanline at once**, stepping straight over
   HBlank, so a game halted with the HBlank IRQ enabled - most of every frame -
   got one interrupt per frame instead of 228. `Ppu::cycles_to_next_event`
   now bounds the step.

4. **The synthetic BIOS IRQ handler acknowledged IF before calling the game's
   handler.** The real BIOS handler does nothing but save r0-r3/r12/lr, jump to
   `[0x03007FFC]` and return - it does *not* touch IF and does *not* update the
   IntrWait flags at `0x03007FF8`; both are the game handler's job. Yggdra's
   IWRAM dispatcher branches on `IE & IF`, read zero every time, fell through
   its whole chain and called the wrong handler for every interrupt, so its
   VBlank work - the only place DISPCNT is written - never ran. This was the
   last thing between a black screen and the title screen.

Related: HLE `IntrWait` now polls `[0x03007FF8]` and re-executes its own SWI
until the wanted interrupt arrives, instead of testing IF once. IF is already
cleared by the time the game's handler returns, so the old version let the
first interrupt of any kind satisfy a `VBlankIntrWait`.

**Application**: two of these four are cases where *our* HLE was doing the
game's work for it. When an HLE stub is "helpful", check GBATEK for what the
real BIOS actually does - a game that implements the hardware contract itself
is broken by a stub that implements it too.

### 2026-08-30 - A reference emulator is worth more than any amount of tracing

Three of the four Yggdra bugs above came out of hand-tracing our own execution,
and each took hours. The fourth - the one that actually mattered - took about
ten minutes once mGBA was available to compare against: a write watchpoint on
DISPCNT showed which function writes it, a breakpoint chain showed who calls
that function, and `dis/a 0x03000000` showed the game's own IRQ dispatcher
reading `IE & IF`.

`mgba-sdl` is installable **without root**: `apt-get download` the package and
its dependency closure, `dpkg-deb -x` each into one tree, and run it with
`LD_LIBRARY_PATH` pointing at that tree. `temp/mgba/run.sh` does this and
survives reboots because it lives in the project. Headless:
`SDL_VIDEODRIVER=dummy SDL_AUDIODRIVER=dummy temp/mgba/run.sh -d <rom>`, then
feed the CLI debugger on stdin - `watch/w <addr>`, `b <addr>`, `c`, `i`,
`dis/t`, `dis/a`, `trace N`.

**Application**: reach for it *first* on "game X does not boot", before writing
a single trace probe. Note mGBA boots through its own BIOS HLE from
`0x00000350`, so instruction counts and early PCs do not line up with ours -
compare behaviour at named addresses, not step numbers.


### 2026-08-31 - The timers were never wired either, and that is what made the sound a thump

Device testing of the newly-booting Yggdra Union turned up "the screen is
really bugged and the audio keeps doing a tum tum infinitely". Three separate
faults, all found on the host with `mgba` as the reference:

1. **`Timer::set_control` and `set_reload` had no caller outside the tests** -
   the seventh instance of this repository's standing pattern. The bus stored
   `TMxCNT` into `io_regs` and the timer unit never heard about it, so no timer
   a game started ever ran: no timer interrupt, and no DMA sound, which is
   driven entirely by timer overflows. Fixed with a `timer_writes` queue on the
   bus, the same shape as `dma_writes` and `sound_writes`, latched on the high
   byte of each register. `TMxCNT_L` also now reads back the live counter
   rather than the reload the game wrote there.

2. **`Apu::on_timer_overflow` was an empty stub** and the mixer popped the
   FIFOs itself, once per output sample. Sound therefore played at our output
   rate rather than the rate the game asked for, and drained far faster than
   the DMA refilled it. With the source pointer running off the end of the
   game's mix buffer into unrelated IWRAM, the result was a burst of garbage
   every six frames - the thump. DMA sound is now: timer overflow pops one byte
   into a latch, the mixer holds that latch, the DMA tops the FIFO up at half
   empty. `overflow_flags` became a count rather than a bool, because one tick
   can span several overflows at a short reload.
   Also fixed while there: the DMA sound volumes are SOUNDCNT_**H** bits 2 and
   3, not SOUNDCNT_L bits 12 and 13 (those are PSG channel enables), and each
   FIFO picks its own timer (H bit 10 for A, bit 14 for B).

3. **`Ppu::apply_alpha_blend` was an empty stub**, and the whole colour-effect
   path was gated on `bldcnt & 0x20` - which selects the *backdrop* as a first
   target, not whether an effect runs. Yggdra's title screen alpha-blends a
   band across the artwork; we drew it flat opaque, which is the grey bar the
   user saw. Mode 0 now composites the top two layers per pixel and applies the
   effect there, because blending cannot be done after the fact from a single
   frame buffer. The other modes still use the old scanline-wide
   approximation.

Separately, `Gba::step`'s halted branch skipped the entire post-instruction
block - FIFO refill, timer overflows, queued sound-register writes, DMA
register application. A game is halted for most of every frame, so all of it
has to run there too; it is now a shared `post_tick`.

**Application**: the "grep for a caller, not a `pub fn`" rule has now paid out
seven times. Two of the three faults above were *stubs with a plausible name
and a comment saying what they would do* - `on_timer_overflow` even documented
the hardware behaviour it did not implement. Treat a function whose body is a
comment as a missing feature, not a simplification, and check the callers of
anything the roadmap calls done.

### 2026-08-31 - Android: no wake lock, and integer scaling was losing a whole step to padding

Two frontend faults found on the device, both one-liners.
`android/app/src/main/java/com/geebeeayy/app/ui/screens/EmulationScreen.kt` had
no `keepScreenOn` anywhere, so the display slept mid-play while the emulator
kept running behind it. And the portrait `ScreenContainer` used
`padding(horizontal = 24.dp)`, which leaves 948 px of a 1080 px screen -
integer scaling rounds that to 3x. At 8.dp it clears 960 px, which is exactly
4x. `AudioOutput.write` also returned `true` for a zero-sample write, so a
frame that produced no audio left the emulation loop with nothing to block on
and nothing to sleep on; it now returns `false` and the caller paces itself.

### 2026-08-31 - Sprites had never rendered: the OBJ tile base was BG VRAM

The user's "pile of pink and green pixels" during Yggdra Union's opening was
the sprite renderer. Four faults in `render_obj_scanline`, and the first alone
means **no sprite this emulator ever drew was correct**:

1. **OBJ tile data was read from `0x06000000`.** It lives at `0x06010000`
   (`0x06014000` in the bitmap modes). Every sprite was textured with whatever
   background tiles happened to sit at the same offset - which is exactly what
   noise made of BG art looks like. Yggdra's opening is drawn entirely with
   sprites, so the whole sequence was bands of garbage.
2. **The colour-depth bit was read from attr0 bit 7**, which is part of the Y
   coordinate. GBATEK: bit 13 is the depth, bits 10-11 the OBJ mode. Any
   sprite at Y >= 128 was decoded as 256-colour. There was also a
   `bitmap_mode` flag taken from bit 13 that does not exist.
3. **Tile numbers step in 32-byte units, always.** A 256-colour tile occupies
   two of them, so an 8bpp sprite's tiles advance by two - in 1D *and* 2D
   mapping, and 2D mapping is a fixed 32-unit-wide grid whatever the sprite's
   width. The old code multiplied the tile number by 64 for 8bpp instead.
4. **X and Y were sign-extended.** They are unsigned (9-bit and 8-bit) and
   *wrap*: a sprite near the bottom reappears at the top. Sign-extending put
   every sprite at Y >= 128 off the top of the screen. Horizontal and vertical
   flip (attr1 bits 12-13) were not implemented at all, and sprites drew in
   ascending OAM order so a higher-numbered sprite overwrote a lower-numbered
   one - backwards.

The affine path additionally read PA/PB/PC/PD at `0x07000000 + (0x07000006 +
group*32)`, adding the OAM base twice, so every scaled sprite was transformed
by whatever the **cartridge save chip** returned from `0x0E0000xx`.

**Application**: none of this showed up in `gba-suite`, in the three homebrew
ROMs, or in 40,000 frames of Yggdra's attract loop, because all of those are
background-only. A whole subsystem can be completely broken and still look
fine if nothing under test exercises it - "the screens I checked render
correctly" is not evidence that a feature works. When a report says a specific
sequence is broken, reproduce *that* sequence rather than sampling elsewhere.

Method note: the layer-isolation probe settled this in one step. Render one
frame with each of BG0-3 and OBJ forced off in DISPCNT (re-forcing it every
step, because the game rewrites DISPCNT in VBlank), then diff the frame
buffers: the garbage band changed only when OBJ was disabled, so nothing but
the sprite path could be responsible.

### 2026-08-31 - Homebrew test ROMs, and the LZ77 decompressor found by them

Added two freely distributable ROMs to `temp/roms/`, with the fetch commands in
`core/tests/ppu.rs`'s header:

- **Celeste Classic** (`JeffRuLz/Celeste-Classic-GBA` release) - a sprite-heavy
  platformer. Title screen and gameplay both render, on the host and on the
  device.
- **240p Test Suite** (`pinobatch/240p-test-mini`, the `240pee_mb.gba` build) -
  a purpose-built PPU test suite. Its "Shadow sprite" test draws a large
  multi-tile sprite and is the sharpest sprite check available; its menus are
  LZ77-compressed into VRAM.

The 240p suite immediately found one: **`LZ77UnCompVram` and `RLUnCompVram`
wrote their output with `bus.write8`.** VRAM ignores byte stores - a `STRB`
there writes the byte into *both* halves of the halfword - which is the entire
reason the BIOS has separate Wram and Vram decompressors. Every halfword came
out as two copies of its second byte, and the back-references then read that
corruption back and compounded it. Both now build the block in a `Vec` (so the
lookback is immune to the destination's behaviour) and write it out through a
shared `write_block` that uses halfwords for anything in 0x05000000-0x07FFFFFF.
The suite's front page went from 7 colours to 15 and the sprite test's dithered
edges became flat.

Still failing: **`fantasy-knight.gba`** (`laqieer/gba-free-fonts`) is a black
screen. DISPCNT is 0xF641 - BG mode 1 with an affine BG2, all three windows
enabled and OBJ on. Neither affine backgrounds (`render_mode1_scanline`'s
affine path only handles 8bpp and is marked "not fully handled yet") nor
windows are implemented: `get_window` and `window_layer_visible` are dead code
and the windowing loop in `render_scanline` computes its flags and discards
them. That is the next real PPU gap, and it will hit commercial games.

### 2026-09-01 - Sprite priority, and the three ordering rules worth not guessing

Sprites were composited into the frame buffer *before* the backgrounds, and
OAM attr2 bits 10-11 - the sprite's priority - were never read, so any opaque
background pixel covered a sprite whatever its priority said. The sprite pass
now records `(colour, priority)` per pixel into `Ppu::obj_pixel` and mode 0's
compositor merges that into the layer ordering.

`search-specialist` was asked for the rules before the code was written, and
returned three that are each easy to get backwards:

1. **A sprite wins a tie with a background.** GBATEK, LCD OBJ - OAM
   Attributes: "In case that the Priority relative to BG is the same than the
   priority of one of the background layers, then the OBJ becomes higher
   priority."
2. **Between backgrounds, the lower BG number wins a tie** (BGxCNT bits 0-1) -
   which a stable sort on priority alone already gives.
3. **Between two sprites the OAM index decides on its own.** A sprite's
   priority field is *never* compared against another sprite. GBATEK's
   "Caution" example under OAM Attributes shows this, and VisualBoyAdvance bug
   #130 was closed after a hardware test confirmed it. **TONC's regobj page
   says the opposite** ("for sprites of the same priority, the higher
   OBJ_ATTRs are drawn first") and is the outlier - the agent recorded that in
   its own `## Discoveries`, verified in the diff.

Still not implemented, with the rules now written down in that agent's report:
semi-transparent sprites (OBJ mode 1) are always a 1st target and always alpha
blend regardless of BLDCNT bits 4 and 6-7, but the 2nd-target bits 8-13 still
decide what they blend with and a non-target underneath means no blend at all.
OBJ-window sprites (mode 2) must still be decoded for their mask shape even
though nothing is drawn, and DISPCNT bits 12 *and* 15 both gate that.

**Application**: this is the first time in the project that a hardware
question went to `search-specialist` before the fix rather than after, and it
paid: two of the three rules are coin-flip cases that would have looked
plausible either way, and one has a widely-read source stating it backwards.

### 2026-09-01 - Affine backgrounds never read their tilemap, and Yggdra's gameplay is mode 1

Driving *Yggdra Union* past the title with a scripted key sequence
(`temp/probes/play.rs`: Start, A, tutorial "No", then A every 250 frames) gets
to the first scene, "Thieves' Stronghold", in about 16,000 frames. **DISPCNT
there is 0x1761 - BG mode 1**, so the affine path matters after all; the
attract loop and the whole opening are mode 0, which is why it had never been
exercised.

`render_affine_bg_pixel` was wrong in four ways at once:

1. **It never read the tilemap.** It took the map *index* as the tile number
   and multiplied it by 8, so the background was a linear walk through
   character memory. On screen that is a plausible-looking striped pattern,
   not obvious garbage.
2. **It skipped the layer entirely unless BGxCNT bit 7 was set** - "4bpp affine
   not fully handled yet" in its own comment. Affine backgrounds are *always*
   256-colour; that bit means nothing for them.
3. **It always wrapped.** BGxCNT bit 13 is Display Area Overflow: clear means
   the area outside the map is transparent.
4. **The reference point was reloaded from BGxX/BGxY every scanline**, which
   throws PB and PD away - the accumulation *is* the vertical component. It is
   now loaded at the top of the frame, advanced by PB/PD per visible line, and
   reloaded when the game writes the register mid-frame.

Also fixed alongside: BGxX/BGxY are 28-bit signed (19.8, sign in bit 27) and
were being taken as full 32-bit values, so every negative reference point came
out as a large positive one.

**Application**: the plan was to fix windows and affine backgrounds only "if
gameplay showed they mattered", and one scripted playthrough answered it in
minutes - mode 1, no windows. Reaching real gameplay is worth more than
reasoning about which gap to close next; the opening and the attract loop of a
game can exercise a completely different quarter of the PPU than the game does.

### 2026-09-01 - What Yggdra's gameplay actually exercises, and a snapshot to iterate from

Drove *Yggdra Union* into real gameplay with a scripted key sequence and then
random input, and looked at every screen it reached: the title menu, the
Tutorial Mode prompt, the "Fall of Castle Paltina" story sequence, the
"Thieves' Stronghold" dialogue, CARD SELECT, the CHARACTER sheet, the
victory/defeat conditions screen and the morale/items menu. **All of them
render correctly** after the affine fix. No new rendering defect surfaced.

Two findings worth keeping:

- **Yggdra never enables a window.** Every DISPCNT value seen across the whole
  playthrough - 0x1040, 0x1240, 0x1761, 0x1B60, 0x1C40, 0x1E40, 0x1F40,
  0x1F60 - has bits 13, 14 and 15 clear. So the unimplemented window logic is
  not what is holding this game back, and the only ROM here that needs it is
  `fantasy-knight.gba` (DISPCNT 0xF641). Mode 1 appears exactly once, in the
  dialogue scene, which is what made the affine bug matter.
- **`A` does nothing on CARD SELECT** while Start, Right, Down, L and R all
  work from the same probe, so it is game logic (a card cannot be confirmed in
  that state), not a stuck input. Worth remembering before chasing it as an
  input bug.

`temp/probes/play.rs` grew `SNAP=<frame>:<file>` and `LOAD=<file>`, which turn
a four-minute run-up into a three-second one. `temp/cardselect.state` is a
snapshot sitting on CARD SELECT; note it is a v5 save state and will be
rejected the moment the format changes again.

### 2026-09-01 - Moving to 48 kHz exposed a PSG that had been 64x flat all along

The roadmap's open Phase 1 item was the denied low-latency audio path: the core
emitted at `16777216 / 964` = 17403 Hz, no device supports that, so AudioTrack
resampled every buffer and refused `AUDIO_OUTPUT_FLAG_FAST`. The sample clock
is now an exact fraction - `sample_accum += cycles * 48000`, a sample due each
time it reaches 16777216 - which lands on 48000 Hz with no accumulated
rounding.

Changing the rate is what exposed the real bug. **Every PSG channel advanced
its phase once per emitted sample**, so its pitch was a function of the output
rate. Measured before the change: the tone ROM programs 128 Hz and the APU
produced **2.0 Hz** - 64x flat. Nobody had noticed because no game tested here
uses the PSG for music; Yggdra's music is all DMA sound, and
`core/tests/tonerom.rs` only asserted that samples were non-zero.

Channel phases now run off the system clock at GBATEK's divisors: `16*(2048-n)`
cycles per duty phase for channels 1 and 2, `8*(2048-n)` per wave sample for
channel 3, and `32*r*2^(s+1)` per LFSR step for channel 4 (with `r=0` meaning
0.5). Measured after: 127.9 Hz against 128.0 programmed.

**Application**: "audio works" had meant "samples are non-zero" for this whole
project. A test that asserts a signal exists says nothing about whether it is
the *right* signal - the pitch check took ten lines and would have caught this
at any point. Where a subsystem has a number the ROM can state and the output
can be measured against, assert on the number.

### 2026-09-01 - The battle map renders

Random-input fuzzing from `temp/cardselect.state` eventually got past CARD
SELECT to the battle map itself - "Thieves' Stronghold", terrain, the map
layout and the Locations/Geography/Units/Formations menu, all correct. That was
the last major screen unseen, so every part of *Yggdra Union* reached so far
renders properly.
