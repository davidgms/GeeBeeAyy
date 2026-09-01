# GeeBeeAyy! - Development Roadmap

The plan, ordered by what actually blocks the next milestone.

**How to read this file.** A box is ticked only when something verifies it - a
test in `core/tests/`, or a measurement on a device. Code existing is not the
same as code working: on 2026-08-27 the ARM and THUMB decoders were marked
"all instructions" complete while every second instruction was being skipped
and most of the THUMB instruction set was decoding to a NOP. The first test
suite ever written found four fatal bugs in minutes. Unverified work is listed
as unverified.

---

## Current state

| Module | Lines | Status |
|--------|-------|--------|
| `cpu/` | ~1900 | ARM + THUMB decoders, full register banking. **Passes gba-suite `arm`, `thumb` and `memory`.** 47 regression tests. |
| `ppu/` | ~1040 | Mode 0 tiled output verified against three test ROMs. Modes 1, 2, 4, 5, sprites, windows, mosaic and blending remain unverified. |
| `apu/` | ~800 | 4 PSG channels + FIFO A/B, full save-state serialisation, and a register map decoded at 16-bit granularity. A channel can be triggered and produces samples. Never driven by a real game. |
| `memory/` + `io.rs` | ~540 | Bus with correct region mirroring and 8-bit video write rules. `KEYINPUT` wired. |
| `dma.rs` | ~300 | 4 channels wired to the bus, immediate/HBlank/VBlank/special, correct address control and repeat semantics. Raises IF bits 8-11. |
| `timer/` | ~110 | Prescaler and cascade. Raises IF bits 3-6. |
| `cart/` | ~480 | ROM load, save type detection, SRAM and Flash wired to the bus and passing the gba-suite save ROMs, Flash chip ID, and EEPROM as the real serial protocol over DMA. |
| `savestate.rs` | ~330 | Format v4: register banks, timers, DMA derived state, the whole `io_regs` file, the APU and the cartridge save. A rejected restore rolls back. Wired to ten UI slots. |
| `bios.rs` | ~450 | HLE SWIs including `CpuFastSet`, `ArcTan`/`ArcTan2` and the three diff unfilters. `BgAffineSet`/`ObjAffineSet`/`BitUnPack` are still stubs; the sound-driver calls are absent. |
| `ffi.rs` | ~560 | C ABI + JNI: input, frame buffer, audio, battery saves with a dirty flag, and save states as bytes. 13 JNI symbols, all present in the built `.so`. |
| `android/` | ~2900 | Compose UI, JNI bridge, `AudioTrack` output, touch overlay wired to the core, battery saves and save-state slots on disk, integer scaling with nearest-neighbour filtering. **Builds, installs and emulates on a device.** |
| `rewind.rs` | ~80 | Bounded ring of save states; cadence left to the frontend. Not wired to any UI. |
| `ios/` | ~750 | SwiftUI views and an engine wrapper. **No Xcode project - has never been compiled.** |

**Verified on the host 2026-08-30**: *Yggdra Union* boots to its title screen,
and `waimanu`, `jumpingbarnabe` and `powerpig` render. See Phase 0's exit
criterion below for what was in the way. Not yet run on a device.

**Verified on hardware 2026-08-30**: all ten GBA inputs reach the core -
confirmed by 72 consecutive frames of `KEYINPUT=0x03F7` while Start was held.
`stripes` and `shades` render correctly at integer 3x with nearest-neighbour
filtering.

**Verified on hardware 2026-08-29**: built for `arm64-v8a`/`armeabi-v7a`,
installed on a Xiaomi Mi 10T Pro (Android 12), and ran jsmolka's `hello.gba` -
"Hello world!" rendered correctly. The ROM browser scans and lists, the app
launches without crashing, and `System.loadLibrary` finds the core. No
*commercial* game has been tried, and the overlay is still missing Start,
Select and the shoulder buttons, so most games are unreachable.

*(Superseded 2026-08-30: Start, Select, L and R are now on the overlay.)*

---

## Phase 0 - Make a game boot

Everything here is a hard blocker. None of it is optional, and the order is
roughly the order to do it in.

### 0.1 Input - `rust-engineer`, then `kotlin-specialist`

- [x] Initialise `0x04000130` (`KEYINPUT`) to `0x03FF` in `MemoryBus::new`.
      The register is active-low and `io_regs` is zero-filled, so today every
      read reports all ten buttons held down, forever.
- [x] Add `geebeeayy_set_keys(handle, u16)` to `core/src/ffi.rs`, plus the JNI
      export.
- [x] Call it from the touch overlay in `EmulationScreen.kt`. Unverified:
      no Android toolchain here, so CI is what will compile it.
