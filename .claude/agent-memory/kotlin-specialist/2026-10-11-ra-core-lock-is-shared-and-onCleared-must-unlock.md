---
name: ra-core-lock-is-shared-and-onCleared-must-unlock
description: RaEngine.coreLock is the VM's engineLock; anything that lock()s it without withLock must unlock
metadata:
  type: project
---

`EmulationViewModel.engineLock` is `RaEngine.coreLock` (EmulationViewModel.kt:304). It is shared by every VM, so
`onCleared` (~1061-1080) takes it with `lock()` to prove the loop left native code and MUST `unlock()` after
`engine.destroy()`, or the next game's load deadlocks. RaEngine's `check(coreLock.isLocked)` only proves
*someone* holds it, not the caller; review by reading, not by the check.

The VM loop's `doFrame` was already inside the per-frame `engineLock.withLock` block (~610-690); no extra wrap needed.
RaEngine unit tests (JVM) fake `RaNative`; `Context` is a `ContextWrapper(null)`. Gradle unit tests: ~30 s.
