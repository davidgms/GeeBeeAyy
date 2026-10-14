# GeeBeeAyy! privacy policy

Effective date: 2026-10-13

## Who we are

GeeBeeAyy! is a free, open source Game Boy Advance emulator for Android. The
source code is public. There is no company server behind the app: we do not run
any backend, database or analytics service.

Contact: davidgms31@gmail.com

## What stays on your device

Everything you put into the app stays on your device:

- your game files (ROMs) and the folders you choose to scan,
- your in-game saves and save states,
- your settings and control layouts,
- the picture and file-name information the app reads from your games to show
  its library.

Your game files and saves are never uploaded by the app.

## What is sent, and to whom

The app can reach the internet in three cases, and only these three.

**1. RetroAchievements - only if you sign in.** If you sign in to your
retroachievements.org account in Settings, the app talks to
retroachievements.org and media.retroachievements.org. It sends your username
and password once to sign in, then a login token instead of the password. When
you play, it sends a fingerprint (hash) of the game file so the service can
recognise the game, and it reports your achievements, leaderboard scores and
what you are playing, as the service requires. The token and username are kept
on your device so you stay signed in; the password is never stored. Signing out
removes them. If you never sign in, nothing is sent to RetroAchievements.

**2. Cover art - only if you turn it on.** Cover art download is off by default.
If you enable it in Settings, the app asks thumbnails.libretro.com for the box
art of a game, and the request contains the game's official title from the
public No-Intro catalogue. The app finds that title from the code inside the
game, or uses the file name only when it is exactly one of those catalogue
titles, so a file name you chose yourself is never sent. Pictures are saved on
your device.

**3. Homebrew downloads - only when you tap download.** The Homebrew catalog
fetches the free test and demo games you choose from github.com and
raw.githubusercontent.com. Only the download request is made.

Like any internet request, these servers can see your IP address and the
standard request details. We do not receive or see any of it; the servers named
above are run by other parties.

All of these connections use HTTPS.

## What we do not do

- No ads.
- No analytics and no crash reporting service.
- No tracking and no advertising identifiers.
- We never sell or share your data, and we hold none.

## Third-party services

What those services do with the data they receive is covered by their own
policies:

- RetroAchievements: https://retroachievements.org/ (see its privacy policy
  there)
- libretro: https://www.libretro.com/ (thumbnails.libretro.com)
- GitHub: https://docs.github.com/en/site-policy/privacy-policies/github-general-privacy-statement

To have your RetroAchievements account or its data deleted, contact
RetroAchievements; we have no copy.

## Backups

Android may back up the app's data to your Google account if you have backup
enabled, and may copy it when you move to a new phone. That is a feature of
Android, run by Google, not by us. Saves and save states can be included so
your progress moves with you. Your RetroAchievements login token is excluded
from backups, so you sign in again on the new phone. You can turn backup off in
your Android settings.

## Children

The app is a general-purpose emulator and is not directed at children. We do
not knowingly collect data from anyone, children included, because we collect
none.

## Changes

If this policy changes, the new version is published at the same place with a
new effective date. The history is in the project repository.
