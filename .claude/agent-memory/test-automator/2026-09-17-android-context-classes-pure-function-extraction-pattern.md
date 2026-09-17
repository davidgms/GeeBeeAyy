---
name: android-context-classes-pure-function-extraction-pattern
description: how to unit-test SharedPreferences/Vibrator-backed Android classes without Robolectric - extract the decision into a companion function
metadata:
  type: project
---

`android/app/src/test/**` is plain JVM JUnit - no Robolectric, no mockito
(see `CLAUDE.md`). Two classes needed under test on 2026-09-17 both wrap a
real Android object that throws "not mocked" outside an instrumented test:
`ControlLayoutStore` (`android/app/src/main/java/com/geebeeayy/app/data/ControlLayoutStore.kt`,
wraps `Context.getSharedPreferences`) and `Haptics`
(`android/app/src/main/java/com/geebeeayy/app/ui/Haptics.kt`, wraps
`Vibrator`/`SystemClock`).

The pattern used both times, and worth repeating for the next one of these:
pull the actual decision - the part a regression would silently break - out
into a `companion object` function that takes plain values and, where it
needs another method of the class, a closure, instead of touching `prefs`/
`vibrator`/`SystemClock` itself:

- `ControlLayoutStore.Companion.resolveLayoutForGame(chosen, layoutIds,
  defaultLayoutId, landscape, orientationOf)` - the fallback chain behind
  `getLayoutForGame`, tested in
  `android/app/src/test/java/com/geebeeayy/app/data/ControlLayoutStoreTest.kt`
  with an in-memory `Map<String, LayoutOrientation>` standing in for the
  store.
- `Haptics.Companion.shouldBuzz(nowMs, lastBuzzMs)` - the debounce decision
  behind `press()`, tested in
  `android/app/src/test/java/com/geebeeayy/app/ui/HapticsTest.kt`.

Both are small, behaviour-preserving extractions (the original method now
just calls the companion function with its real inputs) and were flagged as
production changes in the handback report per the task's constraint on
touching production code to make it testable. This is the smallest version
of the extraction the task brief itself suggested - do this rather than
reaching for Robolectric, which is explicitly banned.

See also [[core-ppu-tests-already-thorough-check-before-adding]].
