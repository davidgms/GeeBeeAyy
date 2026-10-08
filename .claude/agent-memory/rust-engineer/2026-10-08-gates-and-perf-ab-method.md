---
name: gates-and-perf-ab-method
description: Gate scripts must use cargo test --no-fail-fast; shell for-loops over variables are refused, put them in temp/*.sh; how to prove a refactor behaviour-neutral and measure per-frame cost
metadata:
  type: reference
---

- `cargo test` stops at the first failing test binary, so a gate that greps
  "test result: ok" silently skips every later binary. Use `--no-fail-fast`
  (temp/gate.sh in the wait-states worktree does).
- The sandbox refuses inline `for x in ...; do $x ...` loops and long
  `python3 - <<EOF` edits; writing the same thing to `temp/foo.sh` or
  `temp/foo.py` and running the file works.
- Neutral refactor proof: build the base commit (`git archive <sha> core`
  into temp/base) and the worktree as two tiny crates, hash every step's
  (PC, cycles) plus frame buffers over 1500 frames of several ROMs, compare
  hashes (temp/trace.sh pattern).
- Per-frame A/B: interleaved, `taskset -c 3`, 5-7 rounds, compare medians.
  Small source changes swing Mario Tennis by 5-10% (layout, a second match
  defeating a jump table); always re-measure after any hot-path edit.
