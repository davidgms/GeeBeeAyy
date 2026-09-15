### 2026-08-28 - First CI (`.github/workflows/ci.yml`), and two facts that were wrong until now

- **Context**: writing the project's first GitHub Actions workflow (no
  `.github/` existed before). Needed to know what actually builds the Android
  side, since the developer has no local SDK/NDK and no phone.
- **Finding 1 - the committed `.so` claim was false.** CLAUDE.md and
  `ROADMAP.md` (item 0.5) both said `android/app/src/main/jniLibs/*/libgeebeeayy_core.so`
  was "committed" and merely stale. `git ls-files android/app/src/main/jniLibs`
  returns nothing - `.gitignore`'s `*.so` rule (under "Build artifacts")
  matches them and they were never force-added. A fresh clone has no native
  library at all (git does not track the now-empty ABI directories either),
  so a from-scratch Gradle build produces an APK that `UnsatisfiedLinkError`s
  on the first JNI call. Corrected in CLAUDE.md, ROADMAP.md 0.5, this file's
  "Your lane" section, and `.claude/memory.md`.
- **Finding 2 - there is no Gradle wrapper committed either.** No `gradlew`,
  no `gradle/wrapper/gradle-wrapper.jar`, no `gradle-wrapper.properties` -
  only an empty `android/gradle/wrapper/` directory. `./build.sh android`
  calls `./gradlew assembleDebug`, which has never been able to work from a
  clean checkout. Worked around in CI with `gradle/actions/setup-gradle@v3`
  (`gradle-version: "8.7"`, matching AGP 8.5.1's minimum), which provisions
  Gradle directly so the workflow can run `gradle assembleDebug` without a
  wrapper. A real wrapper should probably still be committed at some point
  for local reproducibility - out of scope for the CI task, flagged for the
  developer.
- **Finding 3 - `core/.cargo/config.toml` and the repo-root `.cargo/config.toml`
  hardcode an absolute NDK linker path** (`/home/david/android-sdk/ndk/27.0.12077973/...`)
  from one developer's WSL machine. It does not exist on a CI runner. Cargo
  env vars are documented to take precedence over `config.toml`, and
  `cargo-ndk` sets its own `CARGO_TARGET_*_LINKER` per build, so this is
  probably harmless - but since that precedence rule could not be verified in
  this sandbox (no way to run `cargo ndk` here), the CI workflow deletes both
  files before the Android build step rather than relying on it.
- **Application**: any future FFI/mobile-plumbing task should re-verify these
  three assumptions once CI has actually run once - I could not execute
  `cargo ndk`, `gradle`, `sdkmanager`, or the emulator runner locally to
  confirm any of this works end to end.
