### 2026-08-28 - Wiring the touch overlay: key-state ownership and the frame-buffer alloc

- **Context**: ROADMAP 0.1's last open item - `ui/screens/EmulationScreen.kt`'s
  D-pad/A/B were decorative (`onClick = { /* Handle press */ }`), plus the
  known `GbaScreen` per-frame `Bitmap.createBitmap` allocation.
- **Finding (JNI contract)**: `core/src/ffi.rs:368-379` exports
  `Java_com_geebeeayy_app_engine_GbaEngine_nativeSetKeys(handle: jlong, keys: jint)`,
  a static method taking the bitmask directly (no boxing). GBATEK bit order:
  A=0, B=1, Select=2, Start=3, Right=4, Left=5, Up=6, Down=7, R=8, L=9. Set
  bit = pressed; the core inverts to active-low KEYINPUT itself.
- **Finding (recomposition trap)**: the old `GbaScreen` used
  `remember(frameBuffer) { ...IntArray... ; Bitmap.createBitmap(...) }`.
  Since `EmulationViewModel` publishes a fresh `.copyOf()` `ByteArray` every
  frame (`EmulationViewModel.kt:100`), `ByteArray` has no structural
  `equals`, so `remember(frameBuffer)` never short-circuits - it reruns
  (and reallocates ~150 KB) on every single frame. Fix: `remember { }` with
  no key for the `Bitmap` and the `IntArray` (allocate once), then mutate
  both in place (`bitmap.setPixels(...)`) directly in the composable body on
  every recomposition - no `LaunchedEffect` needed since the conversion is
  synchronous CPU work and doing it inline avoids a one-frame lag between
  the state update and the redraw.
- **Finding (per-button pointerInput touch tracking)**: the existing D-pad/
  action buttons each already had an isolated `pointerInput(Unit) { awaitPointerEventScope { while(true) awaitPointerEvent() } }`
  loop per button, tracking `isPressed` locally for tint only. This is
  naturally multi-touch-safe: two fingers landing on two adjacent buttons
  (e.g. Up + Right) fire two independent gesture loops and their bits OR
  together - so diagonals work with two fingers, just not one, since the
  cross-shaped D-pad layout has no shared corner hit zone. Reported this
  rather than silently treating diagonals as done.
- **Application**: key-state ownership decision - I put a plain (non-Flow)
  `Int` bitmask field + `setKey(key: Int, pressed: Boolean)` in
  `EmulationViewModel` rather than in composable `remember` state, because
  (a) the constraint forbids calling the engine from a composable's
  recomposition path, and pushing aggregation to the ViewModel keeps the one
  `engine.setKeys()` call site there, and (b) composable `remember` does not
  survive rotation, so a config change would silently drop held keys.
  Composables only report discrete press/release transitions upward via a
  threaded `onKeyChange: (Int, Boolean) -> Unit` callback (called from the
  gesture loop, not from the composable's function body). Note this
  reintroduces the same cross-thread JNI pattern already used elsewhere in
  this codebase (UI-thread calls racing `Dispatchers.Default`-thread calls
  into the same `GbaEngine`/`GbaHandle`) - not a new risk I introduced, but
  worth flagging to `rust-engineer` if `bus.set_keys` in `core/` ever turns
  out not to be a plain word write.
