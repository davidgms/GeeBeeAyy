### 2026-08-28 - `cargo fmt` reformats the whole crate; this repo is not rustfmt-clean
- **Context**: implementing the missing HLE BIOS SWIs in `core/src/bios.rs`.
- **Finding**: running `cargo fmt` from `core/` rewrote 16 files and produced a
  1,214-line diff of pure formatting churn on top of a 200-line feature change.
  The codebase predates any formatting pass, so `cargo fmt --check` fails on
  essentially every file even though CLAUDE.md lists `cargo fmt` under Build.
- **Application**: never run bare `cargo fmt` here. Format only what you wrote
  (`rustfmt` the single file, or match the surrounding style by hand) and check
  `git diff --stat` before reporting - a diff much bigger than your change means
  formatter churn crept in. A crate-wide format is its own commit, not a
  side-effect of a feature.