- [x] Test: a ROM that polls `KEYINPUT` sees released buttons by default and
      pressed ones after `set_keys`.

### 0.2 Interrupts - `rust-engineer`

- [x] Raise the timer IRQ. `core/src/timer/mod.rs:59` is a `TODO` with a
      `let _ = bus;` standing in, so a timer interrupt never fires. Games that
      wait on one hang, and DMA-sound refill is timer-driven.
- [x] Raise the DMA IRQ. Same shape at `core/src/dma.rs:161`.
- [x] Test: enabling a timer with IRQ set eventually sets the matching `IF`
      bit and enters the handler.

### 0.3 Banked registers - `rust-engineer`

- [x] Swap `SP` and `LR` on mode change, and `R8-R12` for FIQ.
      `fiq_registers` and `irq_registers` (`core/src/cpu/mod.rs:17`) exist and
      are serialised into save states, but nothing ever swaps them. IRQ mode
      therefore runs on the game's own stack, and the HLE BIOS handler pushes
      six registers onto it. Any game that sets up a separate IRQ stack
      corrupts memory on its first interrupt.
- [x] Test: entering IRQ mode uses the IRQ stack pointer; returning restores
      the caller's.

### 0.4 The accuracy gate - `rust-engineer` with `search-specialist`

- [x] Run jsmolka's `gba-suite` (`arm.gba`, `thumb.gba`, `memory.gba`). Each
      writes the failing test number to `r12` and spins, so the harness is
      small: run frames until PC stops moving, assert `r12 == 0`.
- [x] Point it at `temp/roms/` and skip when the file is absent. **No ROM is
      committed to this repository**, homebrew test suites included.
- [x] Fix what it finds. **All three suites pass.**

It was the highest-value task in Phase 0 and it earned that: `arm.gba`,
`thumb.gba` and `memory.gba` all pass as of 2026-08-28, after eleven further
decoder and memory-map bugs that the hand-written tests had missed entirely.

### 0.5 Rebuild the native library - `mobile-app-developer`

- [x] `android/app/src/main/jniLibs/*/libgeebeeayy_core.so` are **not
      committed at all** - `.gitignore`'s `*.so` rule matches them, and
      `git ls-files` confirms neither copy is tracked. They only exist on
      whichever machine last ran a local build, so a fresh clone has no
      native library and a device build against it would
      `UnsatisfiedLinkError` on the first JNI call, not run stale code.
      `.github/workflows/ci.yml`'s `android` job now builds both ABIs with
      `cargo-ndk` on every push and uploads them as workflow artifacts,
      which is a better answer than committing binaries by hand. Local
      device testing still needs `./build-mobile.sh android-all` or a
      downloaded CI artifact copied into place first.
- [x] `android/app/src/main/cpp/` deleted. It was an unbuilt second JNI
      implementation with no Gradle wiring, still calling three FFI symbols
      that no longer exist.

**Exit criterion:** a commercial ROM reaches its title screen with correct
graphics, and `gba-suite`'s ARM and THUMB suites pass.

**Met 2026-08-30.** *Yggdra Union: We'll Never Fight Alone* boots from a cold
start through the health-and-safety screen to its title screen, artwork and
all. `waimanu`, `jumpingbarnabe` and `powerpig` render too, having shown
nothing at all before the same day's fixes. What stood between the suites
passing and a game booting was four bugs `gba-suite` does not cover, recorded
in [`.claude/memory.md`](.claude/memory.md): an unsigned THUMB `B` offset, a
PPU that overwrote the game's DISPSTAT, a halted CPU that stepped over HBlank,
and an HLE BIOS interrupt handler that acknowledged IF on the game's behalf.

Verified on a device 2026-08-31: *Yggdra Union* boots, plays its opening and reaches the title screen on a Xiaomi Mi 10T Pro, with music. The
opening only became correct once the sprite renderer was fixed - it had
been reading OBJ tiles from BG VRAM, so no sprite this emulator ever drew
was correct.

---

## Phase 1 - Make it playable

