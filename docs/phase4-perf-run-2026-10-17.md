# Phase 4 performance run - 2026-10-17

Mi 10T Pro, Android 12, MIUI. Debug build of `master` at `ee1fac5`. The
240p Test Suite's **Hill zone scroll test** ran the whole time: a full-screen
parallax scroll, so the PPU does real work every frame. The phone stayed on
USB, so it was charging throughout.

Scripts: [`perf/sample.sh`](perf/sample.sh) (one sample a minute) and
[`perf/cpu.sh`](perf/cpu.sh) (10-sample CPU average). Raw data:
[`perf/2026-10-17-session.csv`](perf/2026-10-17-session.csv).

## 30 minutes of play

| Measure | Result |
|---|---|
| Crash, ANR, process restart | None - one pid for the whole run |
| Frames presented by the game's `SurfaceView` | 49.1 to 49.9 a second, every minute |
| Panel refresh rate | 50 Hz, every minute (MIUI, see the 2026-10-01 run) |
| Memory (PSS) | 179 to 191 MB, no growth |
| CPU | 68% to 107% of one core, about 89% on average |
| Thermal status | 0 (none) for the whole run |
| Audio underruns | One burst of 347 frames (about 7 ms at 48 kHz) between minutes 17 and 18; none otherwise. No watchdog rebuild in logcat. |

`gfxinfo` reports about 44 renders a second. Those are the game surface's
own `lockHardwareCanvas` draws in `ui/GameSurface.kt`, not the Compose tree:
`framestats` shows the window itself drew only when the sampler poked it.

## Start-up

| Measure | Result |
|---|---|
| `am start -W` TotalTime, cold | 861 and 760 ms on the first two runs, then 326 to 339 ms |
| Launch to the library being on screen | 2.7 to 3.4 s (the splash follows the real load; includes about 0.5 s of `uiautomator` polling) |
| Tap on a ROM to the controls being on screen | 3.2 s (same polling overhead) |

## Cost of each optional feature

10-sample CPU average, same scene.

| Setting | CPU | Game fps | New underruns |
|---|---|---|---|
| Baseline (frame blending on, filter None) | 89% | 49.9 | 0 |
| Frame blending off | 93% | 49.9 | 0 |
| Fast forward on | 84% to 87% | 49.9 | 0 |
| 2xSaI filter | 122% | 49.9 | 0 |

Frame blending and fast forward are inside the noise. 2xSaI costs about a
third of a core more and still holds the frame rate. Achievements were not
measured: they need a RetroAchievements login on the phone.

## Not measured

- **Battery drain.** The phone charged over USB the whole time, and
  `dumpsys battery unplug` freezes the battery service's readings rather
  than stopping the charge, so the level and temperature columns are flat
  and meaningless. A drain figure needs a run with the cable out.
- **Achievements' cost**, as above.
