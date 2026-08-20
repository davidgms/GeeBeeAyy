package com.geebee.app

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.runtime.*
import androidx.navigation.NavType
import androidx.navigation.compose.*
import androidx.navigation.navArgument
import com.geebee.app.ui.screens.*
import com.geebee.app.ui.theme.GeeBeeTheme

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContent {
            GeeBeeTheme(darkTheme = true) {
                GeeBeeNavHost()
            }
        }
    }
}

@Composable
fun GeeBeeNavHost() {
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
            // Placeholder ROM list
            val sampleRoms = listOf(
                RomEntry("Pokemon Emerald", "pokemon_emerald.gba", "16 MB", "2 hours ago", true),
                RomEntry("Zelda: Minish Cap", "zelda_minish.gba", "16 MB", "Yesterday", true),
                RomEntry("Mario Kart", "mario_kart.gba", "8 MB", null, false),
                RomEntry("Metroid Fusion", "metroid_fusion.gba", "16 MB", null, false),
                RomEntry("Fire Emblem", "fire_emblem.gba", "16 MB", "Last week", false),
            )

            RomBrowserScreen(
                roms = sampleRoms,
                onRomClick = { rom ->
                    navController.navigate("emulation/${rom.fileName}")
                },
                onSettingsClick = {
                    navController.navigate("settings")
                },
                onAboutClick = { },
            )
        }

        composable(
            "emulation/{romName}",
            arguments = listOf(navArgument("romName") { type = NavType.StringType })
        ) { backStackEntry ->
            val romName = backStackEntry.arguments?.getString("romName") ?: ""

            EmulationScreen(
                frameBuffer = null, // Will be connected to engine
                onBack = { navController.popBackStack() },
                onPause = { /* Pause emulation */ },
                onFastForward = { /* Toggle fast forward */ },
                onSaveState = { slot -> /* Save state */ },
                onLoadState = { slot -> /* Load state */ },
            )
        }

        composable("settings") {
            SettingsScreen(
                onBack = { navController.popBackStack() }
            )
        }
    }
}
