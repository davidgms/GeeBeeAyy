### 2026-08-30 - The bus cannot own a DMA write, and the reason is the borrow
- **Context**: same task, deciding where the register writes should be applied.
- **Finding**: `Dma::write_control` runs an immediate transfer inline and needs
  `&mut MemoryBus` - exactly the borrow `MemoryBus::store8` is already holding.
  So the bus cannot apply the write itself. `MemoryBus` now pushes the channel
  index into a private `dma_writes: Vec<usize>` and `Gba::step` drains it via
  `apply_dma_writes`, the same shape as the existing `sound_writes` queue.
  Two details that make it correct rather than merely compiling: capture on the
  **high byte** of DMAxCNT_H (`0xBB | 0xC7 | 0xD3 | 0xDF`, then
  `(offset - 0xBB) / 12`), because `write16`/`write32` decompose into ascending
  byte stores so the high byte is the last one and always sees a complete
  control word; and drain **before** `ppu.tick`, so a channel enabled by this
  instruction is configured for an HBlank or VBlank landing in the same step.
  SAD/DAD/CNT_L latch only on the enable edge (`control & 0x8000 && !enabled`),
  matching the hardware latch - otherwise rewriting CNT_H rewinds a running
  channel's advanced address.
- **Application**: the "bus records it, the owner applies it" queue is the
  general answer in this codebase whenever a bus write needs the bus back.
  Reach for it before trying to restructure ownership.
