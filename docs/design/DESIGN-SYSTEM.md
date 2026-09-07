# GeeBeeAyy design system

The whole system comes from one picture: [`gb-keyart.png`](gb-keyart.png). GB
in his room, at night, holding a Game Boy Advance. Every colour below was
sampled out of that file, not invented. When a decision is unclear, open the
key art and look.

---

## 1. GB

**GB is a bee who plays games.** Not a logo, not a helper, not a guide. He is a
character with a hobby, and the app is his hobby.

His vibe, in the order it has to read:

1. **Cool first.** Wasp-eye shades, permanent grin, head tilted at the camera.
2. **Cute underneath.** Blush on both cheeks, tiny wings, round everything.
   The shades say cool, the blush says he is not actually intimidating.
3. **A gamer, not a mascot for gamers.** He owns the console. He does not
   present it to you like a salesman.

### The rules

- **He is always GB.** Never "G", never "G-Bee", never "the bee" in
  user-facing copy. The app is GeeBeeAyy; the character is GB.
- **Shades never come off.** They are his face. No eyes behind them, ever.
- **He never wears a shirt.** Stripes are the outfit.
- **He never speaks in the UI chrome.** He can appear next to a message; he
  does not become the message. No speech bubbles in menus.
- **He is never the loading spinner.** A mascot that only shows up when the
  app is slow becomes the face of waiting.

### Where he appears

| Place | Which mark | Size |
|---|---|---|
| Launcher icon | Face only | Adaptive foreground, `drawable-nodpi/ic_launcher_foreground.png` |
| Splash | Face only | 160dp |
| Empty ROM list, first run | Face only | 120dp |
| Store graphic, README, key art | Full key art | As supplied |
| Anywhere else | He does not | - |

Two marks exist. Both are in [`gb-icon-compare.png`](gb-icon-compare.png):

- **Face only** ([`gb-face.png`](gb-face.png)) - the default. It survives a
  48dp launcher icon, which is the only size that actually has to work.
- **Face + GBA** ([`gb-face-gba.png`](gb-face-gba.png)) - for anything 96dp and
  larger, where the console still reads as a console instead of four grey
  pixels.

---

## 2. Colour

Two families, and the split between them **is** the system.

- **Night** carries structure. Backgrounds, panels, dividers, borders.
  Sampled from GB's room, and near enough to the GBA's own violet shell that
  the app feels like the hardware it emulates.
- **Honey** carries identity. Actions, key text, GB himself. It is what the
  eye lands on.

> **Night never carries meaning. Honey never carries structure.**
> A violet button is a mistake. A honey background is a mistake.

### Night - structure

| Token | Hex | Use |
|---|---|---|
| `NightVoid` | `#150A2B` | App background, top bar, status and navigation bars |
| `NightPanel` | `#221046` | Cards, list rows, sheets, on-screen control buttons |
| `NightRaised` | `#2E1660` | Dialogs and anything stacked above a panel |
| `NightEdge` | `#45268A` | Dividers, borders, unchecked switch tracks |

### Honey - identity

| Token | Hex | Use |
|---|---|---|
| `GoldenSaplight` | `#FACC15` | Primary action, GB's body, the one colour that means "press this" |
| `AmberResin` | `#A16207` | Pressed and held state of a honey control |
| `HoneyLight` | `#D4A017` | Honey at half power: progress tracks, secondary fills |
| `PineGlowMist` | `#FFF9C2` | Body text and icons on any night surface |
| `BeeWing` | `#FDEEB7` | Headings and large numerals. GB's teeth |
| `BurntRoot` | `#1A0F00` | Ink **on** a honey fill, and nothing else. GB's own outline |

### Neon - accents from the room

Used in single doses. Two neons on one screen is one too many.

| Token | Hex | Use |
|---|---|---|
| `NeonViolet` | `#7C30BC` | The console shell. Secondary actions, save-state chrome |
| `NeonMagenta` | `#650EBA` | Selected chips, active tabs |
| `LensCyan` | `#55F6FD` | Focus rings, and "this is live right now" |
| `WingLavender` | `#B0A6CD` | Disabled controls, secondary text |
| `BlushPink` | `#FCA8CE` | Confirmations that should feel warm. Favourites |
| `LedGreen` | `#7CE04A` | Emulation running. The console's power LED |
| `Error` | `#FF5C7A` | Failures only |

### The one exception

