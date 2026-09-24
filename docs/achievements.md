# Achievements

Research 2026-09-23. The question was "can GeeBeeAyy have achievements, what
does it take, does it work on every game, and is it legal". Everything marked
**measured** was checked directly in this session; everything else is labelled.

## The short answer

**Yes, and the way to do it is RetroAchievements.** There is no second option
worth naming: the value is not the mechanism, it is the ~769 GBA achievement
sets other people have already authored. Nobody else has those.

But it arrives in two stages, and the second one is gated by a calendar rather
than by code:

1. **Softcore now.** Achievements unlock, the player sees them, progress syncs
   to their RetroAchievements account. Save states, rewind and fast-forward all
   keep working.
2. **Hardcore later, or never.** Hardcore unlocks are only granted to emulators
   on RetroAchievements' approved list, and the rule is that an emulator "must
   have been publicly available for at least 6 months before it will be
   considered". GeeBeeAyy is not publicly available at all yet. So hardcore is
   at minimum six months after a public release, whatever we build.

That is not a reason to wait. It is a reason to build the softcore path and not
to promise hardcore.

## How it actually works

An achievement is a small expression over **emulated memory**, evaluated once
per frame. "Coins == 100", "boss HP hit 0 while in room 7". The emulator's job
is to hand a library the bytes and call it every frame; the library decides
what unlocked and when to talk to the server.

The library is **`rcheevos`**, from RetroAchievements itself.

| | |
|---|---|
| Language | **C** (measured) |
| Licence | **MIT** (measured, via the GitHub API) |
| Size | **36,157 lines** of `.c` and `.h` across `src/` and `include/` (measured) |
| Networking | **none.** It builds request URLs and parses responses; the client does the HTTP. |

It is deliberately transport-free, which matters for us: no HTTP stack is
imposed, and the existing `INTERNET` permission already covers the calls.

## What our core would have to grow

Very little, and this is the good news. rcheevos does not want the whole
address space. Its GBA memory map is **three regions** (measured, from
`src/rcheevos/consoleinfo.c`):

| region | address | size |
|---|---|---|
| Internal Work RAM | `0x03000000` | 32 KB |
| External Work RAM | `0x02000000` | 256 KB |
| Game Pak SRAM | `0x0E000000` | 64 KB |

**352 KB, read-only, no writes.** Nothing about VRAM, palettes, I/O registers
or the cartridge ROM.

So `ffi.rs` grows one capability: a way to see those three blocks. The cheap
shape is a pointer per region, the same pattern `geebeeayy_frame_buffer_ptr`
already uses, so the achievement runtime reads them directly rather than
crossing the FFI boundary thousands of times per frame. Plus a reset signal,
which the runtime needs to know a run restarted.

**`core/` stays pure Rust.** rcheevos is C, and it does **not** belong in the
core - it would be the first C dependency in a crate whose whole rule is `log`
and `thiserror`. It belongs on the Android side, built with the NDK, talking to
the core through the FFI it already has. The core's job is to expose memory;
deciding what an achievement means is not emulation.

## Does it work on every original game?

**No, and this is the part to be honest about.**

- **769 of 2,170** GBA games on RetroAchievements have an achievement set
  (secondary: RetroAchievements' own system page, which refuses automated
  fetches, so this is via search rather than read directly). Roughly a third.
  The rest are recognised but have nothing to unlock.
- A game with no set degrades quietly: the emulator asks, the server says
  "nothing here", play continues. No error worth showing.

**How a ROM is identified is the sharper problem.** For GBA, rcheevos hashes
**the whole ROM file with MD5** (measured: GBA falls into the
`rc_hash_buffer` branch in `src/rhash/hash.c`, which is a plain buffer hash of
the file). That means:

| ROM | matches |
|---|---|
| exact No-Intro dump | **yes** |
| renamed | **yes** - the name is not part of the hash |
| **trimmed** | **no** - the bytes changed |
| **patched** (translation, randomiser, romhack) | **no** |
| homebrew | only if somebody authored a set for it |

Note that this is the opposite trade-off from the cover art work, where the
cart's own game code carried us through renames and trims. Here the hash is the
identity, and a trimmed set - which is common - simply will not match.

## Legality

**This one is clean, which is unusual for anything near emulation.**

- The library is **MIT**. No obligation beyond keeping the notice.
- Integrating is not merely permitted, it is the documented purpose:
  RetroAchievements publishes the library, documents `rc_client_t` as the
  entry point for emulator authors, and maintains a list of supported
  emulators.
- The achievement data belongs to RetroAchievements and its community authors.
  We display it; we never redistribute it, and there is nothing to bundle.
- **An account is required, and it is the player's own**, created on
  retroachievements.org. We never hold a developer key for it - unlike the
  separate web API, which does use a key and which we would not be using.
- Nothing here is a ROM, a BIOS, or publisher artwork, so none of the usual
  Google Play emulator hazards apply.

Where it gets conditional is hardcore, and those are RetroAchievements' rules
rather than law: an emulator must be approved, use a stable structured
User-Agent (`EmulatorName/v1.0.0 (OSName 10.0)`), and have been public for six
months. **Not legal advice.**

## What hardcore would take away

If we ever pursue it, hardcore compliance requires **disabling**, while it is
on:

- **save states** (loading is always blocked),
- **rewind**,
- **slowdown and frame advance**,
- **cheats**, memory editors, debuggers, scripting, TAS input playback.

We have save states, rewind and fast-forward today. Fast-forward is not on that
list - slowdown and frame advance are - but do not assume it is fine until
somebody at RetroAchievements says so.

Softcore asks for none of this. Everything keeps working.

## Rough cost

| | |
|---|---|
| Core (Rust) | pointers to three memory regions plus a reset signal. **Small** - a day, most of it tests. |
| Android (C) | build rcheevos with the NDK, wire `rc_client_t`, HTTP callbacks, the per-frame poll. **The bulk of the work.** |
| Android (Kotlin) | login screen, an unlock toast, a per-game achievement list, a settings section. |
| Whole thing | **1 to 2 weeks**, honestly. |

## The smaller version, and why not

The obvious "cheap" version is achievements local to this app: no account, no
server, no library. It is a trap. The entire value is the sets other people
wrote, and a local system means **we** author achievements, per game, by
finding the right memory addresses by hand. That is a full-time job for one
game, let alone 769.

If the cost of the real thing is too high right now, the honest move is to do
nothing rather than to ship a hollow version of it.

## Sources

- <https://github.com/RetroAchievements/rcheevos> - the library; MIT, C, 36k lines
- <https://docs.retroachievements.org/general/hardcore-compliance-requirements.html> - what hardcore forbids, the User-Agent format, the six-month rule
- <https://docs.retroachievements.org/general/emulator-support-and-issues.html> - the supported-emulator list
- <https://api-docs.retroachievements.org/> - the separate web API, which needs a developer key and which an emulator integration does not use
- <https://retroachievements.org/system/5-game-boy-advance/games> - the GBA game list (refuses automated requests; the 769/2170 figure is via search)
