package com.geebeeayy.app.ui.screens

import android.content.Intent
import android.net.Uri
import android.os.Environment

import android.provider.Settings as AndroidSettings
import android.view.KeyEvent
import android.view.InputDevice
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.DpOffset
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.geebeeayy.app.data.ControlFontSize
import com.geebeeayy.app.data.HapticStrength
import com.geebeeayy.app.data.ControlTint
import com.geebeeayy.app.data.ControlLayoutStore
import com.geebeeayy.app.engine.RaEngine
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.runtime.collectAsState
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.ui.res.stringResource
import com.geebeeayy.app.R
import com.geebeeayy.app.data.CoverArt
import com.geebeeayy.app.data.RomEntry
import com.geebeeayy.app.data.SaveExport
import com.geebeeayy.app.data.SaveExportRow
import com.geebeeayy.app.data.SaveImport
import com.geebeeayy.app.data.SaveImportRow
import com.geebeeayy.app.engine.GbaEngine
import com.geebeeayy.app.data.DisplaySettings
import com.geebeeayy.app.data.RomBytes
import com.geebeeayy.app.data.RomFolderManager
import com.geebeeayy.app.data.legacyDocId
import com.geebeeayy.app.data.ScreenOrientation
import com.geebeeayy.app.data.ScaleMode
import com.geebeeayy.app.data.ScreenFilter
import com.geebeeayy.app.engine.AudioOutput
import com.geebeeayy.app.ui.findActivity
import com.geebeeayy.app.ui.orientationFor
import com.geebeeayy.app.ui.theme.*


/**
 * The ratios offered, with 0 meaning unlimited.
 *
 * Three steps, doubling: 2x is exact and looks normal on a game with the
 * headroom for it, 8x is the sloppy end where the picture starts skipping in
 * exchange for getting through a grind. A finer ladder was tried and thrown
 * away - the steps in between were not distinguishable in the hand, and past
 * 8x the speed goes back *down* while the picture keeps getting worse.
 */
private val FastForwardRatios = listOf(2, 4, 8, 0)

private fun fastForwardLabel(ratio: Int): String =
    if (ratio == 0) "Unlimited - no audio" else "${ratio}x"

