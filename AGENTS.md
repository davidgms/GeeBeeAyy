# AGENTS.md - GeeBeeAyy

**[`CLAUDE.md`](CLAUDE.md) is the authoritative version of these conventions**
and carries the parts this summary leaves out: the architecture boundary, the
testing rule, where working files go, and how the agent roster is scoped. This
file is the short form for tools that read `AGENTS.md`.

## Permissions

- `git commit` — allowed
- `git push` — allowed
- `gh pr create` — allowed
- `git merge` — **BLOCKED** — merges are done manually by the user
- `gh pr merge` — **BLOCKED** — merges are done manually by the user

## Language

- Changelogs and PR descriptions must be written in Portuguese-BR.

## Project Overview

- GBA emulator with pixel bee theme
- Rust core (emulation) + Kotlin/Android + Swift/iOS frontends
- Primary target: Android, secondary: iOS

## Structure

```
core/           # Rust emulation core (ARM7TDMI, PPU, APU, memory)
android/        # Kotlin + Jetpack Compose frontend
ios/            # Swift + SwiftUI frontend
docs/           # Documentation, assets, research
```

## Build

- Rust core: `cargo check`, `cargo build --release` (from `core/`)
- Android: `./build.sh android` or Docker variants
- Full build script: `./build.sh --help`
