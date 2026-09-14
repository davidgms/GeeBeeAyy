# What a shipped GBA emulator has that we do not

A walk through screenshots of `com.emu.gba1x`, taken 2026-09-10, against what
GeeBeeAyy does today. The point is not to copy a menu tree - it is to find the
places where we are missing something a player expects, and to be able to say
out loud which of their choices we are deliberately not making.

Everything below is checked against our code, not guessed at.

## The ROM list

| Their app | Us | Verdict |
|---|---|---|
| Thumbnail per row, auto-captured from play | Cover art the player supplies, `RomArtwork.kt` | **Ours is a decision, not a gap.** See below. |
| Relative date: "3 de set. de 2026 (6 days ago)" | Absolute only: "Last played: 9 set., 21:41" | **Take.** "6 days ago" is read at a glance; a date is read by doing arithmetic. |
| Rows for ROMs that are gone, marked "File not found" | Missing files vanish from the list | **Take.** A game disappearing silently looks like the app lost it. |
| Favourites, with a star filter in the header | `RomEntry.isFavorite` exists and **nothing ever sets it** | **Take.** Half of it is already written, including the "Favorites" section in the list. |
| Long press: Reset and start / Rename / Delete save data / Delete game / Show information | Long press: information only | **Take the menu.** We already have the dialog; it should be one item on a menu. |
| List / Grid (small) / Grid (large) | List only | **Later.** Real, but it is a second layout to maintain for a list most people scroll once. |
| `.zip` ROMs, with a Help dialog explaining 7z is not supported | `.gba`, `.agb`, `.bin` only | **Take.** Downloaded ROMs arrive zipped; asking the player to unzip on a PC is the kind of friction that makes an app feel unfinished. |
| "Hide duplicate files" | - | **Skip.** A symptom of their remote-download catalogue, not of a folder the player filled. |

**On thumbnails.** `RomArtwork.kt` says, in as many words, that a thumbnail
which changes every time you put the game down makes the list harder to scan,
and that a fixed picture is what lets you find a game by shape. That argument
still holds. The gap worth closing is that **almost nobody has cover art
sitting next to their ROMs**, so in practice our rows show the same placeholder
pad icon over and over - which is worse than either option. Either we let a
screenshot stand in when no cover exists, or we make the placeholder carry the
game's own identity (its initials, a colour from its title hash).

## In game

| Their app | Us | Verdict |
|---|---|---|
| Hide status bar, hide navigation bar | Both always visible | **Take.** We hand a 20:9 phone's chrome about 200px of a screen that is already limited by width. |
| Clock & battery meter, **as a checkbox** | Always on, added in PR #14 | **Take the checkbox.** We built the feature; theirs is the reminder that some people want the screen clean. |
| Separate layouts for portrait and landscape | One layout per game, shared | **Take.** The two orientations do not want the same button positions, and our landscape branch cannot be customised at all today. |
| Auto load state on open | - | **Take, off by default.** |
| Show FPS | - | **Take.** We have spent two PRs on speed and had to instrument the loop by hand each time. |
| Enable cheating | - | **Skip for now.** Its own feature, and not one you have asked for. |
| High quality / power saving / extreme power saving preset | - | **Skip.** A preset over settings we mostly do not have yet. |

## Controls

| Their app | Us | Verdict |
|---|---|---|
| **D-pad diagonals area size** | Hard-coded `DPAD_CARDINAL_HALF_DEGREES = 30f` | **Take.** This is the exact knob flagged as wanting real-hand tuning when the cross was built. They ship it as a setting because there is no right answer. |
| Strength of vibration, or off | No haptics anywhere | **Take.** A touch button with no feedback is the single biggest difference between a screen and a pad. |
| "Enable A+B between A and B" - a hidden zone that presses both | - | **Take.** Cheap: it is the same angular-sector idea the D-pad already uses. |
| Auto hide virtual pad when a gamepad is connected | No gamepad support at all | **Blocked.** Needs the gamepad feature first. |
| Button animation toggle | - | **Skip.** |

