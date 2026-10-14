# Play Console: Data safety answers

For audit item B5 (`docs/security-audit-phase5.md`). Definition used, from
Play (support.google.com/googleplay/android-developer/answer/10787469):
"collect" means transmitting data from the app off the user's device, by the
app or by a library, to the developer or a third party. The developer has no
server and no analytics, so every "yes" below is a third-party transfer.

Paths are relative to `android/app/src/main/`; Kotlin is under
`java/com/geebeeayy/app/`.

Re-verify before each release: this reflects the tree at 2026-10-13 on branch
`feat/privacy-policy-and-cover-names` (line numbers drift). Nothing here was exercised on a device or with a
traffic capture; it is read from code only.

## Evidence: every network path

Grep for `HttpURLConnection`, `URL(`, `okhttp`, `openConnection`, `http://`,
`https://` over `android/` and `core/src` finds exactly four clients and no
HTTP library. The core (`core/src`) makes no network call.

| Path | Code | Trigger |
|---|---|---|
| RetroAchievements API | `engine/RaEngine.kt:316-322` (`fetch`), called from `:298-300` | rcheevos asks for a URL; only after sign-in (see below) |
| RA badge images | `data/BadgeCache.kt:65-70` | achievements list open; needs a signed-in session to have a URL |
| Cover art | `data/CoverArt.kt:205-206`, `BASE` at `:237-238` | setting `getDownloadCoverArt()` , default `false` (`data/DisplaySettings.kt:260`), toggle at `ui/screens/SettingsScreen.kt:565-569` |
| Homebrew | `data/HomebrewDownloader.kt:135`, URLs in `data/HomebrewCatalog.kt:42-106` | player taps download |

`AndroidManifest.xml:10` declares `INTERNET`; there is no other network
permission. No `networkSecurityConfig` and no `usesCleartextTraffic`, so the
platform default applies: `android/app/build.gradle.kts` sets targetSdk 36
(`:18`), and cleartext HTTP is blocked by default for targetSdk 28 and up.

## Do ROMs or saves leave the device?

No. No code path writes ROM or save bytes to a network stream: the only
`outputStream` on a connection is the RA POST body (`engine/RaEngine.kt:330-335`),
which holds what rcheevos built (login, hash, unlock and score requests), not
files. ROM bytes are given to `RaEngine.hashRom` (`:236`, native
`ra_bridge.c:841`), which returns an MD5 string; only the hash goes out
(`viewmodel/EmulationViewModel.kt:526-527`). Saves and states are written
locally only. User-initiated save export goes through the system file picker,
not a network.

Caveat: Android's own Auto Backup can upload saves and states to the user's
Google account (`AndroidManifest.xml:24`, `allowBackup="true"`). That is
Google's transfer of the user's own data, not a transmission by the app, and it
is outside "collect" as Play defines it. It is stated in the policy.

## Backup rules and the RA token

`res/xml/backup_rules.xml:6` (Android 11 and lower) and
`res/xml/data_extraction_rules.xml:6,9` (Android 12+, both `cloud-backup` and
`device-transfer`) each exclude `sharedpref` `retroachievements.xml`. That is
the file `engine/RaCredentials.kt:20` writes (`getSharedPreferences(
"retroachievements", MODE_PRIVATE)`), holding keys `user` and `token`
(`:37-38`). So the token and username are excluded from backup. Everything else
(saves, states, other settings) is included, deliberately (comment in
`backup_rules.xml:2-4`). The token is stored unencrypted in the app's private
storage (`RaCredentials.kt:13-18`), which is not a Data safety matter but is
true.

## Is anything sent without the player signing in?

No RetroAchievements traffic. `RaEngine.start` only sends a token login when a
saved token exists (`RaEngine.kt:150-155`). `loadGame` is called for every game
(`EmulationViewModel.kt:527`, `RaEngine.kt:189`) but rcheevos
returns `RC_LOGIN_REQUIRED` without a request when no user is signed in
(`cpp/rcheevos/src/client/rc_client.c:2715`). Not exercised on a device.

## Transport security

Everything is HTTPS in practice. No finding of plain HTTP, with notes:

- RA hosts are `https://retroachievements.org` and
  `https://media.retroachievements.org` (`cpp/rcheevos/src/api/rc_api_common.c:12-13`).
  The library also contains `http://` variants (`:14-15`) selected only by
  `rc_api_set_host("http://retroachievements.org")`; `cpp/ra_bridge.c` never
  calls it, so they are dead code here.
- Cover art: hardcoded `https://thumbnails.libretro.com/...` constant
  (`CoverArt.kt:237-238`); no runtime URL is involved, so no guard is needed.
- Homebrew: all eight URLs are `https://github.com` or
  `https://raw.githubusercontent.com` (`HomebrewCatalog.kt:42-106`), hardcoded.
  GitHub release URLs redirect to GitHub's file-storage hosts. The download is
  verified by size and SHA-256 (`HomebrewDownloader.kt:155-160`), so transit
  tampering is also caught.
- **Finding F1 (fixed):** the two paths that take a URL from outside the
  code, RetroAchievements requests (URLs built by rcheevos) and badge images
  (URL from a server response, `RaEngine.kt:260`), used to open whatever they
  were handed. Both now refuse anything that is not `https://`
  (`isHttps` at `BadgeCache.kt:23-24`, checked at `BadgeCache.kt:66` and
  `RaEngine.kt:318`). Cover art and homebrew do not need a guard: their URLs
  are hardcoded `https://` constants (`CoverArt.kt:237-238`,
  `HomebrewCatalog.kt:42-106`).
