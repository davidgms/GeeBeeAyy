# GeeBeeAyy! - iOS Roadmap

**Do not start any of this yet.**

This file is parked on purpose. The project is shipping Android first, and iOS
begins only when two things are true:

1. [`ROADMAP.md`](ROADMAP.md)'s Phase 4 is finished and the Android app is live
   on Google Play.
2. The repository owner has been asked, and has said yes.

The second condition is not a formality. iOS costs a Mac, an Apple Developer
account and a second frontend to keep at parity for as long as the project
lives. That is a decision, not a next task, and Android going quiet is not a
reason to drift into it.

Anyone - human or agent - who finds this file while looking for something to do
should go back to `ROADMAP.md` instead.

---

## Where iOS actually stands

`ios/` holds about 750 lines: SwiftUI views (Splash, ROM browser, Emulation,
Settings), a `GbaEngine.swift` wrapper and a `GeeBeeAyyTheme`. There is **no
Xcode project and no `Package.swift`**, so none of it has ever been compiled.
Treat every line of it as a draft that has not met a compiler, because that is
what it is.

The emulation core needs nothing from this file. `core/` is platform-agnostic
by design and `core/src/ffi.rs` already exposes the C ABI iOS would link
against; the Android JNI exports sit behind `#[cfg(target_os = "android")]` and
do not get in the way.

---

## Doing this without a Mac

Researched in [`temp/ios-without-a-mac.md`](temp/ios-without-a-mac.md). The
short of it:

- Step 1 below is free and needs no Apple hardware. Rust does not need Apple's
  SDK the way Swift does.
- Everything after it needs a Mac. About EUR 1 of hourly Scaleway time to
  create the Xcode project once, then GitHub Actions' free macOS minutes as a
  compile gate.
- The Simulator plays audio through the *host* Mac's stack, so it cannot
  reproduce iPhone CoreAudio latency. For an emulator whose whole timing model
  is "audio is the timing master", that makes it close to worthless for the
  bugs this project actually hits. Real-device testing is not optional here.

---

## The work, in order

Owned by `swift-expert`, with `mobile-app-developer` on the build.

- [ ] Create the Xcode project (or `Package.swift`) - there is currently
      neither.
- [ ] Build the core as a static library for `aarch64-apple-ios` and the
      simulator target.
- [ ] Verify the bridging header against the real C ABI in `core/src/ffi.rs`.
      The header is hand-written and has never been checked against the
      functions it declares.
- [ ] Audio via `AVAudioEngine`, mirroring Android's blocking-write approach so
      the audio device stays the timing master. Do not reintroduce
      timer-paced frame advance - `CLAUDE.md` explains why.
- [ ] Touch controls, MFi controllers, save states, iCloud sync.
- [ ] Storage: the document picker and security-scoped URLs, which is a
      different model from Android's SAF and needs its own design rather than
      a translation of the Kotlin one.
- [ ] TestFlight, then App Store.

**Exit criterion:** a stranger can install GeeBeeAyy! from the App Store, point
it at their own legally dumped ROM, and play it - with the same games working
the same way they do on Android.

---

## Parity is a rule, not a goal

Whenever this starts, the two frontends render the same core. A bug that shows
on one is a bug in `core/` until proven otherwise, and the fix belongs in Rust
where both platforms get it. Neither frontend gets emulation logic of its own.
