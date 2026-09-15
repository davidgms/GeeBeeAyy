### 2026-08-28 - The BIOS arctan series is deliberately inaccurate past PI/4
- **Context**: implementing `ArcTan` (SWI 0x09) and `ArcTan2` (0x0A).
- **Finding**: GBATEK's "there is a problem in accuracy with THETA<-PI/4,
  PI/4<THETA" is not a rounding remark. The BIOS polynomial diverges hard: at
  tan = 1.0 (`r0 = 0x4000`) it returns 0x59B3 where the true answer is 0x2000.
  `ArcTan2` avoids it by folding the quadrant so the series only ever sees
  |ratio| <= 1, but the exact diagonal x == y still lands on the bad point.
- **Application**: a test for either SWI must stay off the diagonals - 0x1000
  and 0x2000 are inside the accurate band (within 9/65536 of `atan2`). Do not
  "fix" the divergence: games calibrate against real hardware.