- **Redirects:** Android's `HttpURLConnection` follows up to five redirects,
  across origins, but "doesn't follow redirects from HTTPS to HTTP or vice
  versa" (developer.android.com/reference/java/net/HttpURLConnection, checked
  2026-10-13). So a redirect cannot downgrade a request to cleartext, and
  cleartext is blocked by the targetSdk default anyway. Two independent
  barriers; neither was exercised on a device.

## Finding F2 (fixed: option C, chosen 2026-10-13)

`CoverArt.candidateNames` used to fall back to the ROM's raw file name when the
game code was not in the bundled index, and that name went into the URL. This
was audit item M7: a file name can contain anything the user typed.

Removing the fallback would have cost the covers of games whose DAT entry has
no game code (289 retail games, *Yggdra Union* among them). Instead the APK now
also ships every No-Intro name (`assets/gba-game-names.txt`, generated by
`tools/gen-cover-index.py`), and the file name is sent only when it is exactly
one of them. Every name that reaches the server is therefore public catalogue
data, never user-typed text.

## Form answers

### Section 1: Does your app collect or share any of the required user data types?

**Yes.** Optional features send data to third parties (RetroAchievements,
libretro). The developer receives nothing, but Play counts the library and
third-party transfers.

### Section 2: Is all of the user data collected by your app encrypted in transit?

**Yes.** See "Transport security".

### Section 3: Do you provide a way for users to request that their data is deleted?

**No account in the app, so the option "users can request deletion" does not
apply to data held by the developer, because the developer holds none.**
Answer the account-creation question as: the app does not let users create an
account (it only signs in to an existing retroachievements.org account), so no
deletion-URL is required. Local data is removed by uninstalling, by Sign out
(`RaEngine.logout`, `RaEngine.kt:181-186`, clears the token) and by Settings >
clear cover cache (`SettingsScreen.kt:582`). Deleting the RetroAchievements
account is done at RetroAchievements.

If Play insists on a deletion path for a given data type, point to the privacy
policy section "Third-party services".

### Section 4: Data types

Types not listed here are **not collected**: location, financial info,
health, messages, photos and videos, audio, files and docs (ROM files are not
transmitted, only a hash), calendar, contacts, web browsing, app info and
performance (no crash reporting, no diagnostics), device or other IDs (the app
reads no advertising ID or Android ID; see the IP address note).

| Data type | Collected | Shared | Optional | Purpose | Evidence |
|---|---|---|---|---|---|
| Personal info > User IDs (RA username, and the one-time password) | Yes | No | Yes, only if the player signs in | App functionality, Account management | `RaEngine.kt:177-179` (login), `RaCredentials.kt:20-38` (stored token) |
| App activity > Other actions (game hash, unlocks, leaderboard scores, rich presence) | Yes | No | Yes, only when signed in | App functionality | `RaEngine.kt:236`, `EmulationViewModel.kt:526-527`, `:298-300` |

For both: **Processed ephemerally: No** (RA keeps them on its servers);
**Required or optional: optional**; encrypted in transit: **yes**.

#### Reasoning for the ambiguous calls

**RA username and token as "User IDs".** The username is an account
identifier on a third-party service, which is what Play's "User IDs" means
("account ID, account number, or account name"). The password and token are
credentials, not a listed type; they belong with the username because they exist
only to authenticate that same account and are sent in the same request. The
token is held on-device (`RaCredentials`), not collected by the developer.
The password is never stored (`RaCredentials.kt:7-12`).

**"Shared" is No, not Yes.** Play does not count a transfer as sharing when the
user starts it for a feature they asked for and the recipient is the service
they chose. The player types their own RetroAchievements credentials, so the
transfer is the user's, to the user's own service. The same reading is
why cover art and homebrew do not count. If review pushes back, the safe
fallback is to tick Shared for User IDs and App activity with purpose "App
functionality". Not verified with Play.

**Game title in the cover-art URL.** Only titles from the public No-Intro
catalogue are sent (F2), so the name says which game the player owns but is
catalogue data, not a user identifier or user-typed content. Cover art is also
a switch the player turns on. It is not declared as a collected type.

**IP address.** Every server above sees it, as it does for any request. Play's
data types do not list IP address as its own entry, and the developer never
receives it. We do not declare it separately. Note that Play's FAQ treats IP
used to identify a device as "Device or other IDs"; none of these services is
sent anything from the app to key on it. Not verified with Play or with a
traffic capture; confirm in review. The privacy policy states that the servers
see it.

**Homebrew downloads.** Request to GitHub and its file-storage hosts (release
URLs redirect there) only; no user data is in it. Not
declared.

**Badge images.** A GET to `media.retroachievements.org` for a public image
(`BadgeCache.kt:70`). Only possible in a signed-in session. Covered by the RA
rows; no separate entry.

### Section 5: Privacy policy URL

Publish `docs/privacy-policy.md` at a public URL (for example the GitHub Pages
or raw rendered page of the repository) and paste the
URL. The URL must be reachable without sign-in.

## Not verified

- No device run or packet capture; everything above is from reading code.
- What RetroAchievements sends during a session (rich presence, ping, hardcore
  data) is rcheevos behaviour, not read line by line.
- Play's current wording and the "shared" exemption were taken from the
  definition quoted at the top, not re-fetched.
- `ROADMAP.md` Phase 6 still says the app "sends nothing anywhere", false since
  RetroAchievements and cover art (audit file, line 143).
