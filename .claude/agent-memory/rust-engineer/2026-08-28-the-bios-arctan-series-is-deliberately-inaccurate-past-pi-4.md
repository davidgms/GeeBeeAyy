### 2026-08-28 - The BIOS arctan series is deliberately inaccurate past PI/4 (WRONG, corrected 2026-10-04)
- **Correction (2026-10-04)**: this was false. The divergence (0x59B3 for
  tan = 1.0) came from three wrong coefficients in our HLE, not from the
  BIOS. Hardware (mGBA suite table) gives 0x2000. The real coefficients and
  side outputs are in `.claude/memory.md`, 2026-10-04 BIOS math entry.
- **Lesson**: "cross-checked against mGBA" was never actually done; a
  hardware table beats a plausible GBATEK remark. Diff constants digit by
  digit against the reference before building a theory on their output.