- [x] **Missing SWIs** - `rust-engineer`. `CpuFastSet` (0x0C),
      `ArcTan`/`ArcTan2` (0x09/0x0A) and the three diff unfilters (0x16/0x17/0x18
      - GBATEK's numbering, not the 0x16/0x17 this line used to claim) are in,
      covered by `core/tests/bios.rs`. The same pass fixed four latent bugs in
      the neighbouring calls: ARM-mode `swi` read the function number from the
      wrong bits, `CpuSet` ignored its fill flag, and both `lz77_decompress` and
      `rl_decompress` looped forever because they never read the header's output
      size. Still missing: `Stop` (0x03), `GetBiosChecksum` (0x0D),
      `HuffUnComp` (0x13), the sound driver calls (0x1A-0x24, 0x28-0x2A),
      `MultiBoot` (0x25) and `HardReset`/`CustomHalt` (0x26/0x27). `BgAffineSet`
      and `ObjAffineSet` (0x0E/0x0F) are still stubs that write an identity
      matrix and ignore the requested angle, and `BitUnPack` (0x10) is worse
      than a stub - it copies bytes until it reads a zero, so a game that calls
      it gets corrupted memory instead of nothing.
      Two decompressor bugs were fixed on 2026-08-31: `LZ77UnCompVram` and
      `RLUnCompVram` wrote their output a byte at a time, and VRAM turns a byte
      store into two copies of that byte across the halfword - which is the
      whole reason those Vram variants exist.
- [x] **Save state FFI is now bytes, not an opaque handle** - `state_size`,
      `state_read`, `state_write` replace `save_state_create`/`load_state`/
      `save_state_destroy`. States were previously not exportable to a file at
      all, which is a bigger problem than "not wired to a UI".
- [x] **Save states in the UI** - `kotlin-specialist`. Ten slots per game in
      `filesDir/states/`, written atomically (temp file, then rename), keyed by
      the ROM header's title (0xA0) and game code (0xAC) so two carts cannot
      collide. Engine calls stay on the emulation thread via the same
      command-mailbox pattern as `keyState`; a rejected load (bad version,
      truncated file) stops emulation and surfaces a message rather than
      running on the hybrid state `SaveState::restore` can leave behind.
      Unverified: not compiled, no device test.
- [x] **Battery saves: cart wired to the bus** - the cartridge now lives in
      `MemoryBus`, the 0x0E000000 region is mapped with its 8-bit databus
      semantics, Flash gained chip erase and the two-stage erase unlock, and
      the four-function FFI (`save_size`, `save_read`, `save_write`,
      `save_take_dirty`) plus JNI exports are in. **`gba_suite_save_sram`,
      `save_flash64`, `save_flash128` and `save_none` all pass.**
- [x] **Battery saves: persist them on the Android side** - `kotlin-specialist`.
      `<romfile>.sav` next to the ROM, loaded before the first frame runs;
      falls back to app-private storage if that write fails. Flush is
      debounced 2s off the core's dirty flag, plus an unconditional flush on
      pause, stop and `onCleared`. Unverified: not compiled, no device test.
- [x] **Flash chip ID reads** - `0x90` enters ID mode and `0xF0` leaves it;
      reads return Panasonic `1B32h` for 64K and Sanyo `1362h` for 128K, per
      GBATEK's device table. Games probe this before writing and a wrong answer
      means they refuse to save.
- [x] **Save state v3** - timers, DMA derived state, the whole `io_regs` file
      and the cartridge save now round-trip, and a rejected state rolls back
      instead of half-applying. Verified by a test that runs frames, snapshots,
      diverges, restores and requires the same frames back. See
      `docs/save-data.md`.
- [x] **APU state in the save state (v4)** - all 78 fields across the six APU
      structs, generated as matching write/read pairs so the orders cannot
      drift. The sample buffer is deliberately excluded: it is drained to the
      frontend every frame, so snapshotting it would replay stale audio.
- [x] **APU register map rewritten** - decoding now happens at 16-bit register
      granularity rather than per byte, which is where the bugs lived:
      `SOUND1CNT_H`'s byte split was off by one, `0x64` was treated as a high
      byte and shifted left 8 when it is the low byte, and **`0x65` had no
      handler at all** - which is where the trigger bit sits, so no PSG channel
      could ever start. `Gba::apu_sound_write` also matched a hand-listed set
      of offsets and dropped roughly half the sound registers plus all of wave
      RAM; it now folds every captured byte onto its containing register.
      Two adjacent fixes: `SOUNDCNT_X` (the PSG/FIFO master enable) was not
      routed to the APU at all, and `sound_on` was read from SOUNDCNT_H bit 15,
      which GBATEK defines as "DMA Sound B Reset FIFO".
      Verified by `a_psg_channel_can_actually_be_triggered` and
      `the_master_enable_silences_the_apu` - the first tests in this project's
      history that get audio out of the APU.
- [x] **Audio verified on hardware** - a tone ROM drives PSG channel 1 and the
      device reports `AudioPlaybackConfiguration ... state:started`, so samples
      reach the audio HAL. Covered by `core/tests/tonerom.rs`. A *game* still
      has not driven it.
- [x] **The core emits 48 kHz** - the sample clock is now an exact fraction
      (`sample_accum += cycles * 48000`, a sample due each time it reaches
      16777216) rather than an integer `CYCLES_PER_SAMPLE` of 964, which gave
      17403 Hz - a rate no device supports, so AudioTrack resampled every
      buffer and refused the fast path (`AUDIO_OUTPUT_FLAG_FAST denied by
      server`), leaving `PERFORMANCE_MODE_LOW_LATENCY` doing nothing.
      Moving the rate forced a much bigger bug into the open: **the PSG
      channels' phase was clocked by emitted samples, not by the system
      clock**, so their pitch was tied to the output rate and the tone ROM's
      128 Hz square was playing at 2 Hz - 64x flat, since the project began.
      Channel phases now advance in cycles at GBATEK's divisors. Covered by
      `a_psg_square_plays_the_frequency_the_rom_asked_for`.
      Not yet confirmed: whether the device now grants the fast path. That
      needs a logcat check on hardware.

**Exit criterion:** a full game is playable start to finish, with sound, on a
physical device, without losing progress.

**Status 2026-08-31:** *Yggdra Union* runs on a device through its opening to
the title screen with music playing, but nobody has yet pressed Start into the
game proper. Everything verified so far is the opening and the attract loop -
both of which turned out to be background-only until the sprite fix, which is
precisely why the sprite renderer stayed broken for so long. The next crop of
bugs is behind that Start press.

---

## Phase 2 - Quality

- [ ] Customisable touch overlay: size, position, opacity, per-game layouts.
- [x] **Screen scaling** - `kotlin-specialist`. `EmulationScreen.kt`'s `GbaScreen`
      now supports Fit (largest size preserving 3:2, letterboxed), Integer
      (largest whole-number multiple, falling back to Fit below 240x160) and
      Stretch (the old fill-everything behaviour); default is Integer.
      Persisted in `DisplaySettings` (plain `SharedPreferences`, matching
      `RomFolderManager`'s pattern - no DataStore dependency exists in this
      project) and changed from `SettingsScreen.kt`. Verified on a device
      2026-08-31, after the portrait padding was cut from 24.dp to 8.dp -
      24.dp left 948 px of a 1080 px screen and integer scaling rounds that
      down to 3x, where 8.dp clears 960 px and gets 4x.
- [x] **Pixel-perfect filtering** - `drawImage`'s `filterQuality` is set to
      `FilterQuality.None` explicitly rather than left at the bilinear
      default, so a 240x160 frame scaled up keeps hard pixel edges. 2xSaI and
      CRT remain undone; they need their own shader/sampling work, not a flag.
      Unverified: not compiled, no device test.
- [ ] Screen filters: 2xSaI, CRT.
- [ ] ROM library with cover art and metadata.
- [x] **Landscape/portrait handling** - `kotlin-specialist`. The manifest no
      longer hard-locks `screenOrientation="portrait"`;  `MainActivity`
      applies the lock at runtime instead, from a `DisplaySettings.forcePortrait`
      toggle that defaults to `true` so an existing install's behaviour does
      not change until the player opts into landscape from Settings. In
      landscape, `EmulationScreen` flanks the play area with the D-pad on the
      left and action/transport buttons on the right instead of stacking
      controls under it. Unverified: not compiled, no device test, and the
      `Configuration.ORIENTATION_LANDSCAPE` recomposition path in particular
      depends on `android:configChanges="orientation|..."` actually keeping
      Compose's `LocalConfiguration` live without recreating the Activity -
      that is documented Compose behaviour, not something exercised here.
- [ ] Input latency measured and driven under 45 ms; consider runahead.
- [ ] Icon and store asset set - `visual-asset-generator`.

---

## Phase 3 - iOS

The iOS target has never been compiled. Owned by `swift-expert`, with
`mobile-app-developer` on the build.

- [ ] Create the Xcode project (or `Package.swift`) - there is currently
      neither.
- [ ] Build the core as a static library for `aarch64-apple-ios` and the
      simulator target.
- [ ] Verify the bridging header against the real C ABI in `core/src/ffi.rs`.
- [ ] Audio via `AVAudioEngine`, mirroring Android's blocking-write approach
      so the audio device is the timing master.
- [ ] Touch controls, MFi controllers, save states, iCloud sync.
- [ ] TestFlight, then App Store.

---

## Phase 4 - Advanced

- [x] **Rewind (core side)** - `core/src/rewind.rs`, a bounded ring of save
      states with the snapshot cadence left to the frontend, same as save
      flushing. A state measures 512,128 bytes, so `Rewind::memory_bytes`
      reports the live cost and a caller sizes itself against the device
      instead of guessing. Not yet wired to any UI.
- [ ] Cheat codes (GameShark / CodeBreaker).
- [ ] Link cable over local WiFi.
- [ ] JIT recompilation, ARM host only. Only after the interpreter is correct -
      a JIT built on a wrong interpreter inherits every bug and makes it harder
      to find.
- [ ] Screen recording and screenshots.
- [ ] Debug tools: breakpoints, memory viewer, register inspector.
- [ ] RetroAchievements.

---

## Known accuracy gaps

Deliberate simplifications, recorded so they are not rediscovered as bugs.
Two earlier entries are gone because gba-suite forced them to be fixed
properly: misaligned halfword loads now rotate and degrade correctly, and the
HLE BIOS IRQ handler now uses the standard `LR = return + 4` entry with a
`subs pc, lr, #4` return.

- No OAM DMA, and no video capture DMA (`core/src/dma.rs:201`).
- No BIOS execute permission checks.
- Sprite priority against backgrounds is correct in **mode 0 only**. The other
  modes lay sprites on top of everything instead of ordering them, which is
  wrong when a background outranks the sprite - but it is what makes sprites
  appear in the bitmap modes at all, since modes 3-5 never ran the sprite pass
  before 2026-09-01.
- Windows, the OBJ window and semi-transparent sprites all work as of
  2026-09-01. What remains approximate: colour effects outside mode 0 still
  apply to a whole scanline rather than per pixel, so brightness and alpha are
  only correct in mode 0.
- Affine backgrounds render, as of 2026-09-01, but their per-pixel path is not
  cycle-shaped: PB and PD are accumulated once per scanline rather than being
  applied inside the line, which is right for the ordinary case and wrong for
  a game that rewrites the matrix mid-line.
- **Colour effects outside mode 0 are a scanline-wide approximation.** Mode 0
  composites the top two layers per pixel and blends correctly; every other
  mode still applies brightness to the whole line and cannot alpha-blend at
  all. Semi-transparent sprites (OBJ mode 1) are not blended in any mode.
- **`BitUnPack` (SWI 0x10) is not a stub, it is wrong.** It copies bytes until
  it hits a zero, which is nothing like the real call - the real one expands
  1/2/4/8-bit source units into wider destination units using a five-byte
  parameter block. A game that uses it gets silently corrupted memory rather
  than a no-op.
- The exact cycle on which a timer or DMA raises its IF bit is not modelled;
  the flag is set when the overflow or the transfer completes. GBATEK does not
  document the sub-cycle behaviour, so settling it needs a hardware capture or
  a timing test ROM rather than more reading.
- HALT wake-up granularity is one PPU event (HBlank start or the end of a
  scanline), not one cycle. It used to be a whole scanline, which stepped
  straight over HBlank and gave a halted game one interrupt a frame
  instead of 228.
- PPU and APU *timing* still have no test-ROM coverage. gba-suite exercises
  the CPU and the memory bus only. Rendering now has some: jsmolka's ppu
  ROMs, the 240p Test Suite and Celeste Classic in `core/tests/ppu.rs`,
  plus `core/tests/dma.rs` and `core/tests/tonerom.rs`. None of them check
  *when* anything happens, only what comes out.

---

## Who owns what

| Area | Agent |
|------|-------|
| `core/` - emulation, decoders, FFI | `rust-engineer` |
| Android frontend | `kotlin-specialist` |
| iOS frontend | `swift-expert` |
| Storage, lifecycle, battery, permissions across both platforms | `mobile-developer` |
| Build, release, device testing, platform parity | `mobile-app-developer` |
| GBA hardware questions, GBATEK, reference emulators | `search-specialist` |
| Touch targets, contrast, screen readers | `accessibility-tester` |
| Icons, theme art, store assets | `visual-asset-generator` |
| Multi-lane decomposition and cross-specialist consultation | `agent-organizer` (second in command; its plans are reviewed before they run) |

---

## References

| Resource | Link |
|----------|------|
| GBATEK (hardware reference) | https://problemkaputt.de/gbatek.htm |
| TONC (GBA programming) | https://www.coranac.com/tonc/text/toc.htm |
| ARM7TDMI TRM | https://developer.arm.com/documentation/ddi0029/ |
| gba-suite (test ROMs) | https://github.com/jsmolka/gba-suite |
| mGBA (reference implementation) | https://github.com/mgba-emu/mgba |
| SkyEmu (per-pixel PPU) | https://github.com/skylersaleh/SkyEmu |
| NanoBoyAdvance (cycle accuracy) | https://github.com/nba-emu/NanoBoyAdvance |
