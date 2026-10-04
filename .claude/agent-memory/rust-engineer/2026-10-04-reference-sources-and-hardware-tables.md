---
name: reference-sources-and-hardware-tables
description: Where to get reference emulator source without auth, and turning a suite's hardware table into a test; mGBA is not always the reference
metadata:
  type: reference
---

- `curl -sSL https://api.github.com/repos/<o>/<r>/git/trees/master?recursive=1`
  lists a repo's files without auth (the code-search API needs auth), then
  `curl -sSLO https://raw.githubusercontent.com/...` per file. Worked for
  mgba-emu/suite, mgba-emu/mgba, zaydlang/multiplication-algorithm.
- **Check the licence before porting anything.** GeeBeeAyy is MIT. mGBA is
  MPL-2.0 and NanoBoyAdvance GPL-3.0: read them for behaviour, never copy or
  translate their code. On 2026-10-04 a port from NBA had to be rewritten
  from the zlib original and the branch history rewritten. Find the original
  author's repo (often zlib/MIT), port from that, put its notice verbatim in
  the source and in `THIRD_PARTY_NOTICES.md`.
- **mGBA is not the accuracy ceiling.** It does not model the multiply carry
  flag (fails 20 of its own suite's multiply-long rows).
- The mGBA suite's `src/*.c` tables are hardware-recorded. A regex over the
  `{ "name", { ... } }` rows (python in `temp/`) turns one into a Rust const
  table for `core/tests/` - all rows, not a sample; that is how bios.rs and
  cpu.rs pin BIOS math and MULL. Clippy wants a `type Row = (...)` alias past
  ~8 tuple fields (`type_complexity`).
- Running `temp/check.sh` in the background while editing `core/tests/` lets
  the later (release) half compile the half-written test: park the edit
  (`cp` to temp, `git show HEAD:path > path`) until the gate finishes.
