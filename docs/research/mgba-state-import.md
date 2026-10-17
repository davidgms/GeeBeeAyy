# mGBA save-state import: decision record

**Decision (2026-10-14): not built.** Save import and export carry GBee's own
states only (PR #91). An mGBA importer is feasible but approximate, costs an
estimated 2 to 4 days, and only helps players coming from mGBA: My Boy! and
Pizza Boy, the most-used Android GBA emulators, use closed, undocumented
formats. Revisit only if players ask for mGBA specifically.

## Core-side findings (rust-engineer, read-only consult)

- Our format (`core/src/savestate.rs`): magic `GBAS`, `SAVE_VERSION` 10,
  versions 6 to 10 restore (11 since 2026-10-14, which appends live state). Fixed-order little-endian fields, ending in the
  ROM CRC-32, which must match.
- Maps directly: registers and banks, CPSR/SPSRs, all memory regions, the I/O
  file, IE/IF/IME, WAITCNT, DMA registers, timer counter/reload/control, FIFO
  bytes, PSG settings, wave RAM, LFSR, battery save memory.
- Needs conversion: PC pipeline offset, timer prescaler phase (aligned to
  `gba.cycles`), and mGBA's scheduler timestamps for video, timers, DMA and
  the APU frame sequencer.
- Biggest risk: a state taken while PC is inside the BIOS (IntrWait, LZ77,
  CpuSet). Our HLE BIOS area is zeros except the IRQ stub at 0x128-0x13C, so
  the BIOS frame would have to be unwound onto the game's SWI, or the game
  hangs. States inside a game IRQ handler work as-is (LR_irq = 0x138 matches
  our stub).
- Our own format also lacks the PPU dot position, the affine reference
  accumulators and the HLE IntrWait flags. That is an existing gap, not one
  specific to import. (Closed by v11, 2026-10-14.)
- Shape if built: a pure `from_mgba(bytes, &Gba)` in `core/` that emits our
  v10 layout and reuses `SaveState::restore`, one FFI function beside
  `gbee_state_write`, synthetic-blob tests in `core/tests/`. The frontend
  inflates the PNG container, since unpacking a file is not emulation logic.

## Format findings (search-specialist)


Method note: sources were fetched with WebFetch, which summarises through a small model. Line numbers it returned
for serialize.c / io.c / video.c etc. are ESTIMATES, flagged "~". Offsets in the header comment are quoted as
returned from the file's own comment. Before coding, re-read the raw files (links below) with real line numbers.

Base: https://github.com/mgba-emu/mgba/blob/master/ (raw: https://raw.githubusercontent.com/mgba-emu/mgba/master/)

## 1. GBASerializedState layout (include/mgba/internal/gba/serialize.h)

- Total size 0x61000 (397,312) bytes, fixed, enforced by a static_assert (header comment "Total size").
- Version: file offset 0x00 = "Version Magic (0x0100000B)". src/gba/serialize.c defines
  GBASavestateMagic = 0x01000000 and GBASavestateVersion = 0x0000000B. Loader: reject if
  > Magic+Version ("Invalid or too new savestate"), reject if < Magic, accept older with
  "Old savestate ... continuing anyway" (serialize.c ~100-108). Version gates seen in code: 0x01000006
  (EXWAITCNT_HI rewrite, io.c), 0x01000007 (video nextEvent / audio lastSample moved, video.c, audio.c),
  a later one for DMA `count` latch (dma.c). So old versions are handled by per-field fallbacks; there is no
  changelog in the header.

Offset map (header comment):

| Offset | Content |
|---|---|
| 0x00000 | version magic; 0x04 BIOS checksum (0xBAAE187F official); 0x08 ROM CRC32; 0x0C master cycles |
| 0x00010-0x1B / 0x1C-0x1F | game title (12) / game code |
| 0x00020-0x5F | r0-r15 |
| 0x60 / 0x64 | CPSR / SPSR |
| 0x68 / 0x6C | cycles since last event / cycles until next event |
| 0x70-0x117 | bankedRegisters[6][7] (42 words) |
| 0x118-0x12F | bankedSPSRs[6] |
| 0x130-0x18B | PSG ch1 (0x130), ch2 (0x144), ch3 incl. wave banks (0x154), ch4 LFSR/envelope (0x17C); each with frame/last-update/next-event counters |
| 0x18C-0x1CB | FIFO A, FIFO B (8 x 32-bit each + internal state) |
| 0x1CC-0x1EF | audio misc (flags, flags2, sample event) |
| 0x1F0-0x1FF | video: 0x1F4 next event, 0x1F8 flags (scanline-callback mode), 0x1FC frame counter |
| 0x200-0x250 | timers 0-3, 0x14 bytes each: reload u16, old reload u16, lastEvent, nextEvent, reserved, flags |
| 0x250-0x28F | DMA 0-3, 0x10 each: nextSource, nextDest, nextCount, nextEvent(when) |
| 0x290-0x2C3 / 0x2C4 | GPIO/RTC/solar/tilt state / SIO next event |
| 0x2C8, 0x2CC, 0x2D0 | latched DMA0 value, last DMA PC, matrix-memory command buffer |
| 0x2E0-0x2F3 | savedata chip state: type, command, flags (flash state machine bits0-1, bank bit4, settling bit5), EEPROM read bits remaining, settling cycles, EEPROM read/write address, flash settling sector |
| 0x2F4-0x2FF | prefetch (cpuPrefetch[2]) |
| 0x310 / 0x318 / 0x31C | global cycle counter (64-bit) / last prefetched PC / misc flags |
| 0x320 / 0x324 | next IRQ event / interruptable BIOS stall cycles |
| 0x328-0x36F | special cartridge union (matrix, unlicensed multicart, VFAME) |
| 0x370-0x3CF | FIFO A/B samples, rendered audio samples |
| 0x3D0 | memory bus value; 0x3D4-0x3E7 latched DMA1-3 values and DMA0-3 counts |
| 0x400-0x7FF | I/O (0x400 bytes) |
| 0x800 / 0xC00 | palette / OAM |
| 0x1000-0x18FFF | VRAM (96 KiB) |
| 0x19000-0x20FFF | IWRAM (32 KiB) |
| 0x21000-0x60FFF | EWRAM (256 KiB) |

Misc flags (0x31C): bit0 halted, bit1 POSTFLG, bit2 IRQ pending, bit3 blocked (DMA CPU block), bits 4-14 key-IRQ keys.
(GBATimerFlags bit layout is not in serialize.h; it lives in the timer header, not fetched.)

Source: https://github.com/mgba-emu/mgba/blob/master/include/mgba/internal/gba/serialize.h
Source: https://github.com/mgba-emu/mgba/blob/master/src/gba/serialize.c (Serialize ~27-93, Deserialize ~95-239)

## 2. Container (src/core/serialize.c)

- Plain .ssN: the raw 0x61000-byte GBASerializedState, then an "extdata" block appended (uncompressed in this path).
  Extdata = table of {tag u32, size i32, offset i64} entries terminated by EXTDATA_NONE, then the blobs
  (mStateExtdataSerialize ~70, Deserialize ~115, struct ~30-34). Tags seen: screenshot (+dimensions), metadata
  (time in microseconds, creator "projectName projectVersion"), savedata, cheats, RTC.
- PNG state (any state saved with SAVESTATE_SCREENSHOT forces PNG): ordinary PNG pixels (screenshot) plus custom
  chunks "gbAs" (state, zlib compress()) and "gbAx" (extdata items; payload = 8-byte header tag+uncompressed
  size, LE, then zlib data) (~184, ~199, reader registered for "gbAs gbAx" ~278, uncompress ~219/246).
- Loader has no magic/game check in the container; checks are in GBADeserialize (section 1): game title/code
  mismatch is FATAL, ROM CRC mismatch warning only, BIOS checksum mismatch fatal only if PC is in BIOS, negative
  or >= 16.78M CPU cycles fatal, no mutation if any fatal check fails.
- Savedata extdata: when present, replaces the save contents on load (SAVESTATE_SAVEDATA flag). The chip
  state machine (above) is in the main struct, the bytes themselves are in extdata.
- Flag values (SAVESTATE_SCREENSHOT/SAVEDATA/CHEATS/RTC/METADATA) are in a header not fetched.

Source: https://github.com/mgba-emu/mgba/blob/master/src/core/serialize.c

## 3. Hardware state (maps 1:1) vs mGBA-specific

1:1, plain hardware (direct copy):
- r0-r15, CPSR, SPSR, banked r8-r14 per mode and banked SPSRs (ordering of the 6 modes must be checked in
  src/arm/ cpu structs: not stated in the header comment).
- IWRAM, EWRAM, VRAM, OAM, palette (note: mGBA restores OAM/palette via GBAStore16, VRAM direct, video.c ~487).
- I/O block: but see caution below.
- Timer reload, enable/prescale/count-up flags (via I/O + flags), DMA source/dest/control (I/O DMAxCNT).
- PSG wave RAM banks, FIFO A/B sample contents, savedata chip type/command/flash bank/EEPROM addresses.
- Halted, POSTFLG, key-IRQ keys.

Needs translation or guessing (mGBA scheduler / internal counters):
- Everything expressed as time: video.nextEvent (relative to mTiming clock, plus the mode flag: 0=derive from
  DISPSTAT hblank bit, 1/3=_startHdraw, 2=_startHblank, video.c ~472-540). Core must map this to the dot/scanline
  position. Scanline position is recoverable: vcount comes from I/O VCOUNT and nextEvent says how far into the
  line, but the exact cycle needs mGBA's HDRAW=960 / HBLANK=272 constants (not fetched, verify).
- Timers: lastEvent and nextEvent are relative offsets (io.c ~690-765). The live counter value is not stored as
  a number; it must be derived from reload, prescale and (now - lastEvent). Doable but precision depends on
  mGBA's timer model (it uses lazy counting). Count-up timers have no scheduled event.
- DMA: `when` is serialised "as-is", an ABSOLUTE timestamp on the mTiming clock (dma.c ~351-365), whereas the
  other events are relative. nextCount carries bit 31 as an "in progress" marker; DMA latch values are
  mGBA's open-bus model.
- Audio: PSG frame sequencer "next frame/last update/next event" are cycle timestamps; FIFO has a read/write
  pointer reset to 0 on load, remaining-samples counts, sample event relative time (audio.c). Need to map to our
  own APU frame-sequencer phase and channel timers; some envelope bit fields are packed weirdly (header TODO:
  ch4 "fields stored big-endian in little-endian field").
- Prefetch buffer and cycles-until-next-event (cpu.cycles/nextEvent), global cycle counter, last prefetched PC,
  BIOS stall, bus value, IRQ event delay: pure implementation detail; our core would have to choose something
  sane (flush prefetch, re-derive pipeline from PC, as mGBA itself does for old states, serialize.c ~192-214).
- SIO next event, GPIO/RTC/solar/tilt (only relevant to a few carts; RTC is also an extdata tag).
- I/O block caution: mGBA stores non-"special" registers through GBAIORead() and some are stored raw
  (_isRSpecialRegister, io.c ~690). So the 0x400 I/O image is "what a CPU read would see", not necessarily
  the raw latch; write-only registers read as 0 on a plain read except those flagged special. Our core must
  re-apply regs through its own write path, as mGBA does on load via GBAIOWrite.

## 4. Do other emulators import mGBA states?

- Found NO evidence of any other emulator reading mGBA states. RetroArch's mGBA libretro core uses mGBA's
  own serializer (it is mGBA), so it is native, not "import". A web search for NanoBoyAdvance/SkyEmu/VBA-M
  importers returned nothing. Searches did not cover those projects' repos directly, so absence is "not found",
  not "proven".
- mGBA forum search snippets (secondary, unverified by me; the one thread I fetched,
  https://forums.mgba.io/showthread.php?tid=5708, did NOT contain the claim) say savestates are not compatible
  between emulators. A guide says .ss1 cannot be loaded in VBA:
  https://www.pokemoncoders.com/importing-saves-in-visual-boy-advance/
- Battery .sav is the portable artifact; Pizza Boy .sav files are noted as incompatible with mGBA's raw format:
  https://git.tdem.in/euan-forrester/save-file-converter
  https://www.pokemoncoders.com/transfer-gba-save-files/

## 5. Android GBA emulators and their state formats

- Search found only that My Boy! (com.fastemulator.gba), Pizza Boy GBA (it.dbtecno.pizzaboygba) and John GBA all
  offer save states and keep .sav under their app dir; NO source documented their state formats. Not verified:
  treat all three as proprietary, closed-source, undocumented. mGBA (Android port, same core) uses the
  format above. Whether mGBA is "the common format": no evidence; the evidence points the other way (each
  emulator has its own).
- Next step if wanted: look at the Pizza Boy/My Boy! state files with a hex dump (private to the user's device).

## Verdict

An mGBA importer is feasible but is a best-effort "approximate resume", not a faithful restore. Plain hardware
state (CPU registers incl. banks, 288 KiB of RAM, VRAM/OAM/palette, I/O image, wave RAM, FIFO contents, flash/EEPROM
state, save bytes from the extdata block) maps almost directly, and the container is simple (raw struct + extdata
table, or zlib chunks gbAs/gbAx in a PNG). The hard 20% is everything mGBA stores as scheduler timestamps
(scanline position, timer counters, DMA progress, PSG frame sequencer, FIFO phase, prefetch/bus state): each must
be converted from mGBA's lazy mTiming model into our core's counters, and no reference implementation exists to
copy. Expect states to load and run in most games, with occasional audio glitches, one-frame video/IRQ phase
errors, or mid-DMA hangs; states taken during DMA or timer-critical code are the riskiest. Cost estimate: a day
for container + 1:1 fields, then several more for timing translation plus tests built from mGBA-saved states
(only the user can supply states; ROMs/BIOS never go in the repo). Risk is moderate on correctness, low on
architecture (a read-only importer in core/ behind an FFI function, nothing else changes). Version handling:
accept 0x0100000B (current) only; older versions have per-field fallbacks that you would not want to port.
Unverified items to check against raw source before building: banked-register mode ordering, GBATimerFlags bit
layout, mGBA's exact HDRAW/HBLANK constants, the DMA count-latch version gate.
