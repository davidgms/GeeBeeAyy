### 2026-08-28 - `android/app/src/main/cpp/jni_bridge.c` is dead code describing a removed API

- **Context**: mid-flight, the coordinator removed the opaque-handle
  save-state JNI functions (`nativeSaveStateCreate`/`nativeLoadState`/
  `nativeSaveStateDestroy`) from `core/src/ffi.rs` and replaced them with a
  byte-based pair. Grepping the Android tree for the old names to update every
  caller surfaced a second, independent JNI implementation.
- **Finding**: `android/app/src/main/cpp/jni_bridge.c` is a hand-written C
  file that `extern`-declares the C ABI (`geebeeayy_*`) and re-implements the
  same `Java_com_geebeeayy_app_engine_GbaEngine_*` JNI exports that
  `core/src/ffi.rs` already provides directly via the `jni` crate. It is not
  wired into the build at all - `android/app/build.gradle.kts` has no
  `externalNativeBuild`/CMake block referencing it, so it never compiles into
  the APK. It is also stale: it still calls the now-removed
  `geebeeayy_save_state_create`/`geebeeayy_load_state`/
  `geebeeayy_save_state_destroy`, and it is missing `nativeSetKeys`,
  `nativeSaveTakeDirty`, `nativeSaveRead`, `nativeSaveWrite`,
  `nativeStateRead` and `nativeStateWrite` entirely - none of the FFI
  additions from the last two save-data passes ever touched it, because
  nothing builds it.
- **Application**: don't update this file when the JNI surface changes - it
  is inert. It is misleading enough (a second "source of truth" for the JNI
  signatures that silently drifts) that it is worth `mobile-developer`
  deciding whether to delete it outright or wire it in; left as-is here since
  touching build files was out of scope for this task and deletion deserves
  its own diff. Flagged in `.claude/memory.md` too.
