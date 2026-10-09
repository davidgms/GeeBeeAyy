---
name: phase5-probes-and-memory-cap
description: Hostile-input probe crate at temp/phase5/probes and the mandatory prlimit cap; random state fuzz only ever hits sample_accum
metadata:
  type: feedback
---

Run every probe/test binary as `prlimit --as=4000000000 timeout 600 <cmd>`.

**Why:** a save-state probe without the cap grew to 15.7 GB, got OOM-killed and
restarted WSL three times (2026-10-08). Under the cap a runaway allocation is a
clean abort ("memory allocation of N bytes failed"), and that abort is a finding.

**How to apply:**
- Probe crate: temp/phase5/probes (modes swi, rom, romfuzz N, statefuzz N FIRST,
  fields, edge, stateseed N, byte AT HEX). Findings in temp/phase5/core.md.
- `timed()` leaks the worker thread on TIMEOUT, so one abort kills the run;
  resume with `statefuzz N FIRST`. The last log line names the seed *before*
  the culprit.
- 500 random state seeds only ever found `sample_accum` (bytes 511896/511897).
  Restored values with a single bad value (timer reload == 0x10000, cycles near
  u64::MAX) need targeted edits (`edge` mode) - the fuzzer never hits them.
- `Gba::step` publishes TMxCNT_L via `timer.counters()` every step, so a
  timer field bug panics immediately even with a ROM that never reads timers.

**Later the same day (emu-core lane):**
- Probe mode `emu CASE` (foreign, eeprom, pc, modes, lz77, video N, edges,
  flash, ppuflags, romfail, rewind, dma, versions); results in
  temp/phase5/emu-core.md. Build the probe crate without `--release` too:
  the debug build is what surfaces overflow panics (bios.rs:406/481/538).
- ASan cannot run under the 4 GB `--as` cap (it reserves far more virtual
  space). For C (rcheevos) use a plain `gcc -O1 -g ... -lm` host build and
  `timeout` for hangs: temp/phase5/rc-harness/harness.c compiles the vendored
  rcheevos sources directly (`src/rcheevos/*.c src/rc_compat.c src/rc_util.c
  src/rhash/md5.c`). Set `TMPDIR=$PWD` so gcc does not write to /tmp.
- The vendored rcheevos is a real git checkout: `git log --oneline | grep
  '#NNN'` settles whether an upstream PR is in the pin.
