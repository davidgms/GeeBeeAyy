### 2026-08-30 - DMA was the sixth "complete but unwired" subsystem, and it hid two decode bugs
- **Context**: wiring `Dma` to `MemoryBus`; `write_sad`/`write_dad`/`write_count`/
  `write_control` had no caller outside `dma.rs` and `core/tests/interrupts.rs`.
- **Finding**: routing the writes was the easy half. Making DMA *live* exposed
  two decode bugs in `core/src/dma.rs` that had been harmless only because
  nothing ever ran:
  1. `decode_control` read `src_adj` from bits 5-6 and `dst_adj` from bits 7-8.
     GBATEK has them the other way round (5-6 dest, 7-8 source). Cross-check:
     the canonical sound-FIFO control word is `0xB640` - enable | timing 3 |
     32-bit | repeat | **dest fixed = bits 5-6 = 2**. With the swap, a FIFO DMA
     would have walked the destination through I/O space while re-reading one
     source word.
  2. `write_count` did `if value == 0 { 0x10000 } else { value as u32 } as u16`.
     The `as u16` truncates 0x10000 straight back to 0, and `do_transfer` then
     returned early on `count == 0` - so every maximum-length transfer was a
     no-op. The expansion has to happen at use, not at store: the maximum does
     not fit the field.
  Also wrong: `do_transfer` reloaded the **source** on repeat, which flattens
  any per-scanline HDMA table to a single value. Hardware reloads only the
  destination, and only in Increment/Reload mode.
- **Application**: when you wire up a subsystem that has never executed, treat
  its entire decode path as unreviewed code, not as working code that merely
  lacked a caller. Budget for the bugs the wiring reveals.
