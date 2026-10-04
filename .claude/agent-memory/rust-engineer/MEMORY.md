# rust-engineer - memory

What a future instance of this agent should not have to re-derive.
Newest last. Each line points at the file holding the detail.

- **2026-08-28** - [SUPERSEDED: crate is rustfmt-clean now; clippy -D warnings needed a toolchain lint sweep](2026-08-28-cargo-fmt-reformats-the-whole-crate-this-repo-is-not-rustfmt.md)
- **2026-08-28** - [Proving "the test comes first" after the fact](2026-08-28-proving-the-test-comes-first-after-the-fact.md)
- **2026-08-28** - [WRONG, corrected 2026-10-04: the arctan "divergence" was our own bad coefficients](2026-08-28-the-bios-arctan-series-is-deliberately-inaccurate-past-pi-4.md)
- **2026-08-30** - [DMA was the sixth "complete but unwired" subsystem, and it hid two decode bugs](2026-08-30-dma-was-the-sixth-complete-but-unwired-subsystem-and-it-hid-.md)
- **2026-08-30** - [The bus cannot own a DMA write, and the reason is the borrow](2026-08-30-the-bus-cannot-own-a-dma-write-and-the-reason-is-the-borrow.md)
- **2026-08-30** - [`git checkout <file>` is blocked by the sandbox classifier](2026-08-30-git-checkout-file-is-blocked-by-the-sandbox-classifier.md)
- **2026-09-10** - [A sprite's priority beats a lower OAM index, and GBATEK says otherwise](2026-09-10-a-sprite-s-priority-beats-a-lower-oam-index-and-gbatek-says-.md)
- **2026-10-01** - [Probing the core without touching the repo](2026-10-01-probing-the-core-without-touching-the-repo.md)
- **2026-10-01** - [Worktree: heredoc/git-substring Bash refusals; temp/roms absent so ROM tests skip](2026-10-01-worktree-sandbox-quirks-heredocs-and-rom-fixtures.md)

Durable facts about the project itself go to `.claude/memory.md` or
`docs/` instead, so every agent and every human gets them; leave a line
here pointing at it.
- **2026-10-02** - [Homebrew suite results and harness traps live in .claude/memory.md (2026-10-02 entry)](../../memory.md)
- **2026-10-02** - [mGBA Timer IRQ / I/O read fixes: CPU vs latched I/O view, fitted IRQ timing (project memory)](../../memory.md)
- **2026-10-02** - [Profiling without perf: SIGPROF sampler + addr2line; samply refused; inlining read8 was slower](2026-10-02-profiling-without-perf.md)
- **2026-10-02** - [PR #60 slowdown was per-step timer work; first-IRQ-offset quirk (project memory)](../../memory.md)
- **2026-10-03** - [Replaying a test ROM's own code (dispatcher included) in a ROM-free test](2026-10-03-replaying-a-test-rom-s-own-code-in-a-unit-test.md)
- **2026-10-03** - [Timer count-up root causes: HALT wake granularity, prescaler alignment, IntrWait cost (project memory)](../../memory.md)
- **2026-10-04** - [mGBA suite FAIL list is in SRAM; git-archive A/B bench](2026-10-04-mgba-suite-fail-list-is-in-sram.md)
- **2026-10-04** - [DMA latch, ROM-increment, alignment rules (project memory)](../../memory.md)
- **2026-10-04** - [Licence check before porting (NBA is GPL, mGBA MPL); source without auth; suite table -> test](2026-10-04-reference-sources-and-hardware-tables.md)
- **2026-10-04** - [BIOS math coefficients, multiply carry model, BIOS read latch (project memory)](../../memory.md)
