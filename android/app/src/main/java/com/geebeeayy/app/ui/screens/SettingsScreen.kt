package com.geebeeayy.app.ui.screens

import android.app.Activity
import android.content.pm.ActivityInfo
import android.net.Uri
import android.os.Environment
import android.provider.DocumentsContract
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
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.geebeeayy.app.data.DisplaySettings
import com.geebeeayy.app.data.RomFolderManager
import com.geebeeayy.app.data.ScaleMode
import com.geebeeayy.app.ui.theme.*
import java.io.File

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
    var controlScale by remember { mutableFloatStateOf(displaySettings.getControlScale()) }
    var controlOpacity by remember { mutableFloatStateOf(displaySettings.getControlOpacity()) }
    var showScaleMenu by remember { mutableStateOf(false) }
    var forcePortrait by remember { mutableStateOf(displaySettings.getForcePortrait()) }

    val folderPicker = rememberLauncherForActivityResult(
        contract = ActivityResultContracts.OpenDocumentTree()
    ) { uri: Uri? ->
        uri?.let {
            val docId = DocumentsContract.getTreeDocumentId(it)
            if (docId.startsWith("primary:")) {
                val path = "/storage/emulated/0/" + docId.removePrefix("primary:")
                val folder = File(path)
                if (folder.isDirectory) {
                    folderManager.addFolder(path)
                    folders = folderManager.getFolderPaths()
                    onFoldersChanged()
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
                    containerColor = BurntRoot,
                ),
            )
        },
        containerColor = BurntRoot
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
                SettingsItem(
                    icon = Icons.Default.Tune,
                    title = "Screen Filter",
                    subtitle = "Nearest Neighbor (pixel perfect)",
                    onClick = { }
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
                SettingsSwitch(
                    icon = Icons.Default.StayCurrentPortrait,
                    title = "Force Portrait",
                    subtitle = "Lock orientation",
                    checked = forcePortrait,
                    onCheckedChange = { checked ->
                        forcePortrait = checked
                        displaySettings.setForcePortrait(checked)
                        (context as? Activity)?.requestedOrientation = if (checked) {
                            ActivityInfo.SCREEN_ORIENTATION_PORTRAIT
                        } else {
                            ActivityInfo.SCREEN_ORIENTATION_UNSPECIFIED
                        }
                    }
                )
            }

            SettingsSection(title = "Audio") {
                SettingsSwitch(
                    icon = Icons.Default.VolumeUp,
                    title = "Sound",
                    subtitle = "Enable audio output",
                    checked = true,
                    onCheckedChange = { }
                )
                SettingsItem(
                    icon = Icons.Default.MusicNote,
                    title = "Audio Backend",
                    subtitle = "AAudio",
                    onClick = { }
                )
            }

            SettingsSection(title = "Controls") {
                SettingsItem(
                    icon = Icons.Default.Gamepad,
                    title = "Layout",
                    subtitle = "Default",
                    onClick = { }
                )
                SettingsSwitch(
                    icon = Icons.Default.Bluetooth,
                    title = "Bluetooth Controller",
                    subtitle = "Xbox / PS / Switch Pro",
                    checked = false,
                    onCheckedChange = { }
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
                .background(HoneyDark),
        ) {
            content()
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
                uncheckedTrackColor = HoneyMid,
            )
        )
    }
}
