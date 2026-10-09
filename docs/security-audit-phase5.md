# Phase 5 security and privacy audit

Audit of `master` at `22e0e68`, 2026-10-08. Audit only: nothing here is fixed
yet. The fix scope waits for review.

Six lanes, raw reports in `temp/phase5/` (not committed):

| Lane | Report | Raw ids |
|---|---|---|
| Rust core and native boundary | `core.md` | CORE-1..9 |
| Storage, permissions, exports, logs | `storage-permissions.md` | STORE, PERM, EXPORT, LOG |
| Android network and untrusted input | `android-input-network.md` | NET, INPUT |
| Kotlin side of JNI (second opinion) | `boundary-kotlin.md` | JNI-K-1..13 |
| Play reviewer (second opinion) | `play-review.md` | - |
| External research | `external.md` | - |

Severity scale: **Critical** = memory corruption reachable from outside.
**High** = crash or data exposure from a file or response a player can
realistically meet. **Medium**, **Low**, **Info** as usual. Release blockers
(things Play would reject) are listed apart, because they are not
vulnerabilities.

Result: **0 Critical, 3 High, 14 Medium**, plus Low and Info, and 5 release
blockers. A second pass compared every finding with the documented security
history of 11 other emulators; see the section at the end. Tests that could
not run yet are in [`security-audit-phase5-pending-tests.md`](security-audit-phase5-pending-tests.md).

## How the audit was run

- Hostile-input probes for the core live in `temp/phase5/probes/` (modes
  `swi`, `rom`, `romfuzz`, `statefuzz`, `fields`, `edge`, `byte`). 200 random
  ROMs, 500 mutated save states, every SWI with hostile registers, targeted
  edits of every restored field.
- **Every probe runs under `prlimit --as=4000000000`.** An uncapped run of the
  state fuzzer reached 15.7 GB, was OOM-killed, and restarted WSL three times.
  That run is CORE-2 below.

## High

### H1 - Save state can make the core allocate without limit (CORE-2)
- `core/src/apu/mod.rs:1041` restores `sample_accum` as a raw `u64`. `tick`
  (`apu/mod.rs:627-630`, `649-719`) then pushes one sample per `GBA_CLOCK` in
  it, in a `while` loop, into `sample_buffer`.
- One byte (state offset 511897, the top byte of `sample_accum`) is enough.
  `load_state` reports success, the first `run_frame` grows the Vec until the
  allocator fails. That is an **abort**, not a panic: `guarded` and
  `catch_unwind` cannot stop it. On the phone the app dies.
- Reach: state files are app-private today. A bad file arrives through a
  backup restore (M1), a rooted device, or any future state import or sync.
- Repro: `prlimit --as=4000000000 temp/phase5/probes/target/release/phase5-probes byte 511897 EE`
  prints `memory allocation of 4294967296 bytes failed`. All 4 aborts in 500
  fuzz seeds were this byte or 511896.
- Fix: `sample_accum %= GBA_CLOCK` (or reject) on restore. Test first.

### H2 - ROM or zip read fully into the heap (INPUT-1, JNI-K-6, STORE-5)
- `android/app/src/main/java/com/geebeeayy/app/data/RomBytes.kt:34-47` calls
  `readBytes()` on the file or zip entry with no size cap. The caller catches
  only `Exception` (`EmulationViewModel.kt:483`), and `OutOfMemoryError` is not
  one, so a zip bomb or a huge `.gba` crashes the app on launch.
- The same unbounded read feeds the delete-saves dialog on the UI thread
  (INPUT-2) and the header read for every cover row (INPUT-3).
- The core already rejects ROMs over 32 MB (`cart/mod.rs:93`), but only after
  Kotlin has allocated the whole file.
- Fix: reject files and zip entries over 32 MB, and enforce the cap while
  streaming (zip headers can lie). Read only the first 0xC0 bytes for headers.
  The SAF rewrite (B1) touches the same read path.

### H3 - RetroAchievements calls race the emulation thread (JNI-K-1, JNI-K-2, CORE-6)
- `RaEngine` natives other than `doFrame` run outside `engineLock`:
  `unloadGame` (`EmulationViewModel.kt:418`), `achievements()` and
  `leaderboards()` from Compose (`EmulationScreen.kt:356, 365, 2128, 2171`),
  `logout` (`SettingsScreen.kt:507`), and `nativeServerResponse` straight from
  the network thread (`RaEngine.kt:325-333`, racy `attachedHandle` check).
