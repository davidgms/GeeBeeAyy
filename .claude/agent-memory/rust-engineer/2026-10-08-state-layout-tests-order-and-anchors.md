---
name: state-layout-tests-order-and-anchors
description: Writing byte-offset save-state tests when one fix in the batch changes the state layout
metadata:
  type: feedback
---

When a batch of save-state fixes includes one that **changes the layout**
(e.g. the v10 ROM CRC appended to the tail), do that item first - test, fix,
green - and only then write the byte-offset tests for the others. Written
against the old layout, every tail-relative offset is 4 bytes off after the
layout change, so the "failing first" run proves nothing.

**Why:** 2026-10-08 Phase 5 batch; caught before writing, not after.

**How to apply:** anchor every offset test with an assertion that the bytes
there hold a known value (`cycles` == `gba.cycles`, APU length field in a
plausible range) before editing them; a layout drift then fails loudly
instead of editing the wrong field and "passing". Head offsets (fixed-size
prefix) are stable; tail offsets move with every version. Current constants:
`core/tests/state_restore.rs`.

Also: an HLE SWI hang test fails fast in **debug** if its addresses sit near
0xFFFFFFFF (the plain `+` overflow panics on entry 0), so the red run does not
need the minutes-long release hang.