## Audio

| Their app | Us | Verdict |
|---|---|---|
| Audio volume | On/off only | **Take.** Ours is a mute button and nothing between. |
| Low latency mode, as a toggle | Always on (`PERFORMANCE_MODE_LOW_LATENCY`) | **Skip.** Theirs is a toggle because OpenSL made it a gamble. Ours is an AudioTrack asking politely; there is nothing to turn off. |
| "Check buffer underrun" | - | **Skip as a setting, take as a fact.** Underruns are what fast forward above 4x sounds like. Worth counting and showing next to the FPS, not worth a checkbox. |

## The smaller things, gone through a second time

The first pass took the substantial features and skipped past the furniture.
Everything visible in the screenshots is now accounted for, including what is
being turned down and why.

| Their app | Us | Verdict |
|---|---|---|
| **Share button** in the ROM list header | We save screenshots to `Pictures/GeeBeeAyy` and then forget about them | **Take, as share a screenshot.** We already take them; nothing in the app offers to send one anywhere. |
| **Orientation** as three choices - auto, portrait, landscape | `Force Portrait`, a boolean | **Take.** Ours can lock to portrait or let go, and cannot lock to landscape - which is the one a player holding a phone sideways for a whole session wants. |
| **"Reset and start"**, distinct from opening the game | - | **Take, once auto load state exists.** Only means anything when opening a game normally resumes it; the two features arrive together or not at all. |
| **"Delete save data"** from the list | Only reachable by deleting files by hand | **Take.** It sits in the same context menu as the rest. |
| **ROM size** beside file size in the info dialog | File size only | **Take with ZIP.** The two numbers only differ for a compressed ROM, so this is part of that work, not its own item. |
| **Help dialog** explaining accepted formats | An empty state that says "Go to Settings, ROM Folders" | **Ours is close enough.** Theirs is a wall of text behind a menu; ours is on the screen where the problem is. |
| Navigation drawer behind a hamburger | A row of icons in the top bar | **Skip.** A drawer earns its place at about a dozen destinations. We have four. |
| Floating **search** button | - | **Skip.** It searches their remote catalogue of commercial ROMs, which is the one thing `HomebrewCatalog.kt` exists to refuse. |
| "Hide Remote Games" | - | **Skip**, same reason. |

## What we have that they do not

Worth writing down, because the goal is a good product and not a clone:

- **Rewind.** Hold to walk backwards through the last six seconds.
- **Interframe blending**, which is why Yggdra Union's "SAVE DATA" title does
  not strobe.
- **Custom buttons** - combo, sequence and toggle-hold, built by the player.
- **Per-button resize** in the layout editor. Theirs resizes the whole pad.
- **A save-state slot list** with times, rather than a bare quick save.
- **Homebrew downloads** from a curated, legal catalogue.

## Decisions taken, 2026-09-10

- **Thumbnails**: cover art when the player supplied it, and a **generated
  placeholder** - the game's own initials over a colour derived from its name -
  when they did not. No screenshots as thumbnails: the reason written into
  `RomArtwork.kt` stands, and this closes the real gap, which was every row
  showing the same grey pad.
- **System bars**: hidden while a game is running, behind a setting. This is
  what makes the clock and battery strip earn its place rather than duplicate
  the one above it.
- **First**: haptics and the D-pad diagonal setting, ahead of the ROM list
  pass. Both are felt on every press.

## Suggested order

1. **Haptics** and the **D-pad diagonal setting** - both are felt on every
   single press, and the second is a knob we already knew we wanted.
2. **Hide the system bars** and make the clock/battery strip a toggle - the
   biggest visible change per line of code.
3. **Favourites**, the **row context menu**, **relative dates** and
   **"file not found"** rows - one pass over the ROM list, all small.
4. **ZIP support** and **auto load state**.
5. **FPS and underrun counter**, then **audio volume**.
6. **Portrait and landscape layouts** kept apart.
