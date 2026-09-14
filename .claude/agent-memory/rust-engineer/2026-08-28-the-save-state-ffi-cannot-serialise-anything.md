### 2026-08-28 - The save-state FFI cannot serialise anything
- **Context**: same consultation.
- **Finding**: `geebeeayy_save_state_create` returns an opaque `*mut c_void`
  (`core/src/ffi.rs:212`) and there is no function returning its bytes or
  length. `SaveState::save_to_file`/`load_from_file` (`savestate.rs:213,218`)
  are `std::fs` and are only reachable from `core/src/main.rs`. So
  `GbaEngine.saveStateCreate()` hands Kotlin a `Long` it can only pass back or
  free - a state cannot survive the process. Save states are not "not wired to
  a UI", they are not exportable.
- **Application**: any save-state persistence task needs a bytes-out and a
  bytes-in entry point before the Kotlin side can be written. Do not add a
  path-taking FFI function instead: Android scoped storage hands out FDs and
  content URIs, not paths the core can `fs::write` to.
