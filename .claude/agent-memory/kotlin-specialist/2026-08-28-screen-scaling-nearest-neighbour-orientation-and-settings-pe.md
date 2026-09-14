### 2026-08-28 - Screen scaling, nearest-neighbour, orientation and settings persistence

- **Context**: ROADMAP Phase 2's display items - `GbaScreen` stretched the
  240x160 frame to fill its box with implicit bilinear sampling, and
  `android:screenOrientation="portrait"` in `AndroidManifest.xml` hard-locked
  the app regardless of anything Compose did.
- **Finding (the aspect-ratio lock was the actual blocker)**: the old
  `EmulationScreen.kt` wrapped `GbaScreen` in a Box with
  `.aspectRatio(240f / 160f)`. That modifier forces the *container* to
  always be exactly 3:2, which makes "Fit vs Integer vs Stretch" a
  distinction without a difference - there is never a mismatch between
  container and source aspect to letterbox against. Scaling modes only mean
  anything once the container is free to be any shape (portrait: whatever
  vertical space is left after the top bar and controls via `.weight(1f)` in
  a `Column`; landscape: whatever horizontal space is left between the D-pad
  and the action buttons via `.weight(1f)` in a `Row`), and `GbaScreen`'s
  `Canvas` computes the actual drawn rect and centers it - the surrounding
  black background then *is* the letterbox, no separate bars to draw.
- **Finding (nearest-neighbour is a `drawImage` parameter, not a Bitmap
  setting)**: `DrawScope.drawImage(..., filterQuality: FilterQuality =
  DrawScope.DefaultFilterQuality)` exists and defaults to bilinear-ish
  filtering; passing `FilterQuality.None` is the entire fix, no
  `Paint`/`BitmapShader` plumbing needed. Confirmed by reading the existing
  call site (this codebase already used the `dstOffset`/`dstSize` overload
  of `drawImage` before this change, just without `filterQuality`), not
  independently verified against a running app - unverified, no Android
  toolchain here (see repo-wide note on that).
- **Finding (`android:configChanges` for orientation was already in the
  manifest, just inert)**: `MainActivity`'s manifest entry already carried
  `android:configChanges="orientation|screenSize|screenLayout|keyboardHidden"`
  *alongside* `android:screenOrientation="portrait"`. The `configChanges`
  half only matters once orientation can actually change, so it had done
  nothing since the app shipped locked to portrait. Removing the
  `screenOrientation` line activates a mechanism that was already half-built:
  Compose's `LocalConfiguration` updates from `AndroidComposeView`'s own
  `onConfigurationChanged` override independent of whether the Activity opts
  out of recreation, so declaring `configChanges` for orientation avoids a
  full Activity (and `ViewModelStore`, GL context, JNI handle) teardown/
  rebuild on rotation while Compose still recomposes correctly. This is
  documented Compose/Activity interop behaviour I did not independently test
  here - flagged unverified in `ROADMAP.md` for exactly that reason.
- **Finding (`requestedOrientation` as a *default-preserving* replacement for
  a manifest lock)**: rather than flip the whole app to free rotation (which
  would be a behaviour change every existing install experiences
  unprompted), the lock moved to `MainActivity.onCreate()` calling
  `requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_PORTRAIT`, driven
  by a `DisplaySettings.getForcePortrait()` flag that **defaults to `true`**.
  The manifest is honestly unlocked (satisfies "nothing in the manifest
  should lock the orientation" literally) while the observed default
  behaviour is unchanged until a player opts out from Settings. Applying it
  again from `SettingsScreen`'s toggle works without an Activity recreate
  for the same `configChanges` reason above.
- **Application**: `DisplaySettings.kt` (new, in `data/`) is a second small
  `SharedPreferences` wrapper next to `RomFolderManager` - this project has
  no DataStore dependency, so don't add one for a handful of enum/bool
  values; `getSharedPreferences(name, MODE_PRIVATE)` + `.edit().apply()` is
  the established pattern here and should stay it for anything else this
  small. `ScaleMode` (FIT/INTEGER/STRETCH) lives in that same file since it
  is persisted data, not UI - `EmulationScreen.kt` and `SettingsScreen.kt`
  both just import it.