- Correction to JNI-K-1: `rc_client` itself is **not** unlocked. rcheevos is
  built without `RC_NO_THREADS`, and `rc_client.c` takes its own mutex
  (52 `rc_mutex_lock` sites). So the rcheevos lists are safe on their own.
- What is not covered by that mutex: `g_gba` (`ra_bridge.c:33`), the raw core
  pointer that rcheevos reads through during game-load address validation.
  If that runs on the network thread while the emulation thread is inside
  `run_frame`, Rust holds `&` and `&mut` to the core at once (undefined
  behaviour). If it overlaps `load_rom` or `nativeDestroy`, it is a
  use-after-free read. Also, the callbacks rcheevos fires go into Kotlin on
  whatever thread called in (JNI-K-7 StateFlow race).
- Severity: High because it is the only path to memory unsafety found, even
  though it needs a timing window rather than a crafted input.
- Fix: deliver every rcheevos call (except the HTTP send) through
  `engineLock` or one single-thread dispatcher; decide queue-or-deliver under
  that lock; make `g_gba` atomic or lock-guarded; detach on every path that
  stops using the handle (JNI-K-4).

## Medium

| Id | Finding | Where | Fix direction |
|---|---|---|---|
| M1 | Auto Backup on with no rules: RA token, prefs, saves and states go to Google Drive and device transfer (STORE-1, NET-4) | `AndroidManifest.xml:30` | `backup_rules.xml` and `data_extraction_rules.xml` that exclude `shared_prefs/retroachievements.xml`; keep saves in on purpose |
| M2 | BgAffineSet / ObjAffineSet loop up to 2^32 times on guest `r2`; one SWI hangs the emulation thread (CORE-1) | `core/src/bios.rs:358-395` | cap the count, wrapping arithmetic |
| M3 | Timer reload 0x10000 from a state: divide by zero every step, release too; `guarded` catches it, game freezes silently (CORE-3) | `core/src/timer/mod.rs:164`, read at `savestate.rs:261` | reject reload and counter > 0xFFFF on restore |
| M4 | rcheevos pin misses #554 (progress deserialization over-read and endless loop); `ra_bridge.c:603` feeds it the state sidecar. Reproduced: a 64-byte sidecar hangs our vendored rcheevos forever while it holds the rc_client mutex, so the app likely stops responding (EMU-CORE-40, JNI-K-8) | submodule at `e3f7b7d` | bump to `e9a6d58` or later, fix CMake globs for #552; check sidecar result |
| M5 | Battery `.sav` lives beside the ROM on shared storage; a planted newer file wins (STORE-3) | `EmulationViewModel.kt:508-513`, `BatterySaveStore.kt` | authoritative copy in app-private storage (part of B1) |
| M6 | Unbounded HTTP bodies: RA response into memory, covers, badges and homebrew to disk (NET-2) | `RaEngine.kt:314-316`, `CoverArt.kt:172`, `BadgeCache.kt:69`, `HomebrewDownloader.kt:77-84` | per-client byte ceiling |
| M7 | Cover lookup sends the raw ROM file name to libretro when the game code is unknown (NET-3) | `CoverArt.kt:68-74` | send only names from the bundled No-Intro index |
| M8 | `RaEngine.start` check-then-set: double `nativeInit`, `store` seen as null (JNI-K-5) | `RaEngine.kt:55-56, 123-135` | synchronise `start()` |
| M9 | Zip inflated on the UI thread for the delete-saves dialog: ANR or OOM (INPUT-2) | `RomBrowserScreen.kt:331` | off main thread, H2 caps |
| M10 | Header read inflates the whole zipped cart per visible row (INPUT-3) | `RomHeader.kt:65-69` | read 0xB0 bytes of the stream |
| M11 | Stale RA response delivered to the next game's session after detach (JNI-K-2 second half) | `RaEngine.kt:152-157, 198-201` | drop queued responses from a previous session |
| M12 | Raw core pointer stays attached on failed load and the leak path (JNI-K-4) | `RaEngine.kt:144-146`, `EmulationViewModel.kt:445, 1058` | detach on every path; folded into H3 |
| M13 | RA token in plain SharedPreferences (STORE-2, NET-5) | `RaCredentials.kt:17-30` | rated Low once M1 is fixed; optional Keystore wrap |
| M14 | A save state from another game loads, and its battery save replaces the player's; survives reset, written to `.sav` on the next in-game save. ROM hacks share the original's header, so they share state slots (EMU-CORE-14) | `core/src/savestate.rs:189-326` | store a ROM hash in the state and reject a mismatch, as mGBA and PPSSPP do |

