# Memory wait states, prefetch and DMA stalls - design

Status: proposal, 2026-10-07. Base: `master` plus branch
`fix/mgba-suite-sio-misc` (save state v9). Nothing here is implemented yet.

Goal: move the mGBA suite "Timing tests" sub-suite off 236/2020, the last big
gap of Phase 4.5, without making games or phones worse.

Licence rule (see `.claude/memory.md`, Hardware references): mGBA (MPL-2.0)
and NanoBoyAdvance (GPL-3.0) are behaviour references only. Everything below
is derived from GBATEK and from the suite's own hardware table
(`src/timing.c`, MIT). No code is to be ported from either emulator.

---

## 1. Current state

### How a cycle is counted today

- `Cpu::step` (`core/src/cpu/mod.rs:154-191`) fetches with a plain
  `bus.read16/read32(pc)` and returns whatever the decoder returned plus
  `entry_cycles` (exception entry 3, `mod.rs:567`; HLE BIOS tails).
- Every decoder arm returns a **fixed constant that ignores the address**:
  - ARM data processing 1, or 3 when Rd = PC (`cpu/arm.rs:262-266`); no +1I
    for a register-specified shift.
  - MUL 4 and MLA 4 (`arm.rs:312-313`), every long multiply 5 (`arm.rs:355`),
    whatever the operand. GBATEK: 1S+mI / 1S+(m+1)I / 1S+(m+1)I / 1S+(m+2)I.
  - LDR 3 (5 to PC), STR 2 (`arm.rs:750-752`); LDM 2+n (+2 to PC), STM 1+n
    (`arm.rs:937-943`); SWP 4; B/BX 3; SWI 3 (`arm.rs:17-20`).
  - THUMB: format 4 ALU always 1 (`thumb.rs:149`, so `mul` and register
    shifts are 1); **format 5 hi-register ops always 3**
    (`thumb.rs:150-153`), so `nop` (`mov r8, r8`) costs 3 cycles. That is a
    plain bug, independent of wait states.
- In other words **the whole machine runs as if every access hit IWRAM**:
  1 cycle per S, N, I, any width, any region, WAITCNT ignored.
- `Gba::step` (`core/src/lib.rs:111-210`) ticks timers, SIO, PPU and APU
  with that total *after* the instruction. I/O reads see the machine at the
  start of the instruction; timer register writes apply after the tick
  (`lib.rs:174-179`). `TMxCNT_L` is published one count ahead
  (`timer/mod.rs:150`, `counter_after_one_cycle`) - a fit that, in effect,
  places an IWRAM `ldr`'s read one cycle into the instruction.

### WAITCNT

- Stored as `MemoryBus::waitcnt: u16` (`memory/mod.rs:25`), written by
  `store8` (`memory/mod.rs:651-655`), saved and restored in the save state
  (`savestate.rs:127`, `:294`), cleared by `reset`.
- `MemoryBus::read_cycles` / `write_cycles` (`memory/mod.rs:738-800`) are
  **dead code** (no caller anywhere) and wrong where they are specific: they
  map WS0 first access 0..3 to 3..6 (GBATEK: 4,3,2,8 waits plus 1), ignore
  the S/N split and the second-access bit, charge IWRAM 32-bit reads 2
  (it is 1), and VRAM 16-bit writes 2 (it is 1). Delete them; do not revive.

### DMA

- `Dma::do_transfer` (`dma.rs:164-251`) moves the whole block inside the
  call and **costs nothing**: immediate DMA runs from `apply_dma_writes`
  (`lib.rs:245`), HBlank/VBlank DMA from inside `Ppu::tick`
  (`ppu/mod.rs:279`, `:318`), sound DMA from `post_tick` (`lib.rs:321-339`).
  The CPU is never stalled.

### Fitted constants that touch timing

| Constant | Where | Fitted on | Code region during the fit |
|---|---|---|---|
| `IRQ_DELAY = 4` | `lib.rs:29` | Timer IRQ test | IWRAM (`timer-irq.c:38` `IWRAM_CODE`) |
| `INTR_WAIT_RETURN_CYCLES = 45` | `bios.rs:116` | Timer count-up | IWRAM (`timers.c:430`) + BIOS |
| `HALT_RETURN_CYCLES = 30` | `bios.rs:127` | SIO timing | IWRAM (`sio-timing.c:44`) + BIOS |
| exception entry 3 | `cpu/mod.rs:567` | Timer IRQ | vector in BIOS |
| timer start delay 1, read one ahead | `lib.rs:307`, `timer/mod.rs:150` | Timer IRQ | IWRAM |

