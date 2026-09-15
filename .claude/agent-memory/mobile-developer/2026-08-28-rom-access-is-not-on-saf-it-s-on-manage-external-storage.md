### 2026-08-28 - ROM access is not on SAF, it's on MANAGE_EXTERNAL_STORAGE

- **Context**: Consultation on battery-save and save-state file layout before
  either is implemented.
- **Finding**: `android/app/src/main/java/com/geebeeayy/app/data/RomFolderManager.kt`
  stores raw path strings in `SharedPreferences("rom_folders")` and walks them
  with plain `java.io.File`. There is no `DocumentFile`, no
  `ACTION_OPEN_DOCUMENT_TREE`, no `takePersistableUriPermission` anywhere in
  `android/`. `AndroidManifest.xml` requests `MANAGE_EXTERNAL_STORAGE` with
  `tools:ignore="ScopedStorage"`, and `MainActivity.kt` gates on
  `Environment.isExternalStorageManager()`. Any design note describing this
  app as "scoped-storage, Uri-permission based" (including this persona's own
  frontmatter) is describing an intended future state, not the current code -
  correct it on sight.
- **Application**: Answers about atomicity of file writes next to the ROM
  depend entirely on which storage model is live. Under the current
  `MANAGE_EXTERNAL_STORAGE` code, `File.renameTo` on the same volume is a
  real `rename(2)` and is atomic. Under a future SAF migration,
  `DocumentFile.renameTo` goes through a content provider and is not
  guaranteed atomic - don't assume the desktop-emulator "temp file next to
  the target, then rename" pattern carries over unchanged if that migration
  happens. `MANAGE_EXTERNAL_STORAGE` is also a Play Store review risk for a
  non-file-manager app; flag it to whoever owns the manifest/store listing
  before it becomes a launch blocker.
