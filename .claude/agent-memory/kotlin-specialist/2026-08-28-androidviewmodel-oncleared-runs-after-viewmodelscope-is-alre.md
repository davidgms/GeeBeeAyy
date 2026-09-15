### 2026-08-28 - `AndroidViewModel.onCleared()` runs after `viewModelScope` is already cancelled

- **Context**: wiring the battery-save flush-on-teardown requirement in
  `EmulationViewModel.onCleared()`.
- **Finding**: `ViewModel.clear()` (the internal method `onCleared()` is
  called from) closes every registered `Closeable` - which includes the
  internal one that cancels `viewModelScope`'s `Job` - **before** it calls
  `onCleared()`. So `viewModelScope.launch { ... }` inside `onCleared()`
  creates a `Job` that is already cancelled and never runs its body. This is
  a well-known but easy-to-miss Android gotcha, not something specific to
  this codebase.
- **Application**: any final-flush-on-teardown logic needed in `onCleared()`
  has to either (a) be plain, non-suspending code called directly (fine for
  something bounded and small, like a single save file write under ~128KB -
  `EmulationViewModel.flushSaveNow()` does exactly this), or (b) live on a
  separate `CoroutineScope` the class owns and manages independently of
  `viewModelScope`. Don't reach for `runBlocking` as a first move; check
  whether the work is already synchronous first.
