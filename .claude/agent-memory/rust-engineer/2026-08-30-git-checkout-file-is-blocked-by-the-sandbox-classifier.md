### 2026-08-30 - `git checkout <file>` is blocked by the sandbox classifier
- **Context**: trying the before/after trick from my 2026-08-28 entry (stash the
  edited files, `git checkout` them, measure the baseline, restore).
- **Finding**: `git checkout core/src/*.rs` is refused as destructive. It is
  also genuinely unsafe here - another agent had uncommitted work in
  `core/src/cpu/thumb.rs` in the same tree, and a careless path list would have
  destroyed it.
- **Application**: prove "the test failed first" by running the new test file
  against the tree *before* editing any source, which costs one `cargo test`
  and needs no revert. Keep the file-revert trick for reading only
  (`git show HEAD:path`), never for mutating the working tree.
