# Project conventions

## Language

Write everything in English: code, comments, documentation, commit messages
and chat replies. This is the default and does not need to be asked for.
Changelogs and PR descriptions are the one carve-out - see below.

The exception is when David asks for another language for a specific piece of
work. That applies to that piece only; the next thing goes back to English.

**Changelogs and PR descriptions are the standing exception: they are written
in Portuguese-BR.** They describe what is being merged to a human reader
rather than to the codebase, so they follow that reader's language. Everything
else stays English, commit messages included. Technical identifiers inside
them - file paths, register names, function names, instruction mnemonics -
keep their real names and are never translated.

Content that is **quoted rather than authored** keeps its original language:
excerpts from GBATEK or the ARM7TDMI manual, error strings from a library,
user-facing strings that already ship in Portuguese
(`android/app/src/main/res/values/strings.xml`). Translating those would be a
product change, not a translation.

## Where to put working files

Never use `/tmp` or any system temp directory. `/tmp` is wiped on reboot,
shutdown, freeze and automatic cleanup. Everything lives inside the project.

- **`temp/`** (gitignored) - analyses, verification scripts, disassembly
  dumps, quick notes, intermediate output, test ROMs. Things that need to
  survive a reboot but do not need to be committed.
- **`docs/`** (committed) - what needs to stay alive: consolidated analyses,
  hardware notes, specs, architecture decisions, audit records.

The deciding question: _will someone need this a month from now to understand
why a decision was made?_ Yes -> `docs/`. No -> `temp/`.

`.gitignore` blocks `*.gba`, `*.bin` and `*.bios` project-wide, so test ROMs
dropped in `temp/` stay out of git twice over. That is deliberate: **no ROM or
BIOS image is ever committed to this repository**, including homebrew test
suites. Point the tests at a path and let each developer supply the file.

## Permissions

- `git commit`, `git push`, `gh pr create` - allowed.
- `git merge` and `gh pr merge` - **blocked**. Merges are done by hand.

## Commit messages

**One short, title-like line. No body.** No AI co-author trailer, no
"Generated with" line, no tool attribution - a commit is authored by the
developer.

Bundle changes of the same kind with `and`; append a different kind after
`&&`:

```
fix: ARM7TDMI instruction dispatch and PC advance && add core test suite
```

Rationale, history and trade-offs for a merge go in the PR description
(Portuguese-BR, per the Language section), never in the commit body.

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

**`.claude/agents/*.md` is the single source of truth for every persona.** The
file holds the frontmatter (`name`, `description`, `model`, `tools`), the
repository context, the working rules, and the persona body ending in
`## Discoveries`.

- The `description` is what makes Claude Code pick the agent on its own,
  without being asked for by name. Every registered agent opens its
  description with `Use PROACTIVELY` and lists concrete triggers drawn from
  **this** project - real file paths, real register names, real symptoms -
  rather than the generic text the persona shipped with. A description that
  could apply to any repository will be selected for tasks it cannot help
  with.
- **Registering an agent for a stack this project does not have is worse than
  useless: it competes.** `mobile-developer` is parked in
  [`.claude/agents-inactive/`](.claude/agents-inactive/README.md) for exactly
  that reason - it is a React Native and Flutter persona, and this project
  rejects both by design. Moving a file out of `.claude/agents/` is the only
  reliable off switch: Claude Code scans that directory *and its
  subdirectories*, and offers no frontmatter field that disables an agent in
  place.
- Agents record what they learn in the `## Discoveries` section of their own
  `.claude/agents/<name>.md`. That is the path the `SubagentStop` hook
  (`.claude/hooks/agent-memory.py`) checks, so the convention and the
  enforcement name the same file.
- Durable knowledge about the project itself goes to `docs/` or to
  `.claude/memory.md` instead, with a one-line pointer left in `## Discoveries`.
- **A new agent needs only the `.claude/agents/` file, but Claude Code builds
  its agent list when the session opens.** A file created mid-session only
  becomes selectable in the next one.

## Formatting

- **Rust** -> `cargo fmt` (applies) and `cargo fmt --check` (checks), from
  `core/`. `cargo clippy --all-targets` for lints.
- **Kotlin** and **Swift** have no formatter wired yet; match the surrounding
  file.
- ASCII hyphens only - never the em dash or en dash character, anywhere,
  including documentation and commit messages.