private val ScaleMode.label: String
    get() = when (this) {
        ScaleMode.FIT -> "Fit"
        ScaleMode.INTEGER -> "Integer (Pixel Perfect)"
        ScaleMode.STRETCH -> "Stretch"
    }

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SettingsScreen(
    onBack: () -> Unit,
    onFoldersChanged: () -> Unit = {},
    /** The library, to match imported saves against. */
    roms: List<RomEntry> = emptyList(),
    /** Id of the game running behind this screen, if Settings was opened from one. */
    openGame: String? = null,
) {
    val context = LocalContext.current
    val folderManager = remember { RomFolderManager(context) }
    var folders by remember { mutableStateOf(folderManager.getFolderPaths()) }
    val scope = rememberCoroutineScope()

    val displaySettings = remember { DisplaySettings(context) }
    var scaleMode by remember { mutableStateOf(displaySettings.getScaleMode()) }
    var screenFilter by remember { mutableStateOf(displaySettings.getScreenFilter()) }
    var showFilterMenu by remember { mutableStateOf(false) }
    var controlScale by remember { mutableFloatStateOf(displaySettings.getControlScale()) }
    var controlOpacity by remember { mutableFloatStateOf(displaySettings.getControlOpacity()) }
    var showScaleMenu by remember { mutableStateOf(false) }
    var orientation by remember { mutableStateOf(displaySettings.getScreenOrientation()) }
    var showOrientationMenu by remember { mutableStateOf(false) }
    var fullscreen by remember { mutableStateOf(displaySettings.getFullscreenInGame()) }
    var showStatusStrip by remember { mutableStateOf(displaySettings.getShowStatusStrip()) }
    var coverArt by remember { mutableStateOf(displaySettings.getDownloadCoverArt()) }
    var autoLoad by remember { mutableStateOf(displaySettings.getAutoLoadQuickSave()) }
    var showPerf by remember { mutableStateOf(displaySettings.getShowPerformance()) }
    var rateTip by remember { mutableStateOf(displaySettings.getRateTip()) }
    var volume by remember { mutableFloatStateOf(displaySettings.getVolume()) }
    var showRaLogin by remember { mutableStateOf(false) }
    var coverCacheBytes by remember { mutableLongStateOf(CoverArt.cacheBytes(context)) }
    val layoutStore = remember { ControlLayoutStore(context) }
    var layouts by remember { mutableStateOf(layoutStore.getLayouts()) }
    var defaultLayoutId by remember { mutableStateOf(layoutStore.getDefaultLayoutId()) }
    var showLayoutMenu by remember { mutableStateOf(false) }
    var controlTint by remember { mutableStateOf(displaySettings.getControlTint()) }
    var showTintMenu by remember { mutableStateOf(false) }
    var controlFont by remember { mutableStateOf(displaySettings.getControlFontSize()) }
    var showFontMenu by remember { mutableStateOf(false) }
    var haptics by remember { mutableStateOf(displaySettings.getHapticStrength()) }
    var showHapticMenu by remember { mutableStateOf(false) }
    var dpadCardinal by remember {
        mutableFloatStateOf(displaySettings.getDpadCardinalHalfDegrees())
    }
    var soundEnabled by remember { mutableStateOf(displaySettings.getSoundEnabled()) }
    var fastForwardRatio by remember { mutableIntStateOf(displaySettings.getFastForwardRatio()) }
    var showSpeedMenu by remember { mutableStateOf(false) }
    var muteFastForward by remember { mutableStateOf(displaySettings.getMuteOnFastForward()) }
    // Read once: a pad paired while this screen is open is rare enough that a
    // reopen is a fair price for not polling the input system every frame.
    //
    // The source bits alone are not enough. This phone's fingerprint reader
    // is a virtual uinput device that claims SOURCE_GAMEPAD, so it was listed
    // as a paired controller. A real pad is physical and has an A button.
    val gamepads = remember {
        InputDevice.getDeviceIds().toList()
            .mapNotNull { InputDevice.getDevice(it) }
            .filter { device ->
                !device.isVirtual &&
                    device.supportsSource(InputDevice.SOURCE_GAMEPAD) &&
                    device.hasKeys(KeyEvent.KEYCODE_BUTTON_A).firstOrNull() == true
            }
            .map { it.name }
    }
    var interframeBlend by remember { mutableStateOf(displaySettings.getInterframeBlend()) }
    var folderError by remember { mutableStateOf<String?>(null) }

    // The picker keeps the tree Uri (and a persistable grant) instead of
    // turning it into a path, which is what stops working without all-files
    // access. A re-pick starts where the old folder was.
    val folderPicker = rememberLauncherForActivityResult(
        contract = ActivityResultContracts.OpenDocumentTree()
    ) { uri: Uri? ->
        uri?.let {
            // The grant, the prefs and the re-keying are I/O; only the refresh is UI.
            scope.launch(Dispatchers.IO) {
                val kept = folderManager.addTree(it)
                val now = folderManager.getFolderPaths()
                withContext(Dispatchers.Main) {
                    if (kept) {
                        folders = now
                        onFoldersChanged()
                        folderError = null
                    } else {
                        folderError = "Could not keep access to that folder. Try picking it again."
                    }
                }
            }
        }
    }
    val unavailable by RomFolderManager.unavailable.collectAsState()

    // Read once, no persisted grant: the tree is only listed and its files read
    // while this screen is up, and keeping access to somebody else's folder
    // would be a permission the app has no use for afterwards.
    var importRows by remember { mutableStateOf<List<SaveImportRow>?>(null) }
    var importStatus by remember { mutableStateOf<String?>(null) }
    var importBusy by remember { mutableStateOf(false) }
    val scanning = stringResource(R.string.import_saves_scanning)
    val noSaves = stringResource(R.string.import_saves_none)
    val noFolder = stringResource(R.string.import_saves_unavailable)
    val savePicker = rememberLauncherForActivityResult(
        contract = ActivityResultContracts.OpenDocumentTree()
    ) { uri: Uri? ->
        uri?.let {
            importBusy = true
            importStatus = scanning
            scope.launch(Dispatchers.IO) {
                val rows = SaveImport.scan(context, it, roms, GbaEngine::saveFit)
                withContext(Dispatchers.Main) {
                    importBusy = false
                    importStatus = if (rows == null) noFolder else if (rows.isEmpty()) noSaves else null
                    importRows = rows?.takeIf { r -> r.isNotEmpty() }
                }
            }
        }
    }

    var exportTree by remember { mutableStateOf<Uri?>(null) }
    var exportRows by remember { mutableStateOf<List<SaveExportRow>?>(null) }
    var exportStatus by remember { mutableStateOf<String?>(null) }
    var exportBusy by remember { mutableStateOf(false) }
    val exportNone = stringResource(R.string.export_saves_none)
    val exportScanning = stringResource(R.string.export_saves_scanning)
    val exportPicker = rememberLauncherForActivityResult(
        contract = ActivityResultContracts.OpenDocumentTree()
    ) { uri: Uri? ->
        uri?.let {
            exportBusy = true
            exportStatus = exportScanning
            scope.launch(Dispatchers.IO) {
                val rows = SaveExport.scan(context, it, roms)
                withContext(Dispatchers.Main) {
                    exportBusy = false
                    exportStatus = if (rows == null) noFolder else if (rows.isEmpty()) exportNone else null
                    exportTree = it
                    exportRows = rows?.takeIf { r -> r.isNotEmpty() }
                }
            }
        }
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = {
                    Text(
                        text = "Settings",
                        fontWeight = FontWeight.Bold,
                        color = GoldenSaplight,
                    )
                },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.Default.ArrowBack, "Back", tint = PineGlowMist)
                    }
                },
                colors = TopAppBarDefaults.topAppBarColors(
                    containerColor = NightVoid,
                ),
            )
        },
        containerColor = NightVoid
    ) { padding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding)
                .verticalScroll(rememberScrollState())
                .padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            SettingsSection(title = "ROM Folders") {
                if (folders.isEmpty()) {
                    Box(
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(16.dp),
                        contentAlignment = Alignment.Center,
                    ) {
                        Text(
                            text = "No folders added yet",
                            fontSize = 14.sp,
                            color = PineGlowMist.copy(alpha = 0.5f),
                        )
                    }
                } else {
                    folders.forEach { path ->
                        val displayName = RomFolderManager.folderLabel(path).substringAfterLast('/')
                        // A path folder on shared storage cannot be read
                        // without a storage permission, which the app no
                        // longer has: it needs picking again.
                        val repick = RomBytes.isPath(path) && legacyDocId(path) != null
                        val problem = when {
                            repick -> "Needs re-pick: tap to grant access again"
                            path in unavailable -> "Folder unavailable: tap to pick it again"
                            else -> null
                        }
                        Row(
                            modifier = Modifier
                                .fillMaxWidth()
                                .padding(horizontal = 16.dp, vertical = 10.dp),
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            Icon(
                                Icons.Default.Folder,
                                contentDescription = null,
                                tint = AmberResin,
                                modifier = Modifier.size(24.dp)
                            )
                            Spacer(modifier = Modifier.width(16.dp))
                            Column(
                                modifier = Modifier
                                    .weight(1f)
                                    .clickable(enabled = problem != null) {
                                        folderPicker.launch(RomFolderManager.repickStart(path))
                                    },
                            ) {
                                Text(
                                    text = displayName,
                                    fontSize = 14.sp,
                                    color = PineGlowMist,
                                )
                                Text(
                                    text = RomFolderManager.folderLabel(path),
                                    fontSize = 11.sp,
                                    color = PineGlowMist.copy(alpha = 0.4f),
                                )
                                problem?.let {
                                    Text(text = it, fontSize = 11.sp, color = Error)
                                }
                            }
                            IconButton(onClick = {
                                scope.launch(Dispatchers.IO) {
                                    folderManager.removeFolder(path)
                                    val now = folderManager.getFolderPaths()
                                    withContext(Dispatchers.Main) {
                                        folders = now
                                        onFoldersChanged()
                                    }
                                }
                            }) {
                                Icon(
                                    Icons.Default.Close,
                                    contentDescription = "Remove",
                                    tint = Error,
                                    modifier = Modifier.size(18.dp)
                                )
                            }
                        }
                    }
                }

                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .clip(RoundedCornerShape(bottomStart = 12.dp, bottomEnd = 12.dp))
                        .clickable { folderPicker.launch(null) }
                        .padding(horizontal = 16.dp, vertical = 14.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Icon(
                        Icons.Default.Add,
                        contentDescription = null,
                        tint = AmberResin,
                        modifier = Modifier.size(24.dp)
                    )
                    Spacer(modifier = Modifier.width(16.dp))
                    Text(
                        text = "Add ROM Folder",
                        fontSize = 16.sp,
                        color = AmberResin,
                    )
                }
            }

            folderError?.let { message ->
                Text(
                    text = message,
                    fontSize = 13.sp,
                    color = AmberResin,
                    modifier = Modifier.padding(horizontal = 16.dp, vertical = 4.dp),
                )
            }

            SettingsSection(title = stringResource(R.string.import_saves_dialog_title)) {
                SettingsItem(
                    icon = Icons.Default.Save,
                    title = stringResource(R.string.import_saves_title),
                    subtitle = importStatus ?: stringResource(R.string.import_saves_subtitle),
                    onClick = { if (!importBusy) savePicker.launch(null) },
                )
            }

            SettingsSection(title = stringResource(R.string.export_saves_dialog_title)) {
                SettingsItem(
                    icon = Icons.Default.Save,
                    title = stringResource(R.string.export_saves_title),
                    subtitle = exportStatus ?: stringResource(R.string.export_saves_subtitle),
                    onClick = { if (!exportBusy) exportPicker.launch(null) },
                )
            }

            SettingsSection(title = "Display") {
                Box {
                    SettingsItem(
                        icon = Icons.Default.Star,
                        title = "Screen Scale",
                        subtitle = scaleMode.label,
                        onClick = { showScaleMenu = true }
                    )
                    DropdownMenu(
                        expanded = showScaleMenu,
                        onDismissRequest = { showScaleMenu = false },
                        offset = SettingsMenuOffset,
                    ) {
                        ScaleMode.entries.forEach { mode ->
                            DropdownMenuItem(
                                text = { Text(mode.label) },
                                onClick = {
                                    scaleMode = mode
                                    displaySettings.setScaleMode(mode)
                                    showScaleMenu = false
                                }
                            )
                        }
                    }
                }
                Box {
                    SettingsItem(
                        icon = Icons.Default.Tune,
                        title = "Screen Filter",
                        subtitle = screenFilter.label,
                        onClick = { showFilterMenu = true }
                    )
                    DropdownMenu(
                        expanded = showFilterMenu,
                        onDismissRequest = { showFilterMenu = false },
                        offset = SettingsMenuOffset,
                    ) {
                        ScreenFilter.entries.forEach { f ->
                            DropdownMenuItem(
                                text = { Text(f.label) },
                                onClick = {
                                    screenFilter = f
                                    displaySettings.setScreenFilter(f)
                                    showFilterMenu = false
                                }
                            )
                        }
                    }
                }
                SettingsSwitch(
                    icon = Icons.Default.BlurOn,
                    title = "Frame Blending",
                    subtitle = "Smooths flicker, like the GBA screen",
                    checked = interframeBlend,
                    onCheckedChange = { checked ->
                        interframeBlend = checked
                        displaySettings.setInterframeBlend(checked)
                    }
                )
                SettingsSlider(
                    icon = Icons.Default.OpenInFull,
                    title = "Control Size",
                    // Shrink only - at 1.0 the row already fills the width,
                    // so growing pushes L, R and the outer D-pad off-screen.
                    subtitle = "%.2fx".format(controlScale) +
                        if (controlScale < 1.0f) " (below the 48.dp touch target)" else "",
                    value = controlScale,
                    range = DisplaySettings.MIN_CONTROL_SCALE..1.0f,
                    onValueChange = { controlScale = it },
                    onValueChangeFinished = { displaySettings.setControlScale(controlScale) },
                )
                SettingsSlider(
                    icon = Icons.Default.Opacity,
                    title = "Control Opacity",
                    subtitle = "%d%%".format((controlOpacity * 100).toInt()),
                    value = controlOpacity,
                    range = DisplaySettings.MIN_CONTROL_OPACITY..1.0f,
                    onValueChange = { controlOpacity = it },
                    onValueChangeFinished = { displaySettings.setControlOpacity(controlOpacity) },
                )
                Box {
                    SettingsItem(
                        icon = Icons.Default.StayCurrentPortrait,
                        title = "Orientation",
                        subtitle = orientation.label,
                        onClick = { showOrientationMenu = true }
                    )
                    DropdownMenu(
                        expanded = showOrientationMenu,
                        onDismissRequest = { showOrientationMenu = false },
                        offset = SettingsMenuOffset,
                    ) {
                        ScreenOrientation.entries.forEach { option ->
                            DropdownMenuItem(
                                text = { Text(option.label) },
                                onClick = {
                                    orientation = option
                                    displaySettings.setScreenOrientation(option)
                                    context.findActivity()?.requestedOrientation =
                                        orientationFor(option)
                                    showOrientationMenu = false
                                }
                            )
                        }
                    }
                }
                SettingsSwitch(
                    icon = Icons.Default.Fullscreen,
                    title = "Hide System Bars",
                    subtitle = "Status bar and navigation bar, while a game runs",
                    checked = fullscreen,
                    onCheckedChange = { checked ->
                        fullscreen = checked
                        displaySettings.setFullscreenInGame(checked)
                    }
                )
                SettingsSwitch(
                    icon = Icons.Default.BatteryStd,
                    title = "Clock & Battery",
                    subtitle = "Along the bottom while playing",
                    checked = showStatusStrip,
                    onCheckedChange = { checked ->
                        showStatusStrip = checked
                        displaySettings.setShowStatusStrip(checked)
                    }
                )
                SettingsSwitch(
                    icon = Icons.Default.Speed,
                    title = "Show FPS, screen Hz and audio underruns",
                    subtitle = "A diagnostic, over the black above the picture",
                    checked = showPerf,
                    onCheckedChange = { checked ->
                        showPerf = checked
                        displaySettings.setShowPerformance(checked)
                    }
                )
                SettingsSwitch(
                    icon = Icons.Default.Speed,
                    title = "Low screen rate tip",
                    subtitle = "Tell me when the phone holds the screen below 60 Hz " +
                        "during a game (Xiaomi phones do)",
                    checked = rateTip,
                    onCheckedChange = { checked ->
                        rateTip = checked
                        displaySettings.setRateTip(checked)
                    }
                )
                SettingsSwitch(
                    icon = Icons.Default.RestartAlt,
                    title = "Resume from quick save",
                    subtitle = "Opening a game jumps back into slot 0 instead " +
                        "of the title screen",
                    checked = autoLoad,
                    onCheckedChange = { checked ->
                        autoLoad = checked
                        displaySettings.setAutoLoadQuickSave(checked)
                    }
                )
                SettingsSwitch(
                    icon = Icons.Default.Image,
                    title = "Download cover art",
                    // Says where it goes and who owns what arrives, because
                    // this is the only thing in the app that reaches the
                    // network on its own once it is on.
                    subtitle = "From thumbnails.libretro.com, for games with " +
                        "no picture beside them. The art belongs to each " +
                        "game's publisher.",
                    checked = coverArt,
                    onCheckedChange = { checked ->
                        coverArt = checked
                        displaySettings.setDownloadCoverArt(checked)
                    }
                )
                if (coverArt || coverCacheBytes > 0) {
                    SettingsItem(
                        icon = Icons.Default.DeleteSweep,
                        title = "Clear downloaded covers",
                        subtitle = if (coverCacheBytes > 0) {
                            "%.1f MB cached. Also retries games that had none."
                                .format(coverCacheBytes / 1024.0 / 1024.0)
                        } else {
                            "Nothing cached yet"
                        },
                        onClick = {
                            CoverArt.clearCache(context)
                            coverCacheBytes = CoverArt.cacheBytes(context)
                        },
                    )
                }
            }

            SettingsSection(title = "RetroAchievements") {
                // Started here as well as when a game opens: signing in
                // happens with no ROM loaded, and without this the client did
                // not exist yet and the first attempt went nowhere at all.
                LaunchedEffect(Unit) { RaEngine.start(context) }
                val raUser by RaEngine.user.collectAsState()
                if (!RaEngine.isAvailable()) {
                    SettingsInfo(
                        icon = Icons.Default.EmojiEvents,
                        title = "Not available in this build",
                        subtitle = "The achievements library was not compiled in",
                    )
                } else if (raUser == null) {
                    SettingsItem(
                        icon = Icons.Default.EmojiEvents,
                        title = "Sign in",
                        // Says whose account it is and what is kept, because
                        // asking for a password deserves both.
                        subtitle = "Your own retroachievements.org account. " +
                            "The password is used once and never stored.",
                        onClick = { showRaLogin = true },
                    )
                } else {
                    SettingsInfo(
                        icon = Icons.Default.EmojiEvents,
                        title = "Signed in as $raUser",
                        subtitle = "Achievements unlock while you play. " +
                            "Hardcore is off - see docs/achievements.md.",
                    )
                    SettingsItem(
                        icon = Icons.Default.ExitToApp,
                        title = "Sign out",
                        subtitle = "Forgets the stored login token",
                        onClick = { RaEngine.logout() },
                    )
                }
            }

            SettingsSection(title = "Emulation") {
                Box {
                    SettingsItem(
                        icon = Icons.Default.FastForward,
                        title = "Fast Forward Speed",
                        subtitle = fastForwardLabel(fastForwardRatio),
                        onClick = { showSpeedMenu = true }
                    )
                    DropdownMenu(
                        expanded = showSpeedMenu,
                        onDismissRequest = { showSpeedMenu = false },
                        offset = SettingsMenuOffset,
                    ) {
                        FastForwardRatios.forEach { ratio ->
                            DropdownMenuItem(
                                text = { Text(fastForwardLabel(ratio)) },
                                onClick = {
                                    fastForwardRatio = ratio
                                    displaySettings.setFastForwardRatio(ratio)
                                    showSpeedMenu = false
                                }
                            )
                        }
                    }
                }
                SettingsSwitch(
                    icon = Icons.Default.VolumeOff,
                    title = "Mute Fast Forward",
                    subtitle = "Silence while speeding up",
                    checked = muteFastForward,
                    onCheckedChange = { checked ->
                        muteFastForward = checked
                        displaySettings.setMuteOnFastForward(checked)
                    }
                )
            }

            SettingsSection(title = "Audio") {
                SettingsSlider(
                    icon = Icons.Default.VolumeDown,
                    title = "Volume",
                    // Separate from the system volume on purpose: an emulator
                    // is often played alongside something else, and turning
                    // the whole phone down is the wrong tool.
                    subtitle = "${(volume * 100).toInt()}% - takes effect on the next game",
                    value = volume,
                    range = 0f..1f,
                    onValueChange = { volume = it },
                    onValueChangeFinished = { displaySettings.setVolume(volume) },
                )
                SettingsSwitch(
                    icon = Icons.Default.VolumeUp,
                    title = "Sound",
                    subtitle = "Enable audio output",
                    checked = soundEnabled,
                    onCheckedChange = { checked ->
                        soundEnabled = checked
                        displaySettings.setSoundEnabled(checked)
                    }
                )
                // Not a button: there is one backend, so a row that opened a
                // picker would be offering a choice that does not exist. It
                // also used to claim "AAudio", which is not what the app
                // builds - AudioOutput drives an AudioTrack directly.
                SettingsInfo(
                    icon = Icons.Default.MusicNote,
                    title = "Audio Backend",
                    subtitle = "AudioTrack - ${AudioOutput.SAMPLE_RATE} Hz mono, low latency",
                )
            }

            SettingsSection(title = "Controls") {
                Box {
                    SettingsItem(
                        icon = Icons.Default.Gamepad,
                        title = "Layout",
                        subtitle = layouts.firstOrNull { it.id == defaultLayoutId }?.name
                            ?: "Default",
                        onClick = {
                            layouts = layoutStore.getLayouts()
                            showLayoutMenu = true
                        }
                    )
                    DropdownMenu(
                        expanded = showLayoutMenu,
                        onDismissRequest = { showLayoutMenu = false },
                        offset = SettingsMenuOffset,
                    ) {
                        layouts.forEach { layout ->
                            DropdownMenuItem(
                                text = { Text(layout.name) },
                                onClick = {
                                    defaultLayoutId = layout.id
                                    layoutStore.setDefaultLayoutId(layout.id)
                                    showLayoutMenu = false
                                }
                            )
                        }
                    }
                }
                Box {
                    SettingsItem(
                        icon = Icons.Default.Palette,
                        title = "Button Colour",
                        subtitle = controlTint.label,
                        onClick = { showTintMenu = true }
                    )
                    DropdownMenu(
                        expanded = showTintMenu,
                        onDismissRequest = { showTintMenu = false },
                        offset = SettingsMenuOffset,
                    ) {
                        ControlTint.entries.forEach { tint ->
                            DropdownMenuItem(
                                text = { Text(tint.label) },
                                onClick = {
                                    controlTint = tint
                                    displaySettings.setControlTint(tint)
                                    showTintMenu = false
                                }
                            )
                        }
                    }
                }
                Box {
                    SettingsItem(
                        icon = Icons.Default.FormatSize,
                        title = "Button Text Size",
                        subtitle = controlFont.label,
                        onClick = { showFontMenu = true }
                    )
                    DropdownMenu(
                        expanded = showFontMenu,
                        onDismissRequest = { showFontMenu = false },
                        offset = SettingsMenuOffset,
                    ) {
                        ControlFontSize.entries.forEach { size ->
                            DropdownMenuItem(
                                text = { Text(size.label) },
                                onClick = {
                                    controlFont = size
                                    displaySettings.setControlFontSize(size)
                                    showFontMenu = false
                                }
                            )
                        }
                    }
                }
                Box {
                    SettingsItem(
                        icon = Icons.Default.Vibration,
                        title = "Vibration",
                        subtitle = haptics.label,
                        onClick = { showHapticMenu = true }
                    )
                    DropdownMenu(
                        expanded = showHapticMenu,
                        onDismissRequest = { showHapticMenu = false },
                        offset = SettingsMenuOffset,
                    ) {
                        HapticStrength.entries.forEach { strength ->
                            DropdownMenuItem(
                                text = { Text(strength.label) },
                                onClick = {
                                    haptics = strength
                                    displaySettings.setHapticStrength(strength)
                                    showHapticMenu = false
                                }
                            )
                        }
                    }
                }
                SettingsSlider(
                    icon = Icons.Default.ControlCamera,
                    title = "D-Pad Diagonals",
                    // Shown as how wide a diagonal is, not as the internal
                    // half-angle: "45 degrees of diagonal" is something a
                    // thumb can picture, "30 degrees of cardinal half" is not.
                    subtitle = "%.0f degrees wide".format(90f - dpadCardinal * 2f),
                    // Reversed: dragging right should widen the diagonal, and
                    // the number that widens it is the one that shrinks.
                    value = DisplaySettings.MAX_DPAD_CARDINAL_HALF - dpadCardinal,
                    range = 0f..(DisplaySettings.MAX_DPAD_CARDINAL_HALF -
                        DisplaySettings.MIN_DPAD_CARDINAL_HALF),
                    onValueChange = {
                        dpadCardinal = DisplaySettings.MAX_DPAD_CARDINAL_HALF - it
                    },
                    onValueChangeFinished = {
                        displaySettings.setDpadCardinalHalfDegrees(dpadCardinal)
                    },
                )
                // Reports what is actually paired and opens the system's own
                // Bluetooth screen, which is the only place pairing happens.
                // Mapping a pad's buttons to the GBA's is not built yet, so
                // this deliberately does not claim it is.
                SettingsItem(
                    icon = Icons.Default.Bluetooth,
                    title = "Bluetooth Controller",
                    subtitle = gamepads.firstOrNull() ?: "None paired - tap to pair",
                    onClick = {
                        runCatching {
                            context.startActivity(
                                Intent(AndroidSettings.ACTION_BLUETOOTH_SETTINGS)
                                    .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                            )
                        }
                    }
                )
            }

            SettingsSection(title = "About") {
                SettingsItem(
                    icon = Icons.Default.Info,
                    title = "GeeBeeAyy!",
                    subtitle = "v0.1.0",
                    onClick = { }
                )
            }

            Spacer(modifier = Modifier.height(32.dp))
        }
    }

    if (showRaLogin) {
        RaLoginDialog(onDismiss = { showRaLogin = false })
    }

    importRows?.let { rows ->
        SaveImportDialog(
            rows = rows,
            roms = roms,
            onToggle = { i -> importRows = importRows?.mapIndexed { j, r -> if (i == j) r.copy(selected = !r.selected) else r } },
            onPick = { i, rom ->
                scope.launch(Dispatchers.IO) {
                    val updated = SaveImport.pick(context, rows[i], rom, GbaEngine::saveFit)
                    withContext(Dispatchers.Main) {
                        importRows = importRows?.mapIndexed { j, r -> if (i == j) updated else r }
                    }
                }
            },
            onDismiss = { importRows = null },
            onConfirm = {
                importRows = null
                importBusy = true
                scope.launch(Dispatchers.IO) {
                    val result = SaveImport.apply(context, rows, openGame)
                    val text = context.getString(R.string.import_saves_result, result.imported, result.skipped, result.failed) +
                        if (result.refused.isEmpty()) "" else
                            "\n" + context.getString(R.string.import_saves_refused, result.refused.joinToString())
                    withContext(Dispatchers.Main) {
                        importBusy = false
                        importStatus = text
                    }
                }
            },
        )
    }

    exportRows?.let { rows ->
        val tree = exportTree
        SaveExportDialog(
            rows = rows,
            onToggle = { i -> exportRows = exportRows?.mapIndexed { j, r -> if (i == j) r.copy(selected = !r.selected) else r } },
            onDismiss = { exportRows = null },
            onConfirm = {
                exportRows = null
                if (tree != null) {
                    exportBusy = true
                    scope.launch(Dispatchers.IO) {
                        val result = SaveExport.apply(context, tree, rows, openGame)
                        val text = context.getString(R.string.export_saves_result, result.exported, result.skipped, result.failed) +
                            if (result.refused.isEmpty()) "" else
                                "\n" + context.getString(R.string.export_saves_refused, result.refused.joinToString())
                        withContext(Dispatchers.Main) {
                            exportBusy = false
                            exportStatus = text
                        }
                    }
                }
            },
        )
    }
}

