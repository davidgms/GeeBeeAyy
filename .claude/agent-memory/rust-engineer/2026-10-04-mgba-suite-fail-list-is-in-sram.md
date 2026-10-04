---
name: mgba-suite-fail-list-is-in-sram
description: The mGBA suite logs every subtest verdict to SRAM (savprintf), so the exact failing cases are one SRAM dump away - no need to drive the per-test menu
metadata:
  type: reference
---

The mGBA suite's `savprintf` writes "DMA test: <name>" / "<field>: Got X vs Y:
FAIL" lines into cartridge SRAM as it runs. After a sub-suite finishes, dump
`gba.bus.peek(0x0E00_0000 + i)` for 64 KB, split on NUL, and print each FAIL
line with the last non-FAIL line before it: that is the exact failing list
with got/expected values. Lines repeat (the log is written twice); `sort -u`.

Probe used on 2026-10-04: a scratch crate in `temp/probe` (path dep on
`../../core`), run_frames(30), press DOWN `index` times, press A,
run_frames(600), dump. Sub-suite indices are those in
`core/tests/homebrew_suites.rs` (0 Memory ... 9 DMA).

Side effect to remember: the suite's own SRAM log is what its SRAM DMA/load
tests read back (0x47 'G', 0x61 'a'), so SRAM expectations depend on it.

Also: a per-commit perf A/B without a second worktree is
`git archive <sha> core | tar -x -C temp/master` plus a bench crate pointing
at it; copy `lto = "fat"`, `codegen-units = 1` into the bench crate's own
`[profile.release]`, since a path dependency's profile is ignored.
