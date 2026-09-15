### 2026-08-28 - Proving "the test comes first" after the fact
- **Context**: same task; the standing rule is the reproducing test is written
  before the fix, which a single-pass agent cannot demonstrate from its diff.
- **Finding**: copying the edited source to the scratchpad, `git checkout`ing
  the file, running the new tests against the unmodified core and then copying
  the file back gives the same evidence in one command. 14 of 16 new cases
  failed on the old core; the 2 that passed pinned down exactly which paths were
  already correct (the THUMB SWI dispatch and `ArcTan(0)`).
- **Application**: do this for every decoder or BIOS change. A test that passes
  against the unfixed code is not a regression test, and the count of
  before/after failures is the cheapest proof the fix is real. Watch the working
  directory: `cd core` persists inside a compound command, so restore with
  absolute paths.