private fun formatBytes(n: Long): String = if (n >= 1024) "${n / 1024} KB" else "$n B"

private fun formatTime(millis: Long): String =
    java.text.DateFormat.getDateTimeInstance(java.text.DateFormat.SHORT, java.text.DateFormat.SHORT).format(millis)

@Composable
private fun SaveImportDialog(
    rows: List<SaveImportRow>,
    roms: List<RomEntry>,
    onToggle: (Int) -> Unit,
    onPick: (Int, RomEntry) -> Unit,
    onDismiss: () -> Unit,
    onConfirm: () -> Unit,
) {
    val chosen = rows.count { it.selected && it.importable }
    var picking by remember { mutableStateOf<Int?>(null) }
    picking?.let { i ->
        GamePickerDialog(
            fileName = rows[i].fileName,
            roms = roms,
            onPick = { rom -> picking = null; onPick(i, rom) },
            onDismiss = { picking = null },
        )
    }
    AlertDialog(
        onDismissRequest = onDismiss,
        containerColor = NightPanel,
        titleContentColor = GoldenSaplight,
        textContentColor = PineGlowMist,
        title = { Text(stringResource(R.string.import_saves_dialog_title), fontWeight = FontWeight.Bold) },
        text = {
            LazyColumn(modifier = Modifier.heightIn(max = 420.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                itemsIndexed(rows) { i, row ->
                    Row(verticalAlignment = Alignment.Top) {
                        Checkbox(
                            checked = row.selected,
                            enabled = row.importable,
                            onCheckedChange = { onToggle(i) },
                        )
                        Column(modifier = Modifier.weight(1f)) {
                            Text(row.fileName, fontSize = 14.sp)
                            Text(row.rom?.name ?: "", fontSize = 12.sp, color = AmberResin)
                            val mine = formatBytes(row.size) to formatTime(row.modified)
                            val note = when {
                                row.rom == null -> stringResource(R.string.import_saves_no_match)
                                row.key == null -> stringResource(R.string.import_saves_unreadable)
                                row.keep == null -> stringResource(R.string.import_saves_misfit, formatBytes(row.size))
                                row.existingSize > 0 -> stringResource(
                                    R.string.import_saves_existing,
                                    formatBytes(row.existingSize), formatTime(row.existingModified), mine.first, mine.second,
                                )
                                else -> stringResource(R.string.import_saves_fresh, mine.first, mine.second)
                            }
                            Text(note, fontSize = 11.sp, color = if (row.importable) PineGlowMist.copy(alpha = 0.6f) else Error)
                            if (row.rom == null || !row.importable) {
                                TextButton(onClick = { picking = i }) {
                                    Text(stringResource(R.string.import_saves_pick_game), color = GoldenSaplight)
                                }
                            }
                        }
                    }
                }
            }
        },
        confirmButton = {
            TextButton(onClick = onConfirm, enabled = chosen > 0) {
                Text(stringResource(R.string.import_saves_confirm, chosen), color = GoldenSaplight)
            }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) { Text(stringResource(R.string.import_saves_cancel), color = PineGlowMist) }
        },
    )
}

