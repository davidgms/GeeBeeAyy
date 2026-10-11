---
name: bash-guard-and-docs-provider-quirks
description: Worktree bash guard refuses some compound commands (use Write/Edit); createDocument may append .bin
metadata:
  type: feedback
---

- In a worktree the Bash tool refused `cd <subdir> && python3 - <<EOF ... EOF` and `cd .. && git ...` as
  "too complex"/"computed directory", with no pattern I could pin down. Fix: run from the worktree root
  with relative paths, and use Write/Edit for file bodies instead of python heredocs.
- `DocumentsContract.createDocument(..., "application/octet-stream", "x.gba")` may come back with a
  provider-appended extension; HomebrewDownloader.copyIntoTree queries DISPLAY_NAME after the copy and
  renames. Not verified on a device (no emulator here) - check on the phone first.
- Ensure JVM tests of tree logic stay Uri-free: `scanTree(root, children = fn)` in data/RomFolders.kt takes a
  listing function, `DIR_MIME` is restated because android.jar constants are not reliable in plain JVM tests.