The emulation screen background is **plain black**, not `NightVoid`. The game
picture is the content there, and every other colour has to get out of its
way. Dialogs and overlay chrome on that screen still use the night tokens -
they are not the background.

### Contrast

Every text pair in the system, measured. WCAG AA needs 4.5.

| Pair | Ratio |
|---|---|
| `PineGlowMist` on `NightVoid` | 17.6 |
| `PineGlowMist` on `NightPanel` | 15.9 |
| `PineGlowMist` on `NightRaised` | 14.0 |
| `BeeWing` on `NightPanel` | 14.7 |
| `LensCyan` on `NightVoid` | 14.4 |
| `GoldenSaplight` on `NightVoid` | 12.4 |
| `BurntRoot` on `GoldenSaplight` | 12.3 |
| `LedGreen` on `NightVoid` | 11.4 |
| `BlushPink` on `NightVoid` | 10.5 |
| `WingLavender` on `NightVoid` | 8.3 |
| `PineGlowMist` on `NeonViolet` | 6.5 |
| `Error` on `NightVoid` | 6.4 |
| `PineGlowMist` on `AmberResin` | 4.6 |

`PineGlowMist` on `AmberResin` is the tightest pair in the system. It passes
for normal text and it is a pressed state, so it is on screen for a moment.
Do not put small text on `AmberResin` permanently.

### Where the tokens live

| File | Owns |
|---|---|
| `android/app/src/main/java/com/geebeeayy/app/ui/theme/Color.kt` | The source of truth |
| `android/app/src/main/java/com/geebeeayy/app/ui/theme/Theme.kt` | Material 3 mapping |
| `android/app/src/main/res/values/themes.xml` | The same hexes, for the XML theme and system bars |
| `ios/GeeBeeAyy/Theme/GeeBeeAyyTheme.swift` | The iOS mirror |

Four files, one palette. Change one, change all four.

There is **no light theme**. The app is a night-themed emulator, and
`GeeBeeAyyTheme(darkTheme = false)` resolves to the same scheme on purpose.

---

## 3. Typography

Two faces, and **28sp is the line between them**.

- **Pixelify Sans** (bundled, `res/font/pixelify_sans_*.ttf`, SIL OFL - see
  [`fonts-LICENSE.txt`](fonts-LICENSE.txt)) carries
  the app's identity: the wordmark and screen titles.
- **The platform default** carries everything that has to be read.

The threshold is a number, not a judgement call, so it stays consistent as
screens get added.

| Style | Size / line | Face | Use |
|---|---|---|---|
| `displayLarge` | 36 / 48 | **Pixelify Sans** Bold | Splash wordmark |
| `headlineLarge` | 28 / 38 | **Pixelify Sans** Bold | Screen title |
| `headlineMedium` | 24 / 32 | Platform default SemiBold | Section heading |
| `titleLarge` | 20 / 28 | Platform default SemiBold | Card title, ROM name |
| `bodyLarge` | 16 / 24 | Platform default | Body copy |
| `bodyMedium` | 14 / 20 | Platform default | Secondary copy, list subtitle |
| `labelLarge` | 14 / 20 | Platform default Medium | Buttons and chips |

Rules:

- **Nothing below 28sp is ever set in the pixel face.** It loses its grid under
  font scaling and stops being readable at body sizes. That is the entire reason
  for the threshold.
- The pixel face gets `0.5sp` of letter-spacing and a taller line height than
  the default face at the same size. Pixel glyphs are dense; they need the room.
- Pixel lettering is also allowed **inside GB's art** - the console screen, the
  key art, a store graphic. That is illustration, not type.
- Never letter-space body text to look retro. The pixel face already does that
  job, at the sizes where it works.

iOS has no Xcode project yet, so the font is not registered there. When that
project exists, add both TTFs to the target and list them under
`UIAppFonts` in `Info.plist`, then apply the same 28pt threshold.

---

## 4. Spacing, shape, motion

Spacing is a 4dp grid. iOS already names it in `GeeBeeAyyDesign`; Android uses
the same numbers inline.

| Name | dp |
|---|---|
| XS | 4 |
| SM | 8 |
| MD | 16 |
| LG | 24 |
| XL | 32 |
| XXL | 48 |

Corner radius:

| Name | dp | Use |
|---|---|---|
| SM | 8 | Chips, small buttons |
| MD | 12 | Cards, list rows |
| LG | 16 | Sheets, dialogs |
| XL | 24 | The splash mark, hero surfaces |