@Composable
private fun SaveExportDialog(
    rows: List<SaveExportRow>,
    onToggle: (Int) -> Unit,
    onDismiss: () -> Unit,
    onConfirm: () -> Unit,
) {
    val chosen = rows.count { it.selected }
    AlertDialog(
        onDismissRequest = onDismiss,
        containerColor = NightPanel,
        titleContentColor = GoldenSaplight,
        textContentColor = PineGlowMist,
        title = { Text(stringResource(R.string.export_saves_dialog_title), fontWeight = FontWeight.Bold) },
        text = {
            Column {
                Text(stringResource(R.string.export_saves_warning), fontSize = 12.sp, color = AmberResin)
                Spacer(modifier = Modifier.height(8.dp))
                LazyColumn(modifier = Modifier.heightIn(max = 360.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                    itemsIndexed(rows) { i, row ->
                        Row(verticalAlignment = Alignment.Top) {
                            Checkbox(checked = row.selected, onCheckedChange = { onToggle(i) })
                            Column(modifier = Modifier.weight(1f)) {
                                Text(row.rom.name, fontSize = 14.sp)
                                Text("-> ${row.targetName}", fontSize = 12.sp, color = AmberResin)
                                val note = if (row.targetDocId == null) {
                                    stringResource(R.string.export_saves_new, formatBytes(row.saveSize))
                                } else {
                                    stringResource(
                                        R.string.export_saves_overwrite,
                                        formatBytes(row.targetSize), formatTime(row.targetModified),
                                    )
                                }
                                Text(note, fontSize = 11.sp, color = PineGlowMist.copy(alpha = 0.6f))
                            }
                        }
                    }
                }
            }
        },
        confirmButton = {
            TextButton(onClick = onConfirm, enabled = chosen > 0) {
                Text(stringResource(R.string.export_saves_confirm, chosen), color = GoldenSaplight)
            }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) { Text(stringResource(R.string.import_saves_cancel), color = PineGlowMist) }
        },
    )
}

