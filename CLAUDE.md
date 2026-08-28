# Project conventions

## Language

The global rules carry the whole policy: English for everything, with
changelogs and PR descriptions in Portuguese-BR. What is specific here is
**what counts as quoted rather than authored**, and therefore keeps its
original language:

- excerpts from GBATEK or the ARM7TDMI manual,
- error strings from a library,
- the user-facing strings that already ship in Portuguese in
  `android/app/src/main/res/values/strings.xml`.

Translating any of those would be a product change, not a translation.

Technical identifiers inside a Portuguese changelog or PR description - file
paths, register names, function names, instruction mnemonics - keep their real
names.

## Where to put working files

`temp/` for scratch, `docs/` for what must stay alive, never `/tmp` - the
global rules cover the split. One addition specific to this repository:

`.gitignore` blocks `*.gba`, `*.bin` and `*.bios` project-wide, so test ROMs
dropped in `temp/` stay out of git twice over. That is deliberate: **no ROM or
BIOS image is ever committed to this repository**, including homebrew test
suites. Point the tests at a path and let each developer supply the file.

## Permissions and commit messages

Both are global rules and are not restated here. In short, for a contributor
reading this file on its own: `git commit`, `git push` and `gh pr create` are
allowed; `git merge` and `gh pr merge` are **not** - merges are done by hand.
Commits are one short title-like line with no body and no AI co-author
trailer, bundling with `and` and `&&`:

```
fix: ARM7TDMI instruction dispatch and PC advance && add core test suite
```

Rationale, history and trade-offs for a merge go in the PR description
(Portuguese-BR), never in the commit body.

## Architecture

Three layers, and the boundary between them is the whole design:

- **`core/`** - the Rust emulation core. Owns every cycle of GBA behaviour:
  ARM7TDMI (`cpu/`), PPU (`ppu/`), APU (`apu/`), memory bus (`memory/`, `io.rs`),
  DMA (`dma.rs`), timers (`timer/`), cartridge and saves (`cart/`), save states
  (`savestate.rs`), HLE BIOS (`bios.rs`). It knows nothing about Android or iOS.
- **`core/src/ffi.rs`** - the only door between the core and the world. A C ABI
  (`geebeeayy_*`) that iOS links against, plus JNI exports
  (`Java_com_geebeeayy_app_engine_GbaEngine_*`) behind
  `#[cfg(target_os = "android")]`. **Every new capability the frontends need
  gets an FFI function here first.** A frontend never reaches into core state
  another way.
- **`android/`** and **`ios/`** - native frontends. They render a frame buffer,
  feed audio, collect input, and manage lifecycle. They contain no emulation
  logic. Android is the primary target; iOS is secondary.

### Rules that follow from that

- **Emulation accuracy work belongs in `core/`, never in a frontend.** If a
  game looks wrong, the fix is in Rust.
- **No frameworks in the frontends' emulation path.** The README rejects
  Flutter and React Native for a measured reason: emulation needs raw audio
  buffers, direct GPU access and no interpreter between input and frame. Do
  not reintroduce them.
- **The core must stay `no_std`-friendly in spirit**: only `log` and
  `thiserror` as cross-platform dependencies. Adding a dependency to `core/`
  is an architectural decision, not a convenience.
- **Audio is the timing master.** `AudioOutput` writes blocking, so the
  emulation loop advances at the rate the device drains samples. Do not
  reintroduce `delay()`-based frame pacing as the primary clock - that is the
  single most common source of crackle and drift in mobile emulators.

## Testing

`core/tests/` is the accuracy gate, and it is the only thing standing between
a decoder change and a silently broken emulator. The core shipped for months
with every second instruction skipped because nothing tested it.

- **`core/tests/cpu.rs`** - hand-assembled instructions written into IWRAM and
  stepped directly. No ROM, no BIOS, no fixtures. Add a case here for every
  instruction-level bug fixed, using the real encoding.
- **`core/tests/integration.rs`** - hand-built ROM images driven through
  `Gba::run_frame`, asserting on the frame buffer and on memory. This is what
  catches wiring bugs between CPU, bus and PPU.
- Run with `cargo test` from `core/`.

**A decoder or timing change without a test is not finished.** When a bug is
found, the test that reproduces it is written before the fix, so the fix is
proven rather than assumed.

## Build

From `core/`:

```
cargo check                # fast compile check
cargo test                 # the accuracy gate
cargo build --release      # optimised core
cargo clippy --all-targets # lints
```

Whole project, from the root:

```
./build.sh core            # Rust core
./build.sh android         # debug APK
./build.sh check           # clippy + format
./build.sh docker-shell    # containerised toolchain (no local SDK needed)
./build-mobile.sh          # Android and iOS target builds
```

**`android/app/src/main/jniLibs/*/libgeebeeayy_core.so` is committed, and it
goes stale the moment the core changes.** A device test against a stale `.so`
tests the old bug. Rebuild it before testing on hardware.

## Agents

How agents are registered, scoped and switched off is a global rule, as is the
orchestration model - the main thread orchestrates and reviews, `agent-organizer`
proposes, specialists advise before they build. Only what is specific to this
repository is below.

**`.claude/agents/*.md` is the single source of truth for every persona.** The
file holds the frontmatter, the repository context, the working rules and the
body ending in `## Discoveries`.

- **The roster and its lanes are in [`ROADMAP.md`](ROADMAP.md#who-owns-what).**
  The boundary that matters most: emulation behaviour is `rust-engineer`'s and
  lives in `core/`; a frontend agent never receives a "the game looks wrong"
  task.
- **Agent memory is enforced, not just requested.** Agents write to the
  `## Discoveries` section of their own `.claude/agents/<name>.md`, and the
  `SubagentStop` hook (`.claude/hooks/agent-memory.py`) checks that the file
  changed before letting the agent finish. The convention and the enforcement
  name the same path on purpose.
- **Durable project knowledge goes to `docs/` or
  [`.claude/memory.md`](.claude/memory.md)** instead, with a one-line pointer
  left in `## Discoveries`. `memory.md` already records the decoder bugs that
  shipped while the roadmap called the decoders complete - which is why an
  unverified checkbox in this repository is treated as unverified.
- **Nothing is parked right now.** [`.claude/agents-inactive/`](.claude/agents-inactive/README.md)
  exists and explains itself; `mobile-developer` used to sit there as a React
  Native persona and was rewritten for the native stack instead.

## Formatting

- **Rust** -> `cargo fmt` (applies) and `cargo fmt --check` (checks), from
  `core/`. `cargo clippy --all-targets` for lints.
- **Kotlin** and **Swift** have no formatter wired yet; match the surrounding
  file.

The ASCII-hyphen-only rule is global and applies here like everywhere else.
