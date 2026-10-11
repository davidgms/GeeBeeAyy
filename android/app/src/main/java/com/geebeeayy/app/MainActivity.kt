package com.geebeeayy.app

import android.os.Bundle
import android.util.Log
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.runtime.*
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalLifecycleOwner
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.navigation.NavType
import androidx.navigation.compose.*
import androidx.navigation.navArgument
import androidx.compose.runtime.CompositionLocalProvider
import com.geebeeayy.app.ui.Haptics
import com.geebeeayy.app.ui.theme.LocalControlFontScale
import com.geebeeayy.app.ui.theme.LocalControlPalette
import com.geebeeayy.app.ui.theme.LocalDpadCardinalHalf
import com.geebeeayy.app.data.DisplaySettings
import com.geebeeayy.app.data.RomBytes
import com.geebeeayy.app.data.RomEntry
import com.geebeeayy.app.data.RomFolderManager
import com.geebeeayy.app.ui.screens.*
import com.geebeeayy.app.ui.theme.GeeBeeAyyTheme
import com.geebeeayy.app.viewmodel.EmulationViewModel
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import com.geebeeayy.app.ui.orientationFor
import java.io.File

private const val TAG = "GeeBeeAyy/Main"

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        // The manifest no longer hard-locks orientation (see AndroidManifest.xml); this is
        // the equivalent lock, driven by a user-togglable setting instead of a fixed value.
        // android:configChanges="orientation|..." on this activity means setting this does not
        // trigger a recreate, so it is safe to call before setContent and again from Settings.
        requestedOrientation = orientationFor(DisplaySettings(this).getScreenOrientation())
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
    val context = LocalContext.current
    val folderManager = remember { RomFolderManager(context) }
    val scope = rememberCoroutineScope()

    var romList by remember { mutableStateOf<List<RomEntry>>(emptyList()) }
    var isFirstLaunch by remember { mutableStateOf(!folderManager.hasFolders()) }
    Log.i(TAG, "NavHost init: isFirstLaunch=$isFirstLaunch")

    fun rescanRoms() {
        // Tree folders carry their own grant. A path folder left from before
        // SAF cannot be read any more and shows as "Needs re-pick" in Settings.
        scope.launch(Dispatchers.IO) {
            // Scans are serialised in the manager, and the save import runs
            // there too, on the list it just published: battery saves are
            // moving into app storage while the app can still read the folders.
            folderManager.scanAndImport { romList = it }
        }
    }

    LaunchedEffect(Unit) {
        if (folderManager.hasFolders()) rescanRoms()
    }

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
            RomBrowserScreen(
                roms = romList,
                onRomClick = { rom ->
                    navController.navigate("emulation/${RomBytes.encodeId(rom.filePath)}")
                },
                onSettingsClick = {
                    navController.navigate("settings")
                },
                onDownloadClick = {
                    navController.navigate("homebrew")
                },
                onRefresh = { rescanRoms() },
            )
        }

        composable("homebrew") {
            // The scan that follows a download is the same one the browser
            // uses, so a finished download shows up without leaving the app.
            HomebrewScreen(
                folders = folderManager.getFolderPaths(),
                onBack = { navController.popBackStack() },
                onDownloaded = { rescanRoms() },
            )
        }

        composable(
            "emulation/{filePath}",
            arguments = listOf(navArgument("filePath") { type = NavType.StringType })
        ) { backStackEntry ->
            val encodedPath = backStackEntry.arguments?.getString("filePath") ?: ""
            val filePath = RomBytes.decodeId(encodedPath)
            val viewModel: EmulationViewModel = androidx.lifecycle.viewmodel.compose.viewModel()
            // Read once per navigation to this route rather than observed live: the only way
            // to change it is the Settings screen, which is a separate back-stack entry, so
            // returning here always recomposes this composable fresh.
            val scaleMode = remember(filePath) { DisplaySettings(context).getScaleMode() }

            val isLoading by viewModel.isLoading.collectAsState()
            val errorMessage by viewModel.errorMessage.collectAsState()
            val stateMessage by viewModel.stateMessage.collectAsState()
            val isRewinding by viewModel.isRewinding.collectAsState()
            val fastForward by viewModel.fastForward.collectAsState()
            val soundEnabled by viewModel.soundEnabled.collectAsState()
            val performance by viewModel.performance.collectAsState()
            // Re-read per ROM launch, the same as the scale mode above, so a
            // change in Settings takes effect the next time a game is opened.
            val controlScale = remember(filePath) { DisplaySettings(context).getControlScale() }
            val controlOpacity = remember(filePath) { DisplaySettings(context).getControlOpacity() }
            val screenFilter = remember(filePath) { DisplaySettings(context).getScreenFilter() }
            val controlTint = remember(filePath) { DisplaySettings(context).getControlTint() }
            val controlFont = remember(filePath) { DisplaySettings(context).getControlFontSize() }
            val dpadCardinal = remember(filePath) {
                DisplaySettings(context).getDpadCardinalHalfDegrees()
            }
            val fullscreen = remember(filePath) { DisplaySettings(context).getFullscreenInGame() }
            val showStatusStrip = remember(filePath) {
                DisplaySettings(context).getShowStatusStrip()
            }
            // Re-read on every press, not per ROM: a player changing the
            // strength in Settings wants to feel the difference on the next
            // button, not the next game. The store itself is held, so a press
            // costs one preference read and not a fresh object.
            val haptics = remember { Haptics(context) }
            val settings = remember { DisplaySettings(context) }
            val hapticStrength = { settings.getHapticStrength() }

            LaunchedEffect(filePath) {
                viewModel.loadRomFromPath(filePath)
            }

            // Audio is the timing master while the app is foregrounded; this
            // stops the loop from running (and draining the battery) behind
            // a lock screen or another app, and resumes it on return unless
            // the player had paused it themselves.
            val lifecycleOwner = LocalLifecycleOwner.current
            DisposableEffect(lifecycleOwner, viewModel) {
                val observer = LifecycleEventObserver { _, event ->
                    when (event) {
                        Lifecycle.Event.ON_PAUSE -> viewModel.onAppBackgrounded()
                        Lifecycle.Event.ON_RESUME -> viewModel.onAppForegrounded()
                        else -> {}
                    }
                }
                lifecycleOwner.lifecycle.addObserver(observer)
                onDispose { lifecycleOwner.lifecycle.removeObserver(observer) }
            }

            CompositionLocalProvider(
                LocalControlPalette provides controlTint.palette,
                LocalControlFontScale provides controlFont.scale,
                LocalDpadCardinalHalf provides dpadCardinal,
            ) {
            EmulationScreen(
                frames = viewModel.frameBuffer,
                isLoading = isLoading,
                errorMessage = errorMessage,
                stateMessage = stateMessage,
                scaleMode = scaleMode,
                screenFilter = screenFilter,
                onDismissStateMessage = { viewModel.clearStateMessage() },
                onBack = {
                    viewModel.stopEmulation()
                    navController.popBackStack()
                    // Rescan so the ROM browser picks up the last-played time
                    // just recorded, and can sort by it.
                    rescanRoms()
                },
                onPause = { viewModel.togglePause() },
                onFastForward = { viewModel.toggleFastForward() },
                fastForward = fastForward,
                isRewinding = isRewinding,
                onRewind = { active -> viewModel.setRewinding(active) },
                controlScale = controlScale,
                controlOpacity = controlOpacity,
                onSaveState = { slot -> viewModel.saveState(slot) },
                onLoadState = { slot -> viewModel.loadState(slot) },
                stateSlots = { viewModel.stateSlots() },
                onScreenshot = { viewModel.takeScreenshot() },
                fullscreen = fullscreen,
                showStatusStrip = showStatusStrip,
                soundEnabled = soundEnabled,
                performance = performance,
                onToggleSound = { viewModel.toggleSound() },
                onReset = { viewModel.reset() },
                onSettings = {
                    // Pausing first is what makes coming back work: the loop
                    // would otherwise keep running behind Settings, and
                    // `loadRomFromPath` resumes this same session on return
                    // rather than reloading the ROM.
                    viewModel.onAppBackgrounded()
                    navController.navigate("settings")
                },
                onKeyChange = { key, pressed ->
                    // Key first, buzz second: `Vibrator.vibrate` is a
                    // blocking binder call into the system, and input latency
                    // is the product here.
                    viewModel.setKey(key, pressed)
                    if (pressed) haptics.press(hapticStrength())
                },
                gameKey = { viewModel.currentRomKey() },
            )
            }
        }

        composable("settings") {
            SettingsScreen(
                onBack = { navController.popBackStack() },
                onFoldersChanged = { rescanRoms() }
            )
        }
    }
}
