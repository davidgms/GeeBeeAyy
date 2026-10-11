---
name: worktree-build-init-the-rcheevos-submodule
description: In a git worktree the rcheevos submodule starts empty; run git submodule update --init, never copy it from the main checkout
metadata:
  type: project
---

- A new worktree has an empty `android/app/src/main/cpp/rcheevos/`, so `assembleDebug` fails in
  `buildCMakeDebug` (rc_client.h missing).
- Fix: `git submodule update --init android/app/src/main/cpp/rcheevos` in the worktree. If CMake
  already configured against the empty folder, delete `android/app/.cxx` and
  `android/app/build/intermediates/cxx`, or the link fails with undefined `rc_client_*` symbols
  (the source glob was evaluated empty).
- Never copy the main checkout's rcheevos folder in: its `.git` pointer file breaks every git
  command in the worktree.
- jniLibs are gitignored and missing too: copy `android/app/src/main/jniLibs` from the main checkout.