## Low

| Id | Finding | Where |
|---|---|---|
| L1 | Restored `cycles` near u64::MAX wraps the frame target, emulation freezes (CORE-4) | `core/src/lib.rs:292`, `savestate.rs:329` |
| L2 | JNI `nativeStateWrite` / `nativeSaveWrite` skip `guarded`; C ABI versions have it (CORE-5) | `core/src/ffi.rs:833-839, 924-933` |
| L3 | `ra_bridge.c` passes JNI NULLs on unchecked; server text into `NewStringUTF` aborts under CheckJNI (CORE-7) | `ra_bridge.c:398, 415, 578, 540-543, 659-662` |
| L4 | No scheme or host check before credentials go into a request (NET-1, JNI-K-11) | `RaEngine.kt:289-306` |
| L5 | Badge host and name come from the server; `..` passes the name check (NET-6) | `BadgeCache.kt:44-62` |
| L6 | Homebrew downloads from mutable GitHub branches, no hash (NET-7, STORE-5) | `HomebrewDownloader.kt`, `HomebrewCatalog.kt` |
| L7 | Audio count trusted; negative collides with the rewind sentinel (JNI-K-3) | `EmulationViewModel.kt:636-697` |
| L8 | `onEvent` StateFlow read-modify-write from several threads (JNI-K-7) | `RaEngine.kt:344-346` |
| L9 | Screenshots through raw path to public Pictures; breaks without all-files access (STORE-4) | `EmulationViewModel.kt:901-908` |
| L10 | Dead VIEW intent filter on `application/octet-stream` (EXPORT-1, INPUT-6) | `AndroidManifest.xml:47-53` |
| L11 | Full paths in logs; `Log.i` not stripped in release (LOG-1..3) | `EmulationViewModel.kt:425, 540`, `proguard-rules.pro` |
| L12 | READ without WRITE on API 26-29: on-card `.sav` never works there (PERM-2) | `AndroidManifest.xml:25-26` |
| L13 | A crafted state leaves the EEPROM chip stuck for the session: reads all FF, writes dropped, reset does not clear it; `.sav` on disk untouched (EMU-CORE-13) | `core/src/cart/mod.rs:534` |

Info: CORE-8 (debug-only overflow on restored counters), CORE-9 (iOS bridging
header names functions the C ABI does not export, so iOS cannot link),
JNI-K-9, JNI-K-10, JNI-K-12 (dead wrappers, no signature-diff check),
JNI-K-13, NET-8, STORE-6, PERM-3, EXPORT-2, INPUT-4, INPUT-5, INPUT-7.

## Release blockers (Play, not security)

| Id | Blocker | Status |
|---|---|---|
| B1 | `MANAGE_EXTERNAL_STORAGE` (PERM-1): Play allows it only for file managers and similar; an emulator has a working alternative. Move to SAF `ACTION_OPEN_DOCUMENT_TREE` + persisted Uri, saves app-private, screenshots via MediaStore. Optional sideload flavor keeps the permission. | Largest item, a real rewrite of `RomFolderManager`, `RomFiles`, `RomBytes`, `RomHeader` and the load path |
| B2 | No release signing and no AAB in CI (LOG-4), `versionCode = 1` | confirmed |
| B3 | targetSdk is 35 (`build.gradle.kts:14`); new apps likely need 36 since 2026-08-31 | unconfirmed - check the Play Console |
| B4 | 16 KB page size: `libgeebeeayy_core.so` (arm64, built 2026-10-08) and `libgeebeeayy_ra.so` have LOAD alignment 0x1000, not 0x4000 (`readelf -lW`). NDK r26/r27, no `max-page-size` flag anywhere | confirmed on local builds; recheck the CI artifact |
| B5 | No privacy policy; data-safety form must declare the RA username, activity and (unless M7 is fixed) file names | confirmed |

Also: `ROADMAP.md` Phase 6 says the app "sends nothing anywhere". That is false
since RetroAchievements and cover art.

## Where the lanes disagreed, and the call taken

