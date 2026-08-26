package com.geebeeayy.app

import android.Manifest
import android.content.Intent
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
import androidx.core.content.ContextCompat
import androidx.navigation.NavType
import androidx.navigation.compose.*
import androidx.navigation.navArgument
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
                onAboutClick = { },
            )
        }

        composable(
            "emulation/{filePath}",
            arguments = listOf(navArgument("filePath") { type = NavType.StringType })
        ) { backStackEntry ->
            val encodedPath = backStackEntry.arguments?.getString("filePath") ?: ""
            val filePath = java.net.URLDecoder.decode(encodedPath, "UTF-8")
            val viewModel: EmulationViewModel = androidx.lifecycle.viewmodel.compose.viewModel()

            val frameBuffer by viewModel.frameBuffer.collectAsState()
            val isLoading by viewModel.isLoading.collectAsState()
            val errorMessage by viewModel.errorMessage.collectAsState()

            LaunchedEffect(filePath) {
                viewModel.loadRomFromPath(filePath)
            }

            EmulationScreen(
                frameBuffer = frameBuffer,
                isLoading = isLoading,
                errorMessage = errorMessage,
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
                onBack = { navController.popBackStack() },
                onFoldersChanged = { rescanRoms() }
            )
        }
    }
}
