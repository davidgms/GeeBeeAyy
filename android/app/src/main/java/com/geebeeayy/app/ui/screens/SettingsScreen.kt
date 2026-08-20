package com.geebeeayyayy.app.ui.screens

import androidx.compose.foundation.background
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
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.geebeeayyayy.app.ui.theme.*

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SettingsScreen(onBack: () -> Unit) {
    var showSaveSlots by remember { mutableStateOf(false) }

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
            // Display Settings
            SettingsSection(title = "Display") {
                SettingsItem(
                    icon = Icons.Default.Star,
                    title = "Screen Scale",
                    subtitle = "2x (Native)",
                    onClick = { }
                )
                SettingsItem(
                    icon = Icons.Default.Tune,
                    title = "Screen Filter",
                    subtitle = "Pixel Perfect",
                    onClick = { }
                )
                SettingsSwitch(
                    icon = Icons.Default.StayCurrentPortrait,
                    title = "Force Portrait",
                    subtitle = "Lock orientation",
                    checked = true,
                    onCheckedChange = { }
                )
            }

            // Audio Settings
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

            // Control Settings
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
                SettingsItem(
                    icon = Icons.Default.Tune,
                    title = "Vibration",
                    subtitle = "On",
                    onClick = { }
                )
            }

            // Save States
            SettingsSection(title = "Save States") {
                SettingsItem(
                    icon = Icons.Default.Save,
                    title = "Save State",
                    subtitle = "Slot 1",
                    onClick = { showSaveSlots = true }
                )
                SettingsItem(
                    icon = Icons.Default.FolderOpen,
                    title = "Load State",
                    subtitle = "Slot 1",
                    onClick = { showSaveSlots = true }
                )
                SettingsItem(
                    icon = Icons.Default.Delete,
                    title = "Manage Saves",
                    subtitle = "10 slots available",
                    onClick = { }
                )
            }

            // Advanced
            SettingsSection(title = "Advanced") {
                SettingsSwitch(
                    icon = Icons.Default.Speed,
                    title = "Fast Forward",
                    subtitle = "Hold button for 2x speed",
                    checked = false,
                    onCheckedChange = { }
                )
                SettingsSwitch(
                    icon = Icons.Default.BugReport,
                    title = "Show FPS",
                    subtitle = "Display frame rate overlay",
                    checked = false,
                    onCheckedChange = { }
                )
            }

            // About
            SettingsSection(title = "About") {
                SettingsItem(
                    icon = Icons.Default.Info,
                    title = "GeeBeeAyy!",
                    subtitle = "v0.1.0 — Bzzt!",
                    onClick = { }
                )
                SettingsItem(
                    icon = Icons.Default.Code,
                    title = "Credits",
                    subtitle = "Open Source GBA Emulator",
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
