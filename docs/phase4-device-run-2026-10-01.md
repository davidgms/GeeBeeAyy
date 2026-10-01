# Phase 4 device run - 2026-10-01

Mi 10T Pro, Android 12 (API 31), MIUI 14. Debug build of `master` at
`e6be7b9` plus the display-mode change. A debug build is a worst case:
the installed app is debug-signed, and a release build would have meant
uninstalling it and losing the player's data.

Sampler: `temp/perf-sample.sh` (read-only: meminfo, top, battery,
thermalservice, gfxinfo). Not committed; it lives in each developer's `temp/`.

## Performance, 10 minutes of Mario Tennis Advance

| Measure | Result |
|---|---|
| Emulation | 60 fps for the whole run (the in-game counter) |
| Memory (PSS) | 218-224 MB, flat - no growth over the run |
| CPU | 113-153% of one core, about 135% typical |
| Battery temperature | 28.2 to 32.2 C, still rising slowly at the end |
| Thermal status | 0 (none) throughout |
| Janky frames | 0.06% at the end |
| **Frames reaching the screen** | **about 50 a second, not 60** |

### The 50 Hz finding

MIUI's smart refresh (`ro.vendor.smart_dfps.enable=true`) holds the panel at
50 Hz while a game runs, with the user setting at 144. The emulator makes 60
frames a second, so about one frame in six is never shown, and motion
judders. Jank counters do not see it: every frame that is drawn is on time.

Tried, in order:

1. `WindowManager.LayoutParams.preferredDisplayModeId` for the 60 Hz mode.
   The request reaches `DisplayModeDirector` (`PRIORITY_APP_REQUEST_BASE_MODE_REFRESH_RATE`
   votes 60) and MIUI still picks mode 4, 50 Hz.
2. `preferredRefreshRate = 60` as well. No change.
3. `Surface.setFrameRate(59.73, FIXED_SOURCE)` on a 1px `SurfaceView`.
   The layer never draws a buffer, has an empty visible region, and the
   compositor ignores its vote. Removed.

Kept: (1) and (2), in `ui/DisplayRate.kt` with `DisplayRateTest`. They are the
standard request and should work on stock Android and on vendors that honour
it. What would work on MIUI: the game drawn into its own `SurfaceView`, voting
with `Surface.setFrameRate` on a surface that actually presents frames. That is
the same change as the open "frame conversion on the UI thread" finding.

## Stress, no crash, no ANR, no skipped-frame warning in logcat

| Step | Result |
|---|---|
| 15 save + 15 load state taps, back to back (240p test ROM) | No crash, 60 fps after |
| 10 background/foreground cycles | Same process throughout, 0% CPU while in the background, resumes at 60 fps |
| 6 rounds of switching between two ROMs | PSS 181 MB at start, 190-194 MB after round 2, then flat |
| 400 taps across A, B and the d-pad, plus 10 fast-forward toggles and 10 rewind holds | No crash, 61 fps after |

## Not covered this run

- Rotation mid-action, low storage, a dropped network during a fetch, and the
  layout editor at its limits.
- A session longer than 10 minutes, cold-start and ROM-load times, and the cost
  of each optional feature (blending, 2xSaI, fast-forward, achievements).

## A mistake in the run

The top bar's save button saves to slot 0 at once; it does not open a picker.
One tap on it during the run overwrote the player's own Mario Tennis slot 0.
After that, the app's `files/states` and `files/saves` were backed up to
`temp/phone-backup-*/` and every save-state test ran on the 240p test ROM.
Device runs must back up first and never press save on a player's game.