All of them were fitted with code in IWRAM and the BIOS, both 1-cycle
regions. **A wait-state model that keeps IWRAM and BIOS at 1 cycle per
access leaves every one of them valid**, provided the refactor is exactly
neutral in those regions. That is the design's main safety property.

### What the games actually run (measured, `temp/waitprobe`, 600 frames)

| ROM | WAITCNT set | Steps/frame | Halted cycles/frame | Code region |
|---|---|---|---|---|
| mariotennis.gba | 0x4017 (WS0 3/1, prefetch on) | 142.5k | 0 | 94% ROM, 94% THUMB |
| yggdra.gba | 0x4314 (WS0 3/1, prefetch on) | 42.5k | 199.9k of 280.9k | 58% IWRAM, 38% ROM, 4% BIOS |

Both enable the prefetch buffer, as GBATEK says nearly every cartridge does
(`WAITCNT=4317h`). Mario Tennis never halts under today's model.

---

## 2. What the hardware does

Sources: GBATEK "GBA Memory Map", "GBA System Control" (WAITCNT),
"GBA GamePak Prefetch", "ARM CPU Instruction Cycle Times", "GBA DMA
Transfers". Local copies in `temp/gbatek/`.

### Region timings (cycles for 8/16/32-bit)

| Region | Bus | Cycles |
|---|---|---|
| BIOS, IWRAM, I/O, OAM | 32 | 1/1/1 |
| EWRAM | 16 | 3/3/6 |
| Palette, VRAM | 16 | 1/1/2 |
| GamePak ROM WS0/1/2 | 16 | N or S per WAITCNT; 32-bit = N16 + S16 (or 2 x S16) |
| GamePak SRAM | 8 | 1 + SRAM wait |

Palette/VRAM/OAM gain +1 when the PPU is using them at the same time.
Declined (section 5): no test here measures it and modelling it needs
per-dot PPU timing.

### WAITCNT (0x04000204)

Bits 0-1 SRAM (4,3,2,8 waits); 2-3 WS0 N (4,3,2,8); 4 WS0 S (2,1); 5-6 WS1 N;
7 WS1 S (4,1); 8-9 WS2 N; 10 WS2 S (8,1); 14 prefetch enable; 15 read-only
cart type. Access time is 1 + waits. Default 0x0000.

Two rules that are easy to miss:

- **A 32-bit gamepak access is two 16-bit accesses, the second always S.**
- **Sequential access is forced to N at every 128 KiB boundary of ROM.**

### N, S, I per instruction

GBATEK's table: ALU 1S (+1I shift by register, +1S+1N to PC), LDR
1S+1N+1I, STR 2N, LDM nS+1N+1I, STM (n-1)S+2N, SWP 1S+2N+1I, B/BL 2S+1N,
THUMB BL 3S+1N, SWI/exception 2S+1N, MUL 1S+mI, MLA 1S+(m+1)I, MULL
1S+(m+1)I, MLAL 1S+(m+2)I. `m` is 1..4 from the multiplier's top bytes
(all-zero or all-one for signed; all-zero only for UMULL/UMLAL). Code cycles
take the code region's timing, the 1N of a data access (and the (n-1)S of
LDM/STM) take the data region's.

**Prefetch Disable Bug** (GBATEK): with prefetch off, a ROM opcode with
internal cycles that does not write R15 turns its following opcode fetch
from S into N.

### Prefetch buffer

Up to eight halfwords, opcode fetches from ROM only. It fills while the CPU
is not using the gamepak bus (internal cycles, accesses to other regions),
and a fetch that hits it costs 1 cycle. A branch, or any data access to the
gamepak, ends the sequential run.

### DMA

`2N + 2(n-1)S + xI`, x = 2, or 4 when both ends are in the gamepak. The CPU is
stopped during the transfer. After an enable edge "wait 2 clock cycles before
accessing any DMA related registers" - there is a start latency.

### Verified convention (hand-checked against the suite's table)

