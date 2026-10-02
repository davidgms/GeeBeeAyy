# Emulators research - conclusions (Phase 4.5)

Written 2026-10-02 from eleven per-emulator notes (mGBA, gpSP, NanoBoyAdvance, SkyEmu, melonDS, melonDS Android, Azahar, Dolphin, PPSSPP, RetroArch, Lemuroid). The notes live in each developer's `temp/emulators-research/` and are not committed; every link this summary depends on is quoted here. Claims
marked **(verified)** were re-read in the source by the main session; the
verification log is at the bottom of `ROADMAP.md`. Everything else is as the
research agents reported it, with the link in the emulator's notes.

## 1. The MIUI 50 Hz problem

**No emulator in this set has a MIUI or Xiaomi refresh-rate workaround** -
not in code, not in issues, not in PRs (all eleven folders, "Not found").

What the others do instead:

| Approach | Who | Result |
|---|---|---|
| `Surface.setFrameRate(60)` on every new surface | PPSSPP, PR #18529 (verified) | Fixed a 90 Hz panel dropping to 50 Hz for one reporter; maintainer says it did nothing useful on 120 Hz |
| `preferredDisplayModeId` = 60 Hz during play, max in menus, plus `appCategory="game"` | Azahar, PR #1492 (verified) | Code comment: Android 15 runs apps categorised as games at 60 Hz by default. No evidence it beats MIUI |
| Do not ask; measure the real panel period and present the newest frame each vsync | melonDS Android, FrameQueue (verified) | A "steady 60 fps but stutters" class of bug fixed (#1537) |
| Ask for nothing | mGBA, gpSP, NBA, SkyEmu, Dolphin, RetroArch (until 2026-08), Lemuroid | - |

GeeBeeAyy already sends both requests PPSSPP and Azahar send, on a
SurfaceView that presents real frames (PR #47, #48). MIUI's cap sits above
them (`docs/research/miui-smart-refresh.html`).

**Decision:** stop chasing a code fix for MIUI. Keep the standard requests
(they are what works elsewhere). Two cheap things are still worth doing:

- Add `android:appCategory="game"` (and `isGame`, as Dolphin does) to the
  manifest. Free on Android 15+, and MIUI may treat category-game apps
  differently. One device test before and after.
- Re-apply the display-mode request in `onResume`, not only when the screen
  is composed: RetroArch found the mode is lost when the window is torn
  down (commit 7c65f1c, not verified on a device by them).

The one-time "press Home and come back" tip (option A in the MIUI research)
stays the player-facing answer.

## 2. Audio as the master clock: keep it

Every emulator picks a clock. Two families:

- **Wall clock or vsync, audio stretched to follow:** Dolphin, PPSSPP,
  melonDS, melonDS Android, Azahar, SkyEmu, NanoBoyAdvance, RetroArch.
- **Audio clock:** mGBA (maintainer: vsync cannot be the clock on panels
  above 60 Hz, mgba#2199), gpSP via its frontend, GeeBeeAyy.

Evidence for staying audio-clocked:

- NanoBoyAdvance's `sleep_until` timer produced years of stutter and pop
  reports (Codeberg #394, #134, #231).
- RetroArch's vsync pacing breaks on a TV stuck at 50 Hz: slow motion or 10
  dropped frames a second, open since 2019 (RetroArch#8598).
- Azahar #1193: FIFO vsync on a sub-60 Hz panel caps emulation speed. On
  MIUI's 50 Hz that would be a 17% slowdown. Our loop never waits on the
  display, so it cannot happen to us - **this must stay true**: the
  SurfaceView draw must never block the emulation thread.
- Wall-clock emulators all need an audio resampler with a feedback controller
  (RetroArch +-0.5%, LibretroDroid PI, PPSSPP +-600 Hz). An audio clock needs
  none.

## 3. Actions, ranked

| # | Action | Why | Cost | Evidence |
|---|---|---|---|---|
| 1 | Present the newest frame per vsync, never block emulation on the draw | Already true after #48; add a test or assertion that the renderer never holds `engineLock` | Small | melonds-android, azahar #1193 |
| 2 | `appCategory="game"` + `isGame="true"`, test on the phone | Android 15 game default 60 Hz; MIUI may react | Tiny | azahar (verified), dolphin |
| 3 | Re-apply the display mode in `onResume` | Mode lost on window teardown | Tiny | retroarch |
| 4 | One-time MIUI tip when a game runs below 60 Hz | Only player-facing fix that works | Small | docs/research/miui-smart-refresh.html |
| 5 | Audio-driven render frameskip on underrun | Keeps sound clean on a hot, throttled phone; game speed unchanged | Small | gpsp (verified) |
| 6 | Audio watchdog: restart the AudioTrack if no audio for ~100 frames | Cheap insurance against a dead track | Small | skyemu |
| 7 | Ask Android for the output buffer size and sample rate (`PROPERTY_OUTPUT_FRAMES_PER_BUFFER`, `PROPERTY_OUTPUT_SAMPLE_RATE`) | We use `getMinBufferSize` only | Small | dolphin PR #12836, ppsspp |
| 8 | Performance Hint API on the emulation thread (API 31+) | Lets the scheduler keep clocks up without sustained mode | Small | melonds-android `bded894` |
| 9 | Poll input more than once per frame | Lower input latency at no emulation cost | Medium (core FFI) | nanoboyadvance |
| 10 | Accuracy suites as a checklist: AGS, mGBA suite, ARMWrestler, gba-suite, FuzzARM | We run jsmolka's gba-tests only | Medium | nanoboyadvance |

Considered and not recommended:

- **Run the game at exactly 60.0 fps and stretch audio 0.46%** (mGBA, melonDS).
  Only helps on a true 60 Hz panel, and it is the opposite of our clock.
- **Sustained performance mode.** Dolphin tried twice and never merged; a
  MIUI user said only Game Turbo helped.
- **Oboe/AAudio.** melonDS Android and LibretroDroid use Oboe, but both carry
  open crackle bugs; PPSSPP rejected AAudio as buggy (#9475). Our blocking
  AudioTrack measured zero underruns in Phase 4. Not worth a new dependency.
- **Emulation inside `GLSurfaceView.onDrawFrame`** (Lemuroid): ties speed to
  the view lifecycle, which caused speed bugs (LibretroDroid #36, #46).

## 4. Where GeeBeeAyy already matches good practice

- Battery saves debounced before writing (we use 2 s; mGBA 15 frames, SkyEmu 10).
- Save-state round-trip test exists (`core/tests/saves.rs`
  `save_state_round_trip_reproduces_the_machine`); NanoBoyAdvance's open
  save-state corruption bug (#429) is the kind it catches.
- RetroAchievements progress travels with save states (sidecar); SkyEmu does
  the same inside the state.
- Storage access stays in Kotlin, the core sees bytes (melonDS Android
  passes file descriptors for the same reason).
- Pause before the surface goes away and redraw on `surfaceChanged`
  (Dolphin's pattern; ours since #48).
