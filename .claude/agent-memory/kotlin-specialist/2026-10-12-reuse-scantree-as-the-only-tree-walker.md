---
name: reuse-scantree-as-the-only-tree-walker
description: scanTree can list a whole SAF tree for non-ROM files by recording the listings its children callback returns; settings screen gets library + open game via MainActivity
metadata:
  type: project
---

- `scanTree(root, children)` (data/RomFolders.kt) calls `children` exactly once per directory, ROMs or not.
  Wrapping the callback to record listings gives every file in the tree without a second walker
  (`findSaves` in data/SaveImport.kt). If scanTree ever skips ROM-less directories this breaks; its test pins it.
- The game open behind Settings: `navController.previousBackStackEntry` with route `emulation/{filePath}`,
  id = `RomBytes.decodeId(arguments["filePath"])` (MainActivity settings composable). No VM access needed.
- Auto-import fit check is lazy `(Int) -> Int?` into `importLegacySave`; the cart is only read when a file is
  actually about to be imported (marker/newer-wins short-circuit first), so the per-scan loop stays cheap.
- values/strings.xml old strings are English and mostly unused (Settings hardcodes English); the save-import
  ones are Portuguese per the lead's instruction.
