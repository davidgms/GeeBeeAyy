---
name: the-two-control-size-multipliers-used-to-stack
description: The global and per-control size sliders are separate graphicsLayers and multiplied, so a 48.dp button could be drawn at 20.dp.
metadata:
  type: project
---

**Context**: reviewing the touch overlay after the September 2026 layout work.

**Finding**: `DisplaySettings.MIN_CONTROL_SCALE` is 0.7 (the Settings slider)
and `ControlLayoutStore.MIN_CONTROL_SCALE` is 0.6 (the layout editor's per
control stepper). They are applied as two nested `graphicsLayer`s - the global
one on the whole overlay in `EmulationScreen.kt`, the per-control one in
`movableControl` - so they **multiply**: 0.42, which draws a 48.dp pill or a
48.dp D-pad arm at 20.16.dp and a 56.dp face button at 23.52.dp.

**Application**: fixed 2026-09-17 by flooring the product at
`MIN_EFFECTIVE_CONTROL_SCALE = 0.7f` inside `movableControl`, using the new
`LocalGlobalControlScale`. The floor lives there, not in the stepper, because
the global slider can move after a control was already sized. When two size
multipliers are applied as separate layers, check the product, never each
minimum on its own.
