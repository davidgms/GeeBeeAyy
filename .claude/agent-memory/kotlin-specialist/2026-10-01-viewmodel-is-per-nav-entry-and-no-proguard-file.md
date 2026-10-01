---
name: viewmodel-is-per-nav-entry-and-no-proguard-file
description: EmulationViewModel lives per "emulation/{filePath}" back-stack entry (so one VM sees one path); proguard-rules.pro does not exist despite isMinifyEnabled
metadata:
  type: project
---

- `MainActivity.kt:~196` calls `viewModel()` inside the `composable("emulation/{filePath}")` block, so the
  VM store owner is the NavBackStackEntry. One VM only ever sees one ROM path; the "different path"
  branches in `loadRomFromPath` (RaEngine.unloadGame, stop-then-reload) are effectively dead, and system
  Back clears the VM (`onCleared` does the flush/destroy, on the main thread).
- `android/app/build.gradle.kts:62-67` enables R8 for release and names `proguard-rules.pro`, which is not in
  the tree (never tracked). `RaEngine`'s `@JvmStatic private` callbacks (`onServerRequest`, `onEvent`,
  `onLogin`, `onGameLoaded`) are looked up by name from `cpp/ra_bridge.c:309-320`; only `native` methods
  are kept by the default optimize file.
- GbaEngine's 18 externals match `ffi.rs` JNI exports exactly (diffed 2026-10-01); RaEngine 18/18 in ra_bridge.c.
- rcheevos `rc_client` has its own internal mutex (`rc_client.c:184`), so Kotlin-side RA calls off the
  emulation thread are lower risk than they look; the dangling emulator handle after VM clear is the real one.
