# Phase 5 - tests still pending

Checks from the Phase 5 audit ([`security-audit-phase5.md`](security-audit-phase5.md))
that could not run on 2026-10-08, and exactly what each one needs. Tick an item
only with a log, a test or a command output to point at.

Every test program runs under a 4 GB memory cap:
`prlimit --as=4000000000 timeout 600 <cmd>` for native programs, and
`systemd-run --user --scope -q -p MemoryMax=4G -p MemorySwapMax=0 timeout 900 <cmd>`
for Gradle and other JVM programs, which cannot start under `prlimit --as`.

## Needs the phone (`7d4c9ae1`)

Back up `files/states` and `files/saves` first, and use 240p, never a player's
game.

- [ ] **H3 - RA race on a device.** Deliver an RA server response while
  `nativeLoadRom` runs, and a second ROM loaded after the first (upstream
  RetroArch hit a crash on the second game). Needs a scripted login-then-load
  sequence; best with a ThreadSanitizer build of the core and `ra_bridge.c`.
- [ ] **M4 / EMU-CORE-40 on the phone.** Plant the 64-byte sidecar from
  `temp/phase5/rc-harness/` next to a 240p state (needs root, or `run-as` on
  the debug build) and load it while signed in to RA. Expected: app stops
  responding. Confirms the ANR, not only the host hang.
- [ ] **M1 - what backup really carries.** `adb shell bmgr backupnow
  com.geebeeayy.app`, then inspect the transport, or a device-to-device
  transfer. Android 12+ uses `data_extraction_rules`, 11 and lower
  `fullBackupContent`, so test one of each when the rules land.
- [ ] **Foreign VIEW intent (L10).** A small sender app or `adb shell am start
  -a android.intent.action.VIEW -d content://... -t application/octet-stream`
  to confirm the app ignores the Uri. Moot if the filter is deleted.
- [ ] **Expired or revoked RA token.** A real account: revoke the token on
  retroachievements.org, then launch a game. Expected: token cleared and a
  login prompt, not a loop.
- [ ] **Process death during a save.** `adb shell am kill com.geebeeayy.app`
  while a state is written; confirm the atomic write leaves the old file.
- [ ] **B1 - SAF behaviour** (only after the SAF rewrite): revoked grant,
  moved or deleted folder, Android 11, 13 and 14.

## Needs a tool we do not have installed

- [ ] **#554 second half - read past a short chunk.** A runtime loaded with a
  real achievement set, under AddressSanitizer or Valgrind. Valgrind is not
  installed; AddressSanitizer reserves terabytes of address space, so it needs
  the `systemd-run` cap, not `prlimit --as`.
- [ ] **B4 on the shipped artifact.** `readelf -lW` on both `.so` files from
  the CI `android` job artifact, or `zipalign -c -P 16 -v 4` on a release
  APK. Local builds already fail (0x1000).

## Needs a Play Console account

- [ ] **B3 - current targetSdk requirement** for new apps (believed 36 since
  2026-08-31).
- [ ] **B5 - data-safety form** answers against the table in
  `temp/phase5/play-review.md` section 2.

## Needs more upstream reading

- [ ] **PPSSPP, SkyEmu, gpsp, melonDS-android security history.** The web pass
  found nothing citable and the GitHub API rate-limited (403). Retry with an
  authenticated `gh api`.
- [ ] **Dolphin and Azahar state and archive parsers.** The local copies in
  `temp/emulators-research/` are sparse checkouts without those files. Read
  the 7 Dolphin and 1 Azahar GitHub advisories (2026) in full against
  `core/src/savestate.rs`.
- [ ] **Lemuroid issue #378** (zipped ROM discrepancy): read it and check
  `RomBytes.kt` entry choice.