A per-instruction model reproduces the table's prefetch-off columns exactly
with this one convention:

> Every instruction pays one code fetch (pc + 2L) as S, **or N if the previous
> cycle was a data access, a refill, or (ROM, prefetch off) an internal
> cycle**. A write to PC adds a refill N + S at the target, in the target's
> region.

The suite measures `value(test) - value(calibration)`. Both share the same
START (`str` to TM0CNT) and END (`ldrh` from TM0CNT_L), so the measured value
is `cost(CODE) + fetch(END after CODE) - fetch(END after START)`, and START
always leaves END's fetch N. Worked cells (WAITCNT 0x0000: ROM N16 = 5,
S16 = 3, so ARM N32 = 8, S32 = 6):

| Test | Column | Derivation | Table |
|---|---|---|---|
| nop | ARM/ROM ... | N8 + (S6 - N8) | 6 |
| ldr r2, [sp] | ARM/ROM ... | N8 + 1 + I1 + (N8 - N8) | 10 |
| ldr r2, [sp] | ARM/ROM ..S (S16 = 2) | N7 + 1 + 1 | 9 |
| ldrh r2, [0x08000000] | ARM/ROM ... | N8 + N5 + 1 | 14 |
| ldr r2, [0x08000000] | ARM/ROM ... | N8 + (N5 + S3) + 1 | 17 |
| ldr r2, [0x08000000] | ARM/IWRAM | 1 + 8 + 1 | 10 |
| mul #0, #0xFF | ARM/ROM ... | N8 + I1 + disable-bug N | 9 |
| b 1f; nop; 1: nop | ARM/ROM ... | N8 + (N8 + S6) + S6 + (S6 - N8) | 26 |
| Trivial loop | ARM/ROM ... | 8 + 6 + 16x12 + 15x20 + 6 - 2 | 510 |
| Trivial loop | ARM/WRAM | 12 + 192 + 270 + 6 + 0 | 480 |
| mul #X | ARM/IWRAM | 1 + m(X) | 2,2,3,4,5,4,3,2,2,2 |
| umull #X | ARM/IWRAM | 1 + m(X) + 1, unsigned m | 3,3,4,5,6,6,6,6,6,6 |
| Trivial DMA (16) | ARM/ROM ... | str 9 + DMA (2N + 2I = 4) | 13 |

The prefetch-on columns cannot be derived by hand this way and need the
buffer model.

### The suite's timing table, grouped

20 columns per THUMB-capable test (10 per mode): ROM with WAITCNT 0x0000,
0x4000, 0x0004, 0x4004, 0x0010, 0x4010, 0x0014, 0x4014, then EWRAM and IWRAM
(both at WAITCNT 0). Four of the eight ROM columns have prefetch on. Test
code in `src/tests/*.s`, macros in `include/macros.s`.

| Family | Tests | Cells | Passing now* | Needs |
|---|---|---|---|---|
| Calibration | 1 | 20 | 4 | intra-instruction position of the timer write/read |
| nop, nop/nop | 2 | 40 | 12 | THUMB hireg fix, code N/S |
| ldr/ldrh/str/strh to [sp] combos | 15 | 300 | 52 | data N, next-fetch N, prefetch |
| loads from ROM | 9 | 180 | 0 | gamepak data N/S, prefetch flush |
| ldmia/stmia sp (ARM only) | 12 | 120 | 32 | LDM/STM nS+N, prefetch |
| ldmia OAM -> ROM overflow | 5 | 100 | 10 | region crossing, 128K N rule, prefetch |
| mul/mla/smull/smlal/umull/umlal | 60 | 700 | ~16 of 160 visible | `m` cycles, disable bug, prefetch |
| b, nop;b, bx | 3 | 60 | ? | refill N+S in target region |
| Trivial loop, C loop | 2 | 40 | ? | refill, C loop: prefetch |
| BIOS Div/Sqrt/ArcTan | 6 | 120 | ? | real BIOS body cost |
| CpuSet | 1 | 20 | ? | real BIOS body cost |
| DMA (trivial/short, 16/32, ROM/RAM ends) | 16 | 320 | IWRAM column | DMA stall, start latency |
| **Total** | **132** | **2020** | **236** | |

*From the suite's SRAM log (`temp/probe`, sub-suite 2). The log is 64 KiB
and truncates inside the multiply tests, so later families are unknown
individually; the 110 cells not attributed above are in them.

