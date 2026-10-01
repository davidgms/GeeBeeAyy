### 2026-08-28 - `cargo fmt` reformats the whole crate (SUPERSEDED 2026-10-01)

- **Was**: on 2026-08-28 `cargo fmt` rewrote 16 files; the crate predated any
  formatting pass.
- **Now (2026-10-01)**: `core/` is rustfmt-clean. On a fresh branch from
  master `cargo fmt --check` flagged only the lines I had written, so running
  `cargo fmt` is safe. Still glance at `git diff --stat` before committing.
- **Clippy, same date**: `cargo clippy --all-targets -- -D warnings` was
  failing on master with 44 lints from the 1.97 toolchain (new_without_default,
  unnecessary_cast, manual_is_multiple_of, doc_lazy_continuation, ...).
  Cleared in commit `chore: clear clippy lints from the newer toolchain ...`
  on `fix/core-review-round-2`. `cargo clippy --fix --allow-dirty --all-targets`
  did 36 of them safely; the rest were by hand. A toolchain bump can bring the
  failures back, so run the gate before assuming it is green.