| Item | Ratings | Taken | Why |
|---|---|---|---|
| Backup (M1) | storage High, network Medium, Play Medium | **Medium** | Google backups need the same account; the token is revocable; the fix is cheap and comes first anyway |
| Zip / ROM read (H2) | network High, Play Medium | **High** | Zips come from the internet; the crash is on launch and uncaught |
| RA threading (H3) | Kotlin lane High ("rc_client has no lock") | **High, reason changed** | `rc_client` does lock; the unguarded part is `g_gba` and the callbacks, which is the only memory-unsafety path found |
| `MANAGE_EXTERNAL_STORAGE` | storage Medium, Play High | **Medium as security, blocker for Play** | It is an over-grant, not an exploit, but Play will reject it |
| Token storage (M13) | storage Medium, network Low, Play Low | **Medium until M1 lands, then Low** | Exposure comes almost entirely through backup |
| Release signing | storage Info, Play High | **Blocker (B2)** | Not a vulnerability, but no release ships without it |

## Suggested fix order

1. Core restore validation: H1, M3, L1, L13, M14, CORE-8 in one change in
   `savestate.rs` / `apu/mod.rs`, plus L2 and M2. Failing tests first, built
   from the probe cases.
2. H3 threading in `RaEngine` and `ra_bridge.c`, with M8, M11, M12, L3, L8.
3. Backup rules (M1) and the rcheevos bump (M4).
4. Capped reads and fetches: H2, M6, M9, M10, L6.
5. B1 SAF rewrite, which also closes M5, L9, L12. The user's call on scope.
6. B2 to B5 before any Play submission.

## Second pass: the security history of other emulators

Sources: the local copies in `temp/emulators-research/` (mGBA `CHANGES`,
NanoBoyAdvance `CHANGELOG`, RetroArch `CHANGES.md` and `SECURITY.md`, plus the
code of all 11 projects), then their upstream history on the web: NVD, GitHub
advisories, issues and fix commits. 81 upstream items, raw list in
`temp/phase5/emu-upstream.md`; mapped onto our code in
`temp/phase5/emu-core.md` (42 items) and `temp/phase5/emu-frontend.md`
(40 items).

### What it added
- **M14** (new): a state from another game is accepted. mGBA and PPSSPP
  reject it. Probe `emu foreign`, `temp/phase5/emu-foreign.log`.
- **L13** (new): EEPROM chip state from a state can lock the chip. mGBA fixed
  the same class. `temp/phase5/emu-eeprom.log`.
- **M4** got stronger: rcheevos PR #554 reproduced on our pin with a host
  harness (`temp/phase5/rc-harness/`, `emu-rcheevos-progress.log`, exit 124).
- **B4** confirmed by `readelf` (Lemuroid hit the same Play rule).
- **H2** and **L5** proven by `android/app/src/test/java/com/geebeeayy/app/data/Phase5ProofTest.kt`
  (2 of 2 pass: a 40 MB zip entry is read whole; `..` passes the badge name check).
- **CORE-8** gained three debug-only overflow sites in the HLE BIOS
  (`bios.rs:406, 481, 538`), Info.

### What it ruled out (checked, not applicable)
- mGBA's HLE bugs: Div INT_MIN/-1, decompressors on bad source addresses,
  LZ77 back-references, Huffman bit length (we do not implement Huffman).
- Out-of-bounds ROM, VRAM, I/O, EEPROM and Flash accesses (mGBA, NBA): our bus
  masks or compares every access; probes `emu edges`, `emu pc`, `emu flash`.
- Invalid CPU mode indexing a register bank (NBA), FIFO overflow, DMA
  self-retrigger, rewind shrink, failed ROM load, old state versions.
- Length-prefixed blobs in states (mGBA extdata, gpsp, melonDS, Dolphin
  offset+size wrap): every length is checked against the remaining bytes.
- rcheevos fixes #268, #471, #477, #504, #516, #519, #521, #539, #541: all in
  our pin. #555 is missing but only touches Wii hashing, which we never call.
- RetroArch CVE-2021-28927 (Windows command injection from file names),
  CVE-2025-9136 (`filestream_vscanf` read), CVE-2025-0459 (DLL search path):
  no equivalent code in GeeBeeAyy. Lesson kept: never build a shell command
  from a file name.
- Cheats, ROM patches (IPS/UPS/BPS), 7z/rar, ELF loaders, movies, debugger,
  config files: features we do not have.

### What it could not check
PPSSPP, SkyEmu, gpsp and melonDS-android have no citable security history
(web search and the GitHub API returned nothing usable). The local Dolphin
and Azahar copies lack their state and archive parsers. See the pending-tests
file.
