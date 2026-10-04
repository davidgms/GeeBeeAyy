---
name: replaying-a-test-roms-own-code
description: To turn a failing homebrew-suite case into a ROM-free test, copy the suite's own instructions (and its IRQ dispatcher) out of a running ROM instead of hand-writing them
metadata:
  type: feedback
---

When a suite's expected values are hardware cycle counts, a hand-written
equivalent does not reproduce them: the IRQ dispatcher's length (libgba
`IntrMain`, ~45 ARM words) is part of the measured result. What worked on
2026-10-03 (`core/tests/timer.rs`, `count_up_case`):

1. Probe binary runs the ROM until the CPU sits on the interesting `swi`/store
   (match `bus.read32(pc)` plus a register value), then dumps
   `read32(0x03007FFC)` (the IRQ handler) and IWRAM around PC.
2. Disassemble with capstone from `/home/david/projects/GeeBeeAyy/temp/venv/bin/python3`
   (`md.skipdata = True`, or it stops at the first data word).
3. Copy the words into the test at the same addresses; fix up literal-pool
   words that point at RAM you relocate; set the loop registers directly
   instead of copying `ldr rX, =...`.

**Why:** the replica fails and passes exactly where the suite does (129 vs
123 passes before the fix), so it is a real gate, not an approximation.

**How to apply:** for an unmatched suite number, first confirm the replica
reproduces the emulator's *wrong* suite value; only then trust it. Also: a
`Gba::step` loop on a halted CPU hides wake-up timing - the halted path steps
in event-sized chunks, so check what it steps to before fitting constants.