/**
 * The library as a centred, titled, scrolling list. A dropdown anchored to
 * the row ran off the screen once the library grew past a handful of games.
 * The folder is shown because two folders can hold a game of the same name.
 */
@Composable
private fun GamePickerDialog(
    fileName: String,
    roms: List<RomEntry>,
    onPick: (RomEntry) -> Unit,
    onDismiss: () -> Unit,
) {
    AlertDialog(
        onDismissRequest = onDismiss,
        containerColor = NightPanel,
        titleContentColor = GoldenSaplight,
        textContentColor = PineGlowMist,
        title = { Text(stringResource(R.string.import_saves_pick_title, fileName), fontWeight = FontWeight.Bold) },
        text = {
            LazyColumn(modifier = Modifier.heightIn(max = 360.dp)) {
                items(roms.filter { it.exists }) { rom ->
                    Column(
                        modifier = Modifier
                            .fillMaxWidth()
                            .clickable { onPick(rom) }
                            .padding(vertical = 10.dp),
                    ) {
                        Text(rom.name, fontSize = 15.sp)
                        rom.folderLabel?.let { Text(it, fontSize = 11.sp, color = PineGlowMist.copy(alpha = 0.6f)) }
                    }
                }
            }
        },
        confirmButton = {},
        dismissButton = {
            TextButton(onClick = onDismiss) { Text(stringResource(R.string.import_saves_cancel), color = PineGlowMist) }
        },
    )
}

