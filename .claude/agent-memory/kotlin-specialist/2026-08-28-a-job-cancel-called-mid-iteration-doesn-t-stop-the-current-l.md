### 2026-08-28 - A `Job.cancel()` called mid-iteration doesn't stop the current loop body

- **Context**: `EmulationViewModel`'s save-state load path calls
  `stopEmulation()` (which calls `emulationJob?.cancel()`) from *inside* the
  emulation loop's own coroutine body, when a load is rejected as corrupt.
- **Finding**: `Job.cancel()` flips `isActive` to `false` synchronously, but
  a `while (isActive) { ... }` loop only re-reads that flag at the top of the
  loop. Code appearing after the `cancel()` call within the same iteration
  keeps running to the end of that iteration before the loop notices -
  cancellation is cooperative at suspension points, not at arbitrary
  synchronous code. Concretely: self-cancelling from inside the loop body and
  then falling through to `engine.runFrame()` runs one more frame on a
  machine the code had just declared unreliable, and calls `audio.write()` on
  an `AudioTrack` that the same `stopEmulation()` call had just paused and
  flushed.
- **Application**: any code path that can self-cancel a loop's own job from
  inside that loop's body needs an explicit `if (!isActive) continue` (or
  equivalent early-exit) placed immediately after the cancelling call, not an
  assumption that cancellation takes effect immediately.
