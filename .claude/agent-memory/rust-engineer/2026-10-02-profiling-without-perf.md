---
name: profiling-without-perf
description: How to profile the core on this WSL box - no perf binary, samply refused at perf_event_paranoid=2, no sudo; an in-process SIGPROF sampler plus addr2line works
metadata:
  type: reference
---

This machine (WSL2) has no `perf`, no `gdb`, no `valgrind`, and no sudo.
`cargo install samply --root temp/tools` builds fine, but `samply record`
refuses: `/proc/sys/kernel/perf_event_paranoid` is 2.

What works (2026-10-02): a SIGPROF sampler inside the probe binary.
`setitimer(ITIMER_PROF)` + a `SA_SIGINFO` handler storing
`uc_mcontext.gregs[REG_RIP]` into a static atomic array; subtract the exe's
base from `/proc/self/maps`; then `addr2line -a -f -i -C -e <bin>` gives the
whole inline chain per address (fat LTO inlines everything into `Gba::step`,
so the innermost frame is the useful one). Timer resolution is ~1 kHz no
matter the interval asked for, so run ~4000 frames for ~9k samples.

**How to apply:** probe crate needs `debug = true` + the core's `lto = "fat"`,
`codegen-units = 1` in its own `[profile.release]` (the probe's profile wins,
not core's). `libc` is in the cargo cache. Bench A/B by interleaving binaries
pinned with `taskset -c 3`; single runs vary up to 15% on this box.

Lesson from the same session: breaking `read8`'s recursion (open bus ->
read32 -> read8) so it could inline into `read16`/`read32` made Mario Tennis
~7% *slower*. Bigger inlined fetch code lost; a dedicated wide-read fast path
for RAM/ROM (`MemoryBus::plain`) won instead. Measure inlining ideas.