@Composable
private fun RaLoginDialog(onDismiss: () -> Unit) {
    var username by remember { mutableStateOf("") }
    var password by remember { mutableStateOf("") }
    var error by remember { mutableStateOf<String?>(null) }
    val user by RaEngine.user.collectAsState()

    // Closes itself when the sign-in lands, rather than making the player
    // dismiss a dialog that has already done its job.
    LaunchedEffect(user) { if (user != null) onDismiss() }
    LaunchedEffect(Unit) { RaEngine.loginError.collect { error = it } }

    AlertDialog(
        onDismissRequest = onDismiss,
        containerColor = NightPanel,
        titleContentColor = GoldenSaplight,
        textContentColor = PineGlowMist,
        title = { Text("RetroAchievements", fontWeight = FontWeight.Bold) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                Text(
                    "Sign in with your own account from retroachievements.org. " +
                        "The password is sent once to get a login token; only " +
                        "the token is kept on this phone.",
                    fontSize = 12.sp,
                )
                OutlinedTextField(
                    value = username,
                    onValueChange = { username = it; error = null },
                    label = { Text("Username") },
                    singleLine = true,
                )
                OutlinedTextField(
                    value = password,
                    onValueChange = { password = it; error = null },
                    label = { Text("Password") },
                    singleLine = true,
                    visualTransformation = PasswordVisualTransformation(),
                )
                error?.let { Text(it, color = Error, fontSize = 12.sp) }
            }
        },
        confirmButton = {
            TextButton(
                onClick = { RaEngine.login(username.trim(), password) },
                enabled = username.isNotBlank() && password.isNotBlank(),
            ) { Text("Sign in", color = GoldenSaplight) }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) { Text("Cancel", color = PineGlowMist) }
        },
    )
}

