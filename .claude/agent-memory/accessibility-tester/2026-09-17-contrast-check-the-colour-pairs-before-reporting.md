---
name: contrast-check-the-colour-pairs-before-reporting
description: The four control tints pass WCAG AA at rest; an earlier review of them reported ratios 3-4x too low by comparing the wrong pairs.
metadata:
  type: project
---

**Context**: a contrast review of `data/ControlTint.kt` reported all four
tints failing WCAG AA, the worst at 1.99:1. Recomputed from the hex values in
`ui/theme/Color.kt` with the sRGB relative-luminance formula, the real numbers
are:

| tint    | label on fill | label on pressed |
|---------|---------------|------------------|
| NIGHT   | 15.91         | 3.84             |
| HONEY   | 4.59          | 12.33            |
| VIOLET  | 6.51          | 14.40            |
| CLASSIC | 7.04          | 8.26             |

**Finding**: every at-rest pair passes 4.5:1. The claim that pressed states are
worse is backwards in three of four - a pressed button swaps to
`labelPressed` on `pressed`, which is a deliberately high-contrast pair
(`BurntRoot` on `GoldenSaplight`, and so on).

The one real shortfall is **NIGHT while held**: `BurntRoot` on `AmberResin` is
3.84:1. That passes the large-text rule for the 20.sp face buttons but not for
the 13.sp pill labels (L, R, START, SELECT). It is transient feedback rather
than content, so it was left alone.

**Application**: `ControlPalette` has four colour pairs, not one - `label` goes
on `fill`, `labelPressed` goes on `pressed`. Compute both, and compute them
from the hex values in `ui/theme/Color.kt` rather than by eye. Check the label
size too: 3:1 is the bar for large text, 4.5:1 for the rest.
