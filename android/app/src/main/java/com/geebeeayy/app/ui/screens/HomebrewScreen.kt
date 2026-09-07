package com.geebeeayy.app.ui.screens

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.ArrowBack
import androidx.compose.material.icons.filled.CheckCircle
import androidx.compose.material.icons.filled.Download
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.geebeeayy.app.data.HomebrewCatalog
import com.geebeeayy.app.data.HomebrewDownloader
import com.geebeeayy.app.data.HomebrewEntry
import com.geebeeayy.app.ui.theme.*
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.io.File

/**
 * The built-in catalogue of freely distributable homebrew and test ROMs.
 *
 * Everything here is published by its author for free download. Commercial
 * games are not listed and will not be: those belong to their publishers, and
 * the player puts their own copies in the ROM folder themselves.
 */
@Composable
fun HomebrewScreen(
    folders: List<String>,
    onBack: () -> Unit,
    onDownloaded: () -> Unit,
) {
    val scope = rememberCoroutineScope()
    // Which folder downloads land in. With one configured folder there is
    // nothing to choose; with several, picking silently was how a download
    // ended up somewhere the player was not looking.
    var destinationPath by remember(folders) { mutableStateOf(folders.firstOrNull()) }
    val destination = destinationPath?.let { File(it) }
    var busy by remember { mutableStateOf<String?>(null) }
    var progress by remember { mutableFloatStateOf(0f) }
    var message by remember { mutableStateOf<String?>(null) }
    val present = remember { mutableStateMapOf<String, Boolean>() }

    LaunchedEffect(destination) {
        HomebrewCatalog.entries.forEach { entry ->
            present[entry.fileName] =
                destination != null && File(destination, entry.fileName).isFile
        }
    }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .background(NightVoid),
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 8.dp, vertical = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            IconButton(onClick = onBack) {
                Icon(Icons.Default.ArrowBack, "Back", tint = PineGlowMist)
            }
            Text("Homebrew", color = GoldenSaplight, fontSize = 20.sp, fontWeight = FontWeight.Bold)
        }

        if (destination == null) {
            Text(
                text = "Add a ROM folder in Settings first - downloads go there.",
                color = AmberResin,
                fontSize = 14.sp,
                modifier = Modifier.padding(16.dp),
            )
            return@Column
        }

        if (folders.size > 1) {
            Text(
                text = "Download to",
                color = AmberResin,
                fontSize = 12.sp,
                modifier = Modifier.padding(start = 16.dp, top = 4.dp),
            )
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .horizontalScroll(rememberScrollState())
                    .padding(horizontal = 16.dp, vertical = 4.dp),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                folders.forEach { path ->
                    val selected = path == destinationPath
                    FilterChip(
                        selected = selected,
                        onClick = { destinationPath = path },
                        enabled = busy == null,
                        label = { Text(path.substringAfterLast('/'), fontSize = 12.sp) },
                        colors = FilterChipDefaults.filterChipColors(
                            containerColor = NightPanel,
                            labelColor = PineGlowMist,
                            selectedContainerColor = GoldenSaplight,
                            selectedLabelColor = BurntRoot,
                        ),
                    )
                }
            }
        }

        message?.let {
            Text(
                text = it,
                color = PineGlowMist,
                fontSize = 13.sp,
                modifier = Modifier
                    .fillMaxWidth()
                    .background(NightPanel)
                    .padding(horizontal = 16.dp, vertical = 8.dp),
            )
        }

        LazyColumn(
            contentPadding = PaddingValues(16.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            items(HomebrewCatalog.entries) { entry ->
                HomebrewCard(
                    entry = entry,
                    alreadyHave = present[entry.fileName] == true,
                    downloading = busy == entry.fileName,
                    progress = progress,
                    enabled = busy == null,
                    onDownload = {
                        busy = entry.fileName
                        progress = 0f
                        message = null
                        scope.launch {
                            val result = withContext(Dispatchers.IO) {
                                HomebrewDownloader.download(entry, destination) { p ->
                                    progress = p
                                }
                            }
                            busy = null
                            when (result) {
                                is HomebrewDownloader.Result.Done -> {
                                    present[entry.fileName] = true
                                    message = "${entry.name} downloaded"
                                    onDownloaded()
                                }
                                is HomebrewDownloader.Result.Failed ->
                                    message = "${entry.name}: ${result.reason}"
                            }
                        }
                    },
                )
            }
        }
    }
}

@Composable
private fun HomebrewCard(
    entry: HomebrewEntry,
    alreadyHave: Boolean,
    downloading: Boolean,
    progress: Float,
    enabled: Boolean,
    onDownload: () -> Unit,
) {
    Card(
        modifier = Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(12.dp))
            .clickable(enabled = enabled && !alreadyHave, onClick = onDownload),
        colors = CardDefaults.cardColors(containerColor = NightPanel),
    ) {
        Column(modifier = Modifier.padding(16.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Column(modifier = Modifier.weight(1f)) {
                    Text(
                        entry.name,
                        fontSize = 16.sp,
                        fontWeight = FontWeight.SemiBold,
                        color = PineGlowMist,
                    )
                    Text(entry.description, fontSize = 12.sp, color = AmberResin)
                    Text(
                        "${entry.source} • ${entry.approxBytes / 1024} KB",
                        fontSize = 11.sp,
                        color = PineGlowMist.copy(alpha = 0.6f),
                    )
                }
                Spacer(modifier = Modifier.width(12.dp))
                when {
                    alreadyHave -> Icon(
                        Icons.Default.CheckCircle,
                        contentDescription = "Already downloaded",
                        tint = GoldenSaplight,
                    )
                    downloading -> CircularProgressIndicator(
                        modifier = Modifier.size(24.dp),
                        color = GoldenSaplight,
                        strokeWidth = 2.dp,
                    )
                    else -> Icon(
                        Icons.Default.Download,
                        contentDescription = "Download",
                        tint = if (enabled) GoldenSaplight else AmberResin,
                    )
                }
            }
            if (downloading) {
                Spacer(modifier = Modifier.height(8.dp))
                LinearProgressIndicator(
                    progress = { progress },
                    modifier = Modifier.fillMaxWidth(),
                    color = GoldenSaplight,
                )
            }
        }
    }
}