@Composable
fun SettingsSection(
    title: String,
    content: @Composable ColumnScope.() -> Unit,
) {
    Column {
        Text(
            text = title,
            fontSize = 14.sp,
            fontWeight = FontWeight.SemiBold,
            color = AmberResin,
            modifier = Modifier.padding(start = 4.dp, bottom = 8.dp)
        )
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .clip(RoundedCornerShape(12.dp))
                .background(NightPanel),
        ) {
            content()
        }
    }
}

/**
 * Where a settings row's dropdown opens.
 *
 * `DropdownMenu` anchors to the top-left of whatever it is nested in, which
 * for a full-width row is the far left edge, under the icon - a menu floating
 * clear of the words it belongs to. A row lays its title out at
 * 16.dp padding + 24.dp icon + 16.dp spacer, so 56.dp lines the menu up with
 * the title text the player just tapped.
 */
private val SettingsMenuOffset = DpOffset(x = 56.dp, y = 0.dp)

/** A settings row that only reports something, with nothing to tap. */
@Composable
fun SettingsInfo(
    icon: androidx.compose.ui.graphics.vector.ImageVector,
    title: String,
    subtitle: String,
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 16.dp, vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Icon(icon, contentDescription = null, tint = AmberResin, modifier = Modifier.size(24.dp))
        Spacer(modifier = Modifier.width(16.dp))
        Column(modifier = Modifier.weight(1f)) {
            Text(text = title, fontSize = 16.sp, color = PineGlowMist)
            Text(
                text = subtitle,
                fontSize = 12.sp,
                color = PineGlowMist.copy(alpha = 0.6f),
            )
        }
    }
}

