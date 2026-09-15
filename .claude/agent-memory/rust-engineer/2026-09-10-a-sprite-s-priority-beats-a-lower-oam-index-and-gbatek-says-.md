### 2026-09-10 - A sprite's priority beats a lower OAM index, and GBATEK says otherwise
- **Context**: Mario Tennis Advance drew its 3-2-1-GO countdown behind the
  target panels it crosses. The countdown is sprites at OAM 9 and 10 with
  priority 0; the panels are OAM 1 and 2 with priority 2.
- **Finding**: this PPU ordered overlapping sprites by OAM index alone, a rule
  that was in the code *and* in a test citing GBATEK and VisualBoyAdvance bug
  #130. Neither citation survives a close read; mGBA and NanoBoyAdvance both
  compare the priority value and nothing else.
- **Application**: full write-up, including why GBATEK is wrong here and what
  would close the question for good, in
  [`.claude/memory.md`](../memory.md) under "GBATEK is wrong about
  sprite-over-sprite ordering".
