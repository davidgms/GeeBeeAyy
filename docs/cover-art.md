# Where cover art could come from

Research 2026-09-23, for the ROM list's thumbnails. Every number below was
measured against the live sources on that date; the commands are in the
sections that produce them, so they can be re-run when this goes stale.

Today `RomArtwork.kt` is local-only: `<rom base name>.png|jpg|jpeg|webp` beside
the cart or in a `covers/` folder, and a generated initials tile when there is
nothing. This is about whether to add a second source, not about replacing
that one.

## The recommendation

**Fetch from the libretro thumbnail server, matching on the cart's own game
code first and the exact No-Intro filename second.**

```
GET https://thumbnails.libretro.com/Nintendo%20-%20Game%20Boy%20Advance/Named_Boxarts/<No-Intro name>.png
```

No key, no account, no registration, no published or observed rate limit.
Verified live: *Yggdra Union - We'll Never Fight Alone (USA)* returns 200 and a
578,659-byte PNG.

**Both match paths are needed. Neither is enough alone**, and that is the one
place this differs from how the approach is usually described.

## Why one match key is not enough

The `serial` field in libretro's No-Intro DAT is exactly the 4-byte game code
at 0xAC, so the join is exact rather than heuristic. Counting the DAT
(`metadat/no-intro/Nintendo - Game Boy Advance.dat`, version 2026.08.01):

| | count |
|---|---|
| entries | 3692 |
| entries carrying a serial | 3218 |
| distinct serials | 2763 |
| serials mapping to more than one name | 337 |
| **entries with no serial at all** | **474** |
| of those, tagged Beta/Proto/Demo/Unl/etc | 185 |
| **of those, plain-looking retail releases** | **289** |

The 289 is what matters. It is easy to assume the serial-less entries are
oddities, and they are not:

```
Yggdra Union (Japan)                              serial= None
Yggdra Union - We'll Never Fight Alone (Europe)   serial= None
Yggdra Union - We'll Never Fight Alone (USA)      serial= None
```

The cart itself has a game code - `BYUE`, read straight out of the test
phone's copy - but the DAT does not carry it, so a game-code lookup finds
nothing.

On the two commercial games actually on the test phone, game code alone is
**1 for 2**:

| game | header | in DAT by serial | filename equals a DAT name | art |
|---|---|---|---|---|
| Mario Tennis Advance - Power Tour (U).gba | `MARIOTENNISA` / `BTME` | **yes** | no (`(U)` vs `(USA, Australia) (En,Fr,De,Es,It)`) | **yes, via serial** |
| Yggdra Union - We'll Never Fight Alone (USA).gba | `YGGDRA UNION` / `BYUE` | **no** | **yes, exactly** | **yes, via filename** |

The two mechanisms cover each other's holes, and both are cheap. This is also
the order Lemuroid uses on Play (CRC, then serial, then filename).

Where the game code does win is the case a filename cannot survive: a renamed
or trimmed ROM. A trimmed GBA ROM keeps its header, so the code still reads,
while a file hash does not survive trimming at all - and hashing costs a read
of up to 32 MB per cart against 176 bytes for the header.

For the 337 serials with several names, the alternatives are revisions of one
title (`... (USA)` / `(Rev 1)` / `(Virtual Console)`) that share box art.
Checked both names under `BTME`: **both return 200**. Taking the first is fine.

## What it costs

| | |
|---|---|
| `INTERNET` permission | **already declared**, for the homebrew downloader. The manifest comment saying nothing else makes a request would need updating. |
| Lookup asset | game code to name: 2763 entries, 141 KB JSON, **38 KB gzipped**. Plus the name list for the filename path: 3691 names, **36 KB gzipped**. |
| Image library | **none.** `RomArtwork` already decodes and downsamples with `BitmapFactory`; a fetch is a stream into `cacheDir`. Adding Coil or Glide would pull a dependency graph into an app that deliberately has almost none. |
| New code | a downloader, one more location in `RomArtwork.findFile`, a Settings toggle. |
| Failure path | 404, no network, no match - the initials tile, which already exists. |

Rough effort: 2 to 4 days, most of it the asset generation script and the
Settings plumbing rather than the download.

## Licensing, stated plainly

**No source in this space has a licence to the box art.** It is
publisher-owned, and libretro's own README says so: *"The game art itself and
promotional art originates from the work of each respective game's developers
and publishers."* ScreenScraper, TheGamesDB and LaunchBox all host
publisher art uploaded by users. The question is not which source is clean -
none is - but where the exposure sits.

The line worth building around:

- **The app fetches, caches, and shows.** The image travels from a third
  party to the user's device at the user's request. GeeBeeAyy is a client, the
  same posture as a browser.
- **The app bundles art in the APK.** Now it is distributing publisher
  artwork through Google Play under its own name. **Do not do this.** It is
  also 2.47 GB for GBA alone, so it was never practical anyway.

Keep the feature on the first side of that line, and keep the fetch **off by
default**: offline-first means the app does not touch the network until asked,
and it makes the posture "the user asked for this".

Two concrete obligations:

- The thumbnail repositories carry **no licence file and no terms of service**
  at all. No permission granted, but no term to breach either.
- `libretro-database`, where the DAT comes from, **is CC-BY-SA-4.0**
  (confirmed: its `LICENSE` is the Attribution-ShareAlike 4.0 text). A lookup
  table derived from it should credit libretro-database and carry the same
  licence. That costs one line in an about screen.

None of this is legal advice.

## The alternatives, and why not

| Source | Matching | Key / account | Verdict |
|---|---|---|---|
| **libretro thumbnails** | serial + filename, via the DAT | **none** | **Take.** |
| ScreenScraper | crc / md5 / sha1, filename, file size | a devid granted after presenting the app on their forum, **plus every user registering their own account** | The fallback if libretro disappears. Its terms also restrict integration to *"entirely free and distributed"* applications, which becomes a standing constraint on ever charging for the app. |
| TheGamesDB | title + platform only | yes, and **no terms of service page could be found at all** | No. A source with no published terms is not a foundation. |
| IGDB | **title only** | Twitch app, client id **and secret** | No. The secret cannot ship in an APK, so it needs a server we would have to run, for a cosmetic feature. Its terms are the clearest of the lot and it explicitly permits caching - it is simply the wrong shape. |
| OpenVGDB | hash only | none | No. Unmaintained since 2021, no licence, `romSerial` is NULL on every GBA row, and its cover URLs point at GameFAQs, which refuses non-browser requests. |
| Bundling art in the APK | - | - | **No.** See above. |

## The honest option zero

The generated initials tile already solves the thing that was actually broken:
every row showing the same grey cartridge, with nothing to aim a thumb at.
Cover art is nicer, not necessary. If the answer to "is a few days of work plus
a network round trip worth prettier rows" is no, keeping `RomArtwork`
local-only is a defensible place to stop, and this document is the record of
why.