@Composable
fun SettingsItem(
    icon: androidx.compose.ui.graphics.vector.ImageVector,
    title: String,
    subtitle: String,
    onClick: () -> Unit,
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(onClick = onClick)
            .padding(horizontal = 16.dp, vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Icon(
            icon,
            contentDescription = null,
            tint = AmberResin,
            modifier = Modifier.size(24.dp)
        )
        Spacer(modifier = Modifier.width(16.dp))
        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = title,
                fontSize = 16.sp,
                color = PineGlowMist,
            )
            Text(
                text = subtitle,
                fontSize = 12.sp,
                color = PineGlowMist.copy(alpha = 0.6f),
            )
        }
        Icon(
            Icons.Default.ChevronRight,
            contentDescription = null,
            tint = PineGlowMist.copy(alpha = 0.4f),
        )
    }
}

/** A labelled slider row, matching [SettingsSwitch]'s shape. */
@Composable
fun SettingsSlider(
    icon: androidx.compose.ui.graphics.vector.ImageVector,
    title: String,
    subtitle: String,
    value: Float,
    range: ClosedFloatingPointRange<Float>,
    onValueChange: (Float) -> Unit,
    onValueChangeFinished: () -> Unit,
) {
    Column(modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Icon(icon, contentDescription = null, tint = AmberResin)
            Spacer(modifier = Modifier.width(16.dp))
            Column {
                Text(title, fontSize = 16.sp, color = PineGlowMist)
                Text(subtitle, fontSize = 13.sp, color = AmberResin)
            }
        }
        Slider(
            value = value,
            valueRange = range,
            onValueChange = onValueChange,
            onValueChangeFinished = onValueChangeFinished,
        )
    }
}

@Composable
fun SettingsSwitch(
    icon: androidx.compose.ui.graphics.vector.ImageVector,
    title: String,
    subtitle: String,
    checked: Boolean,
    onCheckedChange: (Boolean) -> Unit,
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 16.dp, vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Icon(
            icon,
            contentDescription = null,
            tint = AmberResin,
            modifier = Modifier.size(24.dp)
        )
        Spacer(modifier = Modifier.width(16.dp))
        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = title,
                fontSize = 16.sp,
                color = PineGlowMist,
            )
            Text(
                text = subtitle,
                fontSize = 12.sp,
                color = PineGlowMist.copy(alpha = 0.6f),
            )
        }
        Switch(
            checked = checked,
            onCheckedChange = onCheckedChange,
            colors = SwitchDefaults.colors(
                checkedThumbColor = GoldenSaplight,
                checkedTrackColor = AmberResin,
                uncheckedThumbColor = PineGlowMist,
                uncheckedTrackColor = NightEdge,
            )
        )
    }
}