Depth is **colour, not shadow**. `NightVoid` -> `NightPanel` -> `NightRaised`
is the whole elevation scale. Do not add drop shadows: they turn to mud on a
violet background.

Motion:

- 500ms or less for anything the user waits on.
- `FastOutSlowInEasing` for entrances, linear for progress.
- **Honour "Remove animations".** The splash already reads
  `Settings.Global.ANIMATOR_DURATION_SCALE` and drops to 0ms. Anything new
  that scales or moves does the same.

---

## 5. Components

**Primary button** - `GoldenSaplight` fill, `BurntRoot` label, radius SM.
Pressed goes to `AmberResin` with a `PineGlowMist` label. One per screen.

**Secondary button** - `NightPanel` fill, `PineGlowMist` label, `NightEdge`
border.

**Card / list row** - `NightPanel`, radius MD, no border. `NightEdge` divider
between rows only when they are not separated by space.

**Dialog** - `NightRaised`, radius LG. Title `BeeWing`, body `PineGlowMist`,
confirm is the primary button, cancel is text-only in `WingLavender`.

**On-screen GBA controls** - `NightPanel` at rest, `AmberResin` while held,
`NightRaised` for the D-pad's centre dish. Minimum touch target 48dp,
regardless of the drawn size.

The layout copies the AGB-001: a D-pad on the left, A and B offset diagonally
on the right (B low-left, A high-right), SELECT and START as small pills
underneath.

> **The D-pad is one cross, never four arrow glyphs.** That is what the
> hardware looks like, and it is the only shape in which four directions cannot
> end up in four different stroke weights - the failure mode every mixed icon
> set produces. Only A and B carry lettering; the cross carries none.

The app draws it that way: `DPad` in `EmulationScreen.kt` is one `Canvas`
with one touch area, and `ControlButton.DPAD` is a single entry the player
drags as one piece.

The touch area is the whole square, not only the drawn arms - a finger in the
corner, outside the cross, reads as a diagonal. Each cardinal owns 60 degrees
and each diagonal 30 (`DPAD_CARDINAL_HALF_DEGREES`), and the centre dish is
drawn at exactly the dead zone's radius, so the thumb rest a player sees is
the region that reports nothing. Those two numbers are the tuning knobs; they
want a real thumb on real glass, not a screenshot.

**Chip** - unselected `NightPanel` with `WingLavender` label; selected
`GoldenSaplight` with `BurntRoot` label.

**Status** - running is `LedGreen`, paused is `WingLavender`, failed is
`Error`. Never a colour alone: always a colour plus a word or an icon.

---

## 6. Assets, and how to rebuild them

The marks are drawn in code, not painted. The generator is
`temp/design-canvas/gb.mjs` (the character) plus `canvas.mjs` and `png.mjs`
(a pixel canvas and a dependency-free PNG codec). No npm install, no
`node_modules`.

```bash
cd temp/design-canvas
node export-icons.mjs a     # 'a' = face only, 'b' = face + GBA
```

That one command writes every icon asset:

| Output | What |
|---|---|
| `android/.../mipmap-{m,h,xh,xxh,xxxh}dpi/ic_launcher.png` | Legacy launcher icon, 48 to 192px |
| `android/.../drawable-nodpi/ic_launcher_foreground.png` | Adaptive foreground, 432px, art inside the safe zone |
| `android/.../drawable-nodpi/gb_face.png` | The mascot for in-app use |
| `android/.../drawable-nodpi/gb_face_gba.png` | The face + GBA mark |
| `docs/design/gb-face.png`, `gb-face-gba.png` | Full-size marks for docs |
| `docs/design/gb-icon-compare.png` | Both marks at 192, 96 and 48px |

The adaptive icon background is a vector, `drawable/ic_launcher_background.xml`
- `NightVoid` with a faint `NightPanel` honeycomb.

**To change GB, edit `gb.mjs` and re-run the export.** Never hand-edit a
generated PNG: the next export throws the edit away.

### The reference files

| File | What it is |
|---|---|
| `gb-keyart.png` | **The reference.** Every colour here was sampled from it |
| `color-pallete.jpg` | The original four honey colours |
| `hexagons.jpg` | The honeycomb motif |
| `bee-ref-1/2/3.jpg` | Early style references |