How many cells are reachable without prefetch: in 180 of the 808
prefetch-on cells the hardware value equals the prefetch-off value (branches,
loops, ROM data loads, BIOS - nothing for the buffer to do), so a model with
no buffer still passes those.

---

## 3. Design options

### Option 0 - scale costs per region (rejected)

Multiply today's constants by a per-region factor. Cheap, but gets almost no
cell (the table is exact to one cycle) and still makes games' speed wrong in
both directions. Listed only to close it.

### Option A - per-access N/S wait states, no prefetch buffer

**What changes where**

- `core/src/memory/timing.rs` (new, small): a wait table indexed by
  `address >> 24 & 0xF` x {N, S} x {16, 32}, rebuilt on every WAITCNT write
  and **after save-state load and reset** (`savestate.rs:294` assigns
  `waitcnt` directly today; make it a setter). EWRAM 3/6, palette/VRAM 1/2,
  ROM per WAITCNT, SRAM per WAITCNT, everything else 1.
- `MemoryBus` gains `cycles: u32` (this step's accumulator) and
  `next_fetch_n: bool`, plus three charge-only methods the CPU calls:
  `charge_fetch(addr, width)`, `charge_data(addr, width, seq)`,
  `charge_idle(n)`. They **do not wrap** `read*/write*`: the 51 HLE BIOS and
  the DMA bus calls must stay free, so only the CPU charges.
- `Cpu::step` charges the fetch of pc + 2L, and after the instruction, if
  `branched`, the refill N + S at the new PC. `execute_arm/thumb` stop
  returning totals; each arm charges only its data accesses and I cycles. The
  per-instruction "3 when PC is written" special cases disappear into the
  central refill.
- HLE SWI: entry stays 3 (BIOS region is 1 cycle), and the return is charged
  as a refill in the caller's region. In IWRAM that is the same 3 total as
  today; from ROM it adds the real return refill.
- Prefetch on is treated as off, **without** the disable bug (so games with
  prefetch on are not penalised twice).
- Fix the CPU-internal counts on the way: `m` for every multiply, +1I for a
  register-specified shift (ARM and THUMB format 4), THUMB hireg 1 (+refill
  when Rd = PC), THUMB `mul`.
- DMA stall (can be done in either option): `do_transfer` returns
  `2N + 2(n-1)S + xI` from the same table; the bus adds it to a `stall`
  accumulator that `Gba::step` consumes after the instruction, ticking
  timers/PPU/APU over it like any other cycles. HBlank/VBlank DMA raised
  inside `Ppu::tick` is charged the same way on the next step. Sound DMA
  (4 words, ~10 cycles) too.

**Expected score** (prefetch-off columns plus the prefetch-on cells whose
hardware value equals the prefetch-off one):

| Family | Cells | Option A |
|---|---|---|
| Calibration | 20 | 4 (needs the intra-instruction piece, see A+) |
| nop | 40 | 40 |
| [sp] loads/stores | 300 | 180 |
| ROM loads | 180 | ~160 |
| ldm/stm sp | 120 | 72 |
| ldm overflow | 100 | ~60-80 |
| multiply | 700 | 420 |
| branches | 60 | 60 |
| loops | 40 | 32 |
| BIOS, CpuSet | 140 | 0 |
| DMA | 320 | ~100-190 (IWRAM column needs the start latency) |
| **Total** | **2020** | **~1130-1240** |

**Risk to games.** Audio is the clock: a frame is always 280,896 emulated
cycles and the frontend blocks on `AudioTrack` writes, so the audio rate and
the frame rate do not move. What moves is how many instructions fit in a
frame. Today ROM THUMB code runs at ~1 cycle per instruction where hardware
needs ~2 (WS0 S = 2 cycles per halfword) and ARM-in-ROM ~4, so **games
currently get roughly 1.5-4x more CPU than a GBA**. With waits, game logic
per frame drops to what the hardware allows:

- A game that fits its frame on hardware (the normal case) is unaffected
  except that it spends more of the frame running and less halted.
- A game that overran on hardware now lags in-game exactly as on hardware
  (slower gameplay, same audio pitch - the sound DMA is timer-driven, not
  CPU-driven). Today such a game runs "better than hardware"; after the
  change players may perceive it as a regression. This is correct
  behaviour, but worth saying in the PR.
- **Option A specifically makes prefetch-on games slower than hardware**
  wherever the buffer would have hidden waits behind I cycles or RAM
  accesses (loads to IWRAM stacks, multiplies, LDM/STM - the bread and butter
  of compiled THUMB). Typical overstatement is 10-30% of ROM-code time. A game
  that fits its frame on hardware with less than that margin would lag here
  and not on a GBA. This is the strongest argument against stopping at A.
- Mid-frame effects: an HBlank handler in ROM now finishes later. The PPU
  draws a line at cycle 960 (`ppu/mod.rs:295`), and the memory entry "A
  scanline drawn at the end of its own line eats the HBlank handler's writes"
  shows this is load-bearing. Raster effects must be checked on screen.
- DMA stall: a VBlank DMA of a 1 KiB OAM copy costs ~520 cycles of CPU, a
  16 KiB VRAM copy ~8k-16k. Games budget for this on hardware; today they get
  it free.

**Host and phone cost.** The per-step work grows by a table lookup per
access (two or three per instruction) and a branch on `branched`:
estimated +2-5 ns on a ~43 ns step (Mario Tennis host, `.claude/memory.md`
2026-10-02). But steps per frame fall wherever the CPU was spinning: Mario
Tennis executes 142.5k steps a frame and never halts, so with ~1.5-2x cycles
per ROM instruction it should drop to ~75-95k steps and get **cheaper** per
frame. Yggdra halts 71% of the frame; its step count stays near 42.5k and it
pays the per-step overhead, ~+5%, on a 4.5 ms frame. The Mi 10T Pro runs at
106-110% of a core today (ROADMAP Phase 4.5), so +5% on the cheaper game is
within budget; measure, do not assume.

**Save state.** WAITCNT is already in the state. `next_fetch_n`, the step
accumulator and the DMA stall are per-step transient (like `irq_since`,
`lib.rs:45`) and are not saved; loading a state costs at most one wrong fetch
type. No version bump. The wait table must be rebuilt on load (above).
Rewind uses save states and inherits that.

**Constants.** IWRAM/BIOS stay 1 cycle, so `IRQ_DELAY`, 45, 30, the entry 3
and the timer fits are untouched **if** commit 1 is exactly neutral. Verify:
Timer IRQ 90/90, Timer count-up 936/936, SIO timing 4/4, Misc. edge 6/12
unchanged after every commit (the gate in section 4).

### Option B - Option A plus the prefetch buffer

**What changes on top of A**: a `Prefetch` struct in the bus, active only
while the CPU executes from ROM with WAITCNT bit 14 set:
`next: u32` (address of the next halfword it will fetch), `count: u8`
(0..8 halfwords buffered), `progress: u8` (cycles into the halfword in
flight). Rules, from GBATEK, fitted against the table's prefetch columns:

- `charge_idle(n)` and any `charge_data` to a non-gamepak region advance the
  buffer by those cycles: one halfword per S16 of the fetch's region, up to
  eight.
- `charge_fetch(addr)`: if `addr` is buffered, 1 cycle per halfword (ARM
  takes two); if it is the one in flight, the remaining cycles; otherwise the
  normal N/S cost and restart from there.
- A refill (branch) or a `charge_data` to the gamepak empties the buffer. What
  exactly an in-flight fetch costs a gamepak data access is not in GBATEK;
  fit it to the "ROM loads" and "ldm overflow" prefetch columns (the
  plus-or-minus-one cells, e.g. ldm overflow 1: ARM ..S 28 but P.S 29).
- With the buffer on, the disable bug does not apply.

**Expected score**: A plus most of the 808 prefetch-on cells: [sp] +120,
multiply +280, ldm/stm +48, ROM loads +~16, overflow +~20, loops +8, DMA
+~60. **~1550-1700.**

**Risk to games**: lower than A. Prefetch-on games run at their hardware
speed instead of slower; this is the configuration that matches what games
were tuned on. The residual risk is in the edge rules (a wrong flush rule is
a few cycles per ROM data load) - small and symmetric.

**Host cost**: a second branch per fetch, taken only for ROM code with
prefetch on; the buffer is three bytes of state. Measure on Mario Tennis,
which is 94% ROM THUMB and the worst case.

**Save state**: the buffer is transient and is not saved (flush on load), so
still no version bump. If a future test proves a state-load mismatch
visible, add it to v10 then.

**Constants**: unaffected (IWRAM code never touches the buffer).

### Option A+ - sub-instruction timing for I/O (Calibration, DMA latency)

Two families need to know *where inside an instruction* an access happens:
the Calibration row (absolute value of START/END) and the DMA tests' IWRAM
and prefetch columns (hardware value 2 = the DMA starts after END's read,
because of the start latency). The test cells cancel the read position out;
these do not.

**What changes**: the timer moves into `MemoryBus` (or a copy of its state
does), so a `TMxCNT_L` read computes the counter at
`step_start + bus.cycles` - the cycles charged so far in this instruction -
instead of the "one count ahead" publication. Timer writes apply at the
write's cycle, not after the tick. DMA gets `start_at = now + latency` and
runs when the CPU passes it.

**Gain**: Calibration +16, DMA +~60-100. **Risk**: this replaces the timer
fits (`counter_after_one_cycle`, write-after-tick, `delay_start(1)`) with a
model meant to explain them; the IWRAM tests must come out identical and the
memory entry on 2026-10-02 shows how long those took to converge. It also
puts back the per-step timer work that was optimised out in #62 unless reads
are computed lazily. The memory note on 2026-10-02 called moving the timer
into the bus "not worth it for ~3%"; here it buys ~100 cells, still not
much for the risk.

### Option C - cycle-level scheduler (declined)

A per-access event scheduler (PPU, timers, DMA, IRQ all evaluated between
bus cycles), the way accuracy-first emulators work. Gets the Calibration
row, the DMA latency and the remaining edge cells for free, but is a rewrite
of `Gba::step`, re-opens every fitted constant, and costs per-cycle host
work on phones. The gain over B + A+ is perhaps 50-100 cells. Declined.

---

## 4. Test plan

The suite's table is the ground truth, and no ROM or BIOS is committed.

### `core/tests/timing.rs` (new) - the suite's method, hand-assembled

Reproduce the suite's harness, not just its instructions, because the
result is a timer difference:

1. Write a test body into ROM image bytes (`load_rom` of a hand-built image,
   as `tests/integration.rs` does), IWRAM and EWRAM: `str r1, [r0]` to
   TM0CNT (prescaler 1, enable), CODE, `ldrh r2, [r0]`, then a terminator.
2. Set WAITCNT, run through `Gba::step` until the terminator, read r2.
3. Run the same with empty CODE (calibration) and subtract.
4. Embed the hardware rows from `src/timing.c` as a table (the suite is MIT;
   keep its notice in the test header and in `THIRD_PARTY_NOTICES.md` if
   rows are copied verbatim).

Run it per commit with the families that commit claims. Start with: nop,
ldr/str [sp], ldr [ROM], mul/umull (four `m` values), b, trivial loop; then
the prefetch columns; then DMA. Each row is ARM and THUMB, ten columns each.

### `core/tests/cpu.rs` - internal cycles

One case per fixed encoding, run from IWRAM: THUMB `mov r8, r8` costs 1;
`mul` with Rs = 0x78 / 0x5678 / 0x345678 / 0x12345678 costs 2/3/4/5;
`umull` vs `smull` with Rs = 0xFF000000 differ; `add r0, r0, r1, lsl r2`
costs 2. Written before the fix, as CLAUDE.md requires.

### Equivalence gate for the refactor (commit 1)

With the flat table (everything 1 cycle) the new accounting must equal the
old constants, minus the known-wrong ones. A test steps a list of encodings
from IWRAM and asserts per-instruction cycles; and the mGBA suite total on
this branch must stay **5208/6998, every sub-suite identical**.

### Regression gate after every commit

- `cargo test` (all), `cargo clippy --all-targets`,
  `cargo check --target aarch64-linux-android`.
- mGBA suite score report (`homebrew_suites.rs`, `mgba_suite_score`):
  Timer IRQ 90/90, Timer count-up 936/936, SIO timing 4/4, Memory 1552, DMA
  1244, I/O read 130 must not move. Only Timing (and maybe Misc. edge) may.
- gba-suite, ARMWrestler, FuzzARM unchanged (they are correctness gates,
  timing-agnostic, but they run from ROM and will now run slower; check the
  frame budgets in their harnesses still suffice).

### Commercial ROMs (`temp/roms/`, not committed)

- `mariotennis.gba` and `yggdra.gba`: steps per frame, halted cycles per
  frame and host ms per frame, interleaved A/B with `temp/ab.sh`
  (`temp/waitprobe` prints the first two). Expect Mario Tennis steps down
  sharply, Yggdra roughly flat with fewer halted cycles.
- Screens: Yggdra title (alternating-frame transparency), the battle map
  (mode 1, HBlank effects), Mario Tennis court; `240p.gba` test patterns
  with raster effects. Compare against today's screenshots; any tearing or
  a shifted split line is a timing regression, not a PPU bug.
- Audio: a minute of each with the host audio sink, listening for new
  underruns (there should be none - audio rate does not depend on CPU
  speed).
- An old save state (v9) of each, taken before the change, loads and plays.

### Device

Build both ABIs (`./build-mobile.sh android-all`), Mi 10T Pro: 10 minutes
each of Mario Tennis and Yggdra, CPU % of a core (today 106-110%), underrun
count from the log, fps counter at 60, fast-forward rate. Follow the
device-test rule: never save on player games; test on `240p.gba`.

---

## 5. Recommendation

**Do Option B (A plus the prefetch buffer) with the DMA stall, in six
commits; treat A+ as optional and stop before Option C.**

Commit order, each one green on its own:

1. **Neutral refactor**: `memory/timing.rs` with a flat 1-cycle table;
   `charge_fetch/charge_data/charge_idle`; central refill in `Cpu::step`;
   decoders return nothing but charge their data and I cycles; delete
   `read_cycles/write_cycles`. Gate: equivalence test, suite total
   unchanged, host A/B within +5%.
2. **CPU internal cycles**: multiply `m`, register-shift +1I, THUMB hireg,
   THUMB mul. Tests in `cpu.rs` first. Expected Timing +~80 (IWRAM column).
3. **Region waits + WAITCNT**: real table, 32-bit = N16+S16, 128K boundary,
   next-fetch-N after data, disable bug when prefetch off, HLE SWI return
   refill. `tests/timing.rs` prefetch-off rows. Expected Timing ~1000.
   Commercial-ROM and device check here - this is the commit that changes
   how games feel.
4. **Prefetch buffer**. Prefetch-on rows. Expected ~1450-1550.
5. **DMA stall** for immediate, HBlank, VBlank and sound DMA. DMA rows.
   Expected ~1550-1650. Second commercial/device check.
6. *(optional)* **A+**: timer reads at the access cycle and DMA start
   latency. Only if 1-5 leave the timer fits intact and the cost is
   measured; otherwise record it as declined.

**Stop condition.** Done enough is **Timing >= 1500/2020 after commit 5**,
with no other sub-suite moving down, both commercial ROMs visually and
audibly unchanged except for hardware-correct speed, and the device within
its current CPU budget. Declined with reasons:

- **BIOS Div/Sqrt/ArcTan and CpuSet (140 cells)**: the cost is the real
  BIOS's body, data-dependent (Div 338 vs 78 cycles for swapped operands in
  IWRAM). The HLE runs no BIOS code and no BIOS image may enter the repo;
  fitting per-input costs would be gaming the test. No game depends on them.
- **VRAM/OAM/palette PPU contention (+1)**: no test measures it, needs
  per-dot PPU timing.
- **Option C**: rewrite of the core loop for the last ~5%.
- **A+** if it threatens any fitted constant.

**Main risks**

1. Games get the CPU budget of a real GBA instead of 1.5-4x more. Correct,
   but any game that overran on hardware now lags in-game; stopping at
   Option A would make prefetch-on games lag *more* than hardware, which is
   why the buffer is in the recommendation, not optional.
2. HBlank handlers in ROM finish later; raster splits can move. Check on
   screen at commits 3 and 5.
3. The refactor touches every decoder arm; commit 1's neutrality gate is
   what keeps the fitted constants (4, 45, 30, entry 3) valid. If commit 1
   is not exactly neutral, stop and fix it before any wait is added.
4. Host cost on halting games (~+5% on Yggdra estimated); measured at
   commit 1 and again at 4.
