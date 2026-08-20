package com.geebeeayyayy.app

import android.content.Intent
import android.net.Uri
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.compose.setContent
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.runtime.*
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.navigation.NavType
import androidx.navigation.compose.*
import androidx.navigation.navArgument
import com.geebeeayyayy.app.ui.screens.*
import com.geebeeayyayy.app.ui.theme.GeeBeeAyyTheme
import com.geebeeayyayy.app.viewmodel.EmulationViewModel

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContent {
            GeeBeeAyyTheme(darkTheme = true) {
                GeeBeeAyyNavHost()
            }
        }
    }
}

@Composable
fun GeeBeeAyyNavHost() {
    val navController = rememberNavController()

    NavHost(navController = navController, startDestination = "splash") {
        composable("splash") {
            SplashScreen(
                onTimeout = {
                    navController.navigate("rom_browser") {
                        popUpTo("splash") { inclusive = true }
                    }
                }
            )
        }

        composable("rom_browser") {
            var romList by remember { mutableStateOf(loadSavedRoms()) }
            var showFilePicker by remember { mutableStateOf(false) }

            val filePickerLauncher = rememberLauncherForActivityResult(
                contract = ActivityResultContracts.OpenDocument()
            ) { uri: Uri? ->
                uri?.let {
                    // TODO: Copy ROM to app storage and add to list
                    // For now, just add a placeholder entry
                    val name = getFileNameFromUri(it) ?: "Unknown ROM"
                    romList = romList + RomEntry(
                        name = name,
                        fileName = name.lowercase().replace(" ", "_") + ".gba",
                        size = "Unknown",
                        lastPlayed = null,
                        isFavorite = false,
                    )
                }
            }

            RomBrowserScreen(
                roms = romList,
                onRomClick = { rom ->
                    navController.navigate("emulation/${rom.fileName}")
                },
                onSettingsClick = {
                    navController.navigate("settings")
                },
                onAboutClick = { },
            )

            // File picker trigger
            if (showFilePicker) {
                filePickerLauncher.launch(arrayOf("application/*", "application/octet-stream"))
                showFilePicker = false
            }
        }

        composable(
            "emulation/{romName}",
            arguments = listOf(navArgument("romName") { type = NavType.StringType })
        ) { backStackEntry ->
            val romName = backStackEntry.arguments?.getString("romName") ?: ""
            val viewModel: EmulationViewModel = viewModel()

            // Collect state from ViewModel
            val frameBuffer by viewModel.frameBuffer.collectAsState()
            val isRunning by viewModel.isRunning.collectAsState()
            val isFastForward by viewModel.isFastForward.collectAsState()

            EmulationScreen(
                frameBuffer = frameBuffer,
                onBack = {
                    viewModel.stopEmulation()
                    navController.popBackStack()
                },
                onPause = { viewModel.togglePause() },
                onFastForward = { viewModel.toggleFastForward() },
                onSaveState = { slot -> viewModel.saveState(slot) },
                onLoadState = { slot -> viewModel.loadState(slot) },
            )
        }

        composable("settings") {
            SettingsScreen(
                onBack = { navController.popBackStack() }
            )
        }
    }
}

private fun loadSavedRoms(): List<RomEntry> {
    // TODO: Load from app's internal storage
    return listOf(
        RomEntry("Pokemon Emerald", "pokemon_emerald.gba", "16 MB", "2 hours ago", true),
        RomEntry("Zelda: Minish Cap", "zelda_minish.gba", "16 MB", "Yesterday", true),
        RomEntry("Mario Kart", "mario_kart.gba", "8 MB", null, false),
        RomEntry("Metroid Fusion", "metroid_fusion.gba", "16 MB", null, false),
        RomEntry("Fire Emblem", "fire_emblem.gba", "16 MB", "Last week", false),
    )
}

private fun getFileNameFromUri(uri: Uri): String? {
    // Simple extraction - in production, use DocumentFile
    return uri.lastPathSegment?.split("/")?.lastOrNull()
}
