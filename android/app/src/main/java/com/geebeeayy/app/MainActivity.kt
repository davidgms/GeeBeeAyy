package com.geebeeayy.app

import android.Manifest
import android.content.Intent
import android.content.pm.ActivityInfo
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.os.Environment
import android.provider.Settings
import android.util.Log
import androidx.activity.ComponentActivity
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.compose.setContent
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.runtime.*
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalLifecycleOwner
import androidx.core.content.ContextCompat
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.navigation.NavType
import androidx.navigation.compose.*
import androidx.navigation.navArgument
import com.geebeeayy.app.data.DisplaySettings
import com.geebeeayy.app.data.RomEntry
import com.geebeeayy.app.data.RomFolderManager
import com.geebeeayy.app.ui.screens.*
import com.geebeeayy.app.ui.theme.GeeBeeAyyTheme
import com.geebeeayy.app.viewmodel.EmulationViewModel
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import java.io.File

private const val TAG = "GeeBeeAyy/Main"

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        // The manifest no longer hard-locks orientation (see AndroidManifest.xml); this is
        // the equivalent lock, driven by a user-togglable setting instead of a fixed value.
        // android:configChanges="orientation|..." on this activity means setting this does not
        // trigger a recreate, so it is safe to call before setContent and again from Settings.
        requestedOrientation = if (DisplaySettings(this).getForcePortrait()) {
            ActivityInfo.SCREEN_ORIENTATION_PORTRAIT
        } else {
            ActivityInfo.SCREEN_ORIENTATION_UNSPECIFIED
        }
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
    var hasStoragePermission by remember {
        mutableStateOf(
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
                Environment.isExternalStorageManager()
            } else {
                ContextCompat.checkSelfPermission(
                    context, Manifest.permission.READ_EXTERNAL_STORAGE
                ) == PackageManager.PERMISSION_GRANTED
            }
        )
    }

    Log.i(TAG, "NavHost init: isFirstLaunch=$isFirstLaunch, hasStoragePermission=$hasStoragePermission")

    val storagePermissionLauncher = rememberLauncherForActivityResult(
        contract = ActivityResultContracts.StartActivityForResult()
    ) {
        hasStoragePermission = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            Environment.isExternalStorageManager()
        } else {
            ContextCompat.checkSelfPermission(
                context, Manifest.permission.READ_EXTERNAL_STORAGE
            ) == PackageManager.PERMISSION_GRANTED
        }
    }

    fun requestStoragePermission() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            if (!Environment.isExternalStorageManager()) {
                val intent = Intent(Settings.ACTION_MANAGE_APP_ALL_FILES_ACCESS_PERMISSION).apply {
                    data = Uri.parse("package:${context.packageName}")
                }
                storagePermissionLauncher.launch(intent)
            }
        } else {
            // For older devices, handled via standard permission request
        }
    }

    fun rescanRoms() {
        if (!hasStoragePermission) {
            requestStoragePermission()
            return
        }
        scope.launch(Dispatchers.IO) {
            romList = folderManager.scanAllFolders()
        }
    }

    LaunchedEffect(hasStoragePermission) {
        if (hasStoragePermission && folderManager.hasFolders()) {
            rescanRoms()
        }
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
            if (!hasStoragePermission) {
                LaunchedEffect(Unit) { requestStoragePermission() }
            } else if (isFirstLaunch) {
                // Show hint to add folder
            }

            RomBrowserScreen(
                roms = romList,
                onRomClick = { rom ->
                    navController.navigate("emulation/${java.net.URLEncoder.encode(rom.filePath, "UTF-8")}")
                },
                onSettingsClick = {
                    navController.navigate("settings")
                },
                onDownloadClick = {
                    navController.navigate("homebrew")
                },
                onAboutClick = { },
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
            val filePath = java.net.URLDecoder.decode(encodedPath, "UTF-8")
            val viewModel: EmulationViewModel = androidx.lifecycle.viewmodel.compose.viewModel()
            // Read once per navigation to this route rather than observed live: the only way
            // to change it is the Settings screen, which is a separate back-stack entry, so
            // returning here always recomposes this composable fresh.
            val scaleMode = remember(filePath) { DisplaySettings(context).getScaleMode() }

            val frameBuffer by viewModel.frameBuffer.collectAsState()
            val isLoading by viewModel.isLoading.collectAsState()
            val errorMessage by viewModel.errorMessage.collectAsState()
            val stateMessage by viewModel.stateMessage.collectAsState()
            val isRewinding by viewModel.isRewinding.collectAsState()
            // Re-read per ROM launch, the same as the scale mode above, so a
            // change in Settings takes effect the next time a game is opened.
            val controlScale = remember(filePath) { DisplaySettings(context).getControlScale() }
            val controlOpacity = remember(filePath) { DisplaySettings(context).getControlOpacity() }
            val screenFilter = remember(filePath) { DisplaySettings(context).getScreenFilter() }

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
                        Lifecycle.Event.ON_STOP -> viewModel.onAppBackgrounded()
                        Lifecycle.Event.ON_START -> viewModel.onAppForegrounded()
                        else -> {}
                    }
                }
                lifecycleOwner.lifecycle.addObserver(observer)
                onDispose { lifecycleOwner.lifecycle.removeObserver(observer) }
            }

            EmulationScreen(
                frameBuffer = frameBuffer,
                isLoading = isLoading,
                errorMessage = errorMessage,
                stateMessage = stateMessage,
                scaleMode = scaleMode,
                screenFilter = screenFilter,
                onDismissStateMessage = { viewModel.clearStateMessage() },
                onBack = {
                    viewModel.stopEmulation()
                    navController.popBackStack()
                },
                onPause = { viewModel.togglePause() },
                onFastForward = { viewModel.toggleFastForward() },
                isRewinding = isRewinding,
                onRewind = { active -> viewModel.setRewinding(active) },
                controlScale = controlScale,
                controlOpacity = controlOpacity,
                onSaveState = { slot -> viewModel.saveState(slot) },
                onLoadState = { slot -> viewModel.loadState(slot) },
                stateSlots = { viewModel.stateSlots() },
                onScreenshot = { viewModel.takeScreenshot() },
                onKeyChange = { key, pressed -> viewModel.setKey(key, pressed) },
            )
        }

        composable("settings") {
            SettingsScreen(
                onBack = { navController.popBackStack() },
                onFoldersChanged = { rescanRoms() }
            )
        }
    }
}
