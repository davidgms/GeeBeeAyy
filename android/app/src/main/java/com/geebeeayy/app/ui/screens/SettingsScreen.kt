package com.geebeeayy.app.ui.screens

import android.content.Intent
import android.net.Uri
import android.os.Environment
import android.provider.DocumentsContract
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
import com.geebeeayy.app.data.ControlTint
import com.geebeeayy.app.data.ControlLayoutStore
import com.geebeeayy.app.data.DisplaySettings
import com.geebeeayy.app.data.RomFolderManager
import com.geebeeayy.app.data.ScreenOrientation
import com.geebeeayy.app.data.ScaleMode
import com.geebeeayy.app.data.ScreenFilter
import com.geebeeayy.app.engine.AudioOutput
import com.geebeeayy.app.ui.findActivity
import com.geebeeayy.app.ui.orientationFor
import com.geebeeayy.app.ui.theme.*
import java.io.File

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
) {
    val context = LocalContext.current
    val folderManager = remember { RomFolderManager(context) }
    var folders by remember { mutableStateOf(folderManager.getFolderPaths()) }

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
    val layoutStore = remember { ControlLayoutStore(context) }
    var layouts by remember { mutableStateOf(layoutStore.getLayouts()) }
    var defaultLayoutId by remember { mutableStateOf(layoutStore.getDefaultLayoutId()) }
    var showLayoutMenu by remember { mutableStateOf(false) }
    var controlTint by remember { mutableStateOf(displaySettings.getControlTint()) }
    var showTintMenu by remember { mutableStateOf(false) }
    var controlFont by remember { mutableStateOf(displaySettings.getControlFontSize()) }
    var showFontMenu by remember { mutableStateOf(false) }
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

    val folderPicker = rememberLauncherForActivityResult(
        contract = ActivityResultContracts.OpenDocumentTree()
    ) { uri: Uri? ->
        uri?.let {
            // A non-primary volume (an SD card) has a docId like
            // "1A2B-3C4D:Roms", which maps to /storage/<volume>/... rather
            // than /storage/emulated/0. Only "primary:" used to be handled and
            // everything else fell through in silence, so picking a folder on
            // a card closed the dialog and added nothing, with no explanation.
            val docId = DocumentsContract.getTreeDocumentId(it)
            val volume = docId.substringBefore(':', "")
            val relative = docId.substringAfter(':', "")
            val path = when {
                volume == "primary" -> "/storage/emulated/0/$relative"
                volume.isNotEmpty() -> "/storage/$volume/$relative"
                else -> ""
            }.trimEnd('/')
            val folder = if (path.isEmpty()) null else File(path)
            if (folder != null && folder.isDirectory && folder.canRead()) {
                folderManager.addFolder(path)
                folders = folderManager.getFolderPaths()
                onFoldersChanged()
                folderError = null
            } else {
                folderError = "Could not read that folder. Pick one on internal storage."
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
                        val displayName = File(path).name
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
                            Column(modifier = Modifier.weight(1f)) {
                                Text(
                                    text = displayName,
                                    fontSize = 14.sp,
                                    color = PineGlowMist,
                                )
                                Text(
                                    text = path,
                                    fontSize = 11.sp,
                                    color = PineGlowMist.copy(alpha = 0.4f),
                                )
                            }
                            IconButton(onClick = {
                                folderManager.removeFolder(path)
                                folders = folderManager.getFolderPaths()
                                onFoldersChanged()
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
                    title = "Fullscreen In Game",
                    subtitle = "Hide the status and navigation bars",
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
