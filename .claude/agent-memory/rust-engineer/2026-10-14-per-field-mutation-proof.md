---
name: per-field-mutation-proof
description: Proving each field of a multi-field fix has its own failing test, and a pre-fix run without touching the worktree
metadata:
  type: feedback
---

When one fix adds several fields (save state v11 added ten), a single
"all new tests fail at HEAD" run does not show that each field is covered:
one field (PPU `cycle_counter`) broke 6 of 9 tests at once and hid that the
halt test never exercised `halt_swi`.

**How to apply:**
- Pre-fix proof: `git archive HEAD core | tar -x -C temp/prefix`, copy the new
  test file in, run with `CARGO_TARGET_DIR=temp/prefix-target` so the main
  target dir is not rebuilt. No stash or checkout needed (checkout is blocked).
- Per-field proof: a script that swaps one `field = read_x(r)?;` for
  `let _ = read_x(r)?;`, runs the test file, records failures, and restores
  the source in a `finally`. Template: `temp/v11_mutate.py` (gitignored, may be
  gone; it is 40 lines). Run cargo **without `-q`** so `test name ... FAILED`
  lines are parseable.
- A field whose mutation fails nothing either needs a test or is derivable -
  that is how the DISPSTAT copies were found to be derivable, not saved.

**Why:** 2026-10-14, state v11. Details in `.claude/memory.md` (2026-10-14 entry).
