package com.geebeeayy.app.ui.screens

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.combinedClickable
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.input.nestedscroll.nestedScroll
import java.io.File
import com.geebeeayy.app.data.RomHeader
import com.geebeeayy.app.engine.RaEngine
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.Sort
import androidx.compose.material.icons.filled.*
import androidx.compose.material3.*
import androidx.compose.material3.pulltorefresh.PullToRefreshContainer
import androidx.compose.material3.pulltorefresh.rememberPullToRefreshState
import androidx.compose.runtime.*
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.foundation.Image
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.graphics.asImageBitmap
import com.geebeeayy.app.data.RomArtwork
import com.geebeeayy.app.data.CoverArt
import com.geebeeayy.app.data.DisplaySettings
import com.geebeeayy.app.data.Favorites
import com.geebeeayy.app.data.LastPlayed
import com.geebeeayy.app.data.RelativeTime
import com.geebeeayy.app.data.RomEntry
import com.geebeeayy.app.data.RomFiles
import com.geebeeayy.app.data.RomPlaceholder
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.style.TextAlign
import com.geebeeayy.app.ui.theme.*

/** A sort order for the ROM list, plus the comparator that applies it. */
enum class RomSortOrder(val label: String) {
    LAST_PLAYED_DESC("Recently Played"),
    LAST_PLAYED_ASC("Least Played"),
    NAME_ASC("Name (A-Z)"),
    NAME_DESC("Name (Z-A)"),
    SIZE_DESC("Size (largest)"),
    SIZE_ASC("Size (smallest)"),
    DATE_DESC("Newest"),
    DATE_ASC("Oldest");

    fun sort(roms: List<RomEntry>): List<RomEntry> = when (this) {
        // There is no play-count, only a last-played time, so "least played"
        // reads as "least *recently* played" - never-played ROMs (no
        // recorded timestamp) count as the least played of all and sort
        // first.
        LAST_PLAYED_DESC -> roms.sortedByDescending { it.lastPlayedMillis ?: Long.MIN_VALUE }
        LAST_PLAYED_ASC -> roms.sortedBy { it.lastPlayedMillis ?: Long.MIN_VALUE }
        NAME_ASC -> roms.sortedBy { it.name.lowercase() }
        NAME_DESC -> roms.sortedByDescending { it.name.lowercase() }
        SIZE_DESC -> roms.sortedByDescending { it.sizeBytes }
        SIZE_ASC -> roms.sortedBy { it.sizeBytes }
        DATE_DESC -> roms.sortedByDescending { it.dateModifiedMillis }
        DATE_ASC -> roms.sortedBy { it.dateModifiedMillis }
    }
}

@OptIn(ExperimentalMaterial3Api::class, ExperimentalFoundationApi::class)
@Composable
fun RomBrowserScreen(
    roms: List<RomEntry>,
    onRomClick: (RomEntry) -> Unit,
    onSettingsClick: () -> Unit,
    onDownloadClick: () -> Unit,
    onRefresh: () -> Unit = {},
) {
    val context = LocalContext.current
    val favorites = remember { Favorites(context) }
    // Read once for the whole list. Built per row it was a SharedPreferences
    // object allocated for every card that scrolled into view.
    val downloadCovers = remember { DisplaySettings(context).getDownloadCoverArt() }
    var query by remember { mutableStateOf("") }
    var infoRom by remember { mutableStateOf<RomEntry?>(null) }
    var menuRom by remember { mutableStateOf<RomEntry?>(null) }
    var confirmDeleteSaves by remember { mutableStateOf<RomEntry?>(null) }
    var confirmDeleteRom by remember { mutableStateOf<RomEntry?>(null) }
    var favoritesOnly by remember { mutableStateOf(false) }
    // The scan is a coroutine on an IO dispatcher and gives no completion
    // signal back, so the spinner is held for a beat rather than until the
    // list changes: a rescan that finds nothing new changes nothing, and a
    // spinner waiting for that would never stop.
    val refreshState = rememberPullToRefreshState()
    var sortOrder by remember { mutableStateOf(RomSortOrder.LAST_PLAYED_DESC) }
    var sortMenuOpen by remember { mutableStateOf(false) }

    val visibleRoms = remember(roms, query, sortOrder, favoritesOnly) {
        var filtered = if (query.isBlank()) {
            roms
        } else {
            roms.filter { it.name.contains(query, ignoreCase = true) }
        }
        if (favoritesOnly) filtered = filtered.filter { it.isFavorite }
        sortOrder.sort(filtered)
    }
    Scaffold(
        topBar = {
            TopAppBar(
                // The wordmark alone. A fifth action button went in beside
                // it and the old "GeeBeeAyy! ROMs" pair had nowhere left to
                // go, so it wrapped mid-word to "RO / Ms". The list under it
                // says what the screen is.
                title = {
                    Text(
                        text = "GeeBeeAyy!",
                        fontWeight = FontWeight.Bold,
                        color = GoldenSaplight,
                        maxLines = 1,
                    )
                },
                colors = TopAppBarDefaults.topAppBarColors(
                    containerColor = NightVoid,
                    titleContentColor = PineGlowMist,
                ),
                actions = {
                    // A filter, not a sort: the Favorites section at the top
                    // of the list already answers "what did I star"; this
                    // answers "show me only those" on a long list.
                    IconButton(onClick = { favoritesOnly = !favoritesOnly }) {
                        Icon(
                            if (favoritesOnly) Icons.Default.Favorite else Icons.Default.FavoriteBorder,
                            contentDescription = if (favoritesOnly) "Show all games" else "Show favourites only",
                            tint = if (favoritesOnly) GoldenSaplight else AmberResin,
                        )
                    }
                    Box {
                        IconButton(onClick = { sortMenuOpen = true }) {
                            Icon(
                                Icons.AutoMirrored.Filled.Sort,
                                contentDescription = "Sort",
                                tint = AmberResin
                            )
                        }
                        DropdownMenu(expanded = sortMenuOpen, onDismissRequest = { sortMenuOpen = false }) {
                            RomSortOrder.entries.forEach { option ->
                                DropdownMenuItem(
                                    text = { Text(option.label) },
                                    onClick = {
                                        sortOrder = option
                                        sortMenuOpen = false
                                    },
                                    leadingIcon = if (option == sortOrder) {
                                        { Icon(Icons.Default.Check, contentDescription = null) }
                                    } else null,
                                )
                            }
                        }
                    }
                    IconButton(onClick = onRefresh) {
                        Icon(
                            Icons.Default.Refresh,
                            contentDescription = "Rescan ROM folders",
                            tint = AmberResin
                        )
                    }
                    IconButton(onClick = onDownloadClick) {
                        Icon(
                            Icons.Default.Download,
                            contentDescription = "Homebrew downloads",
                            tint = AmberResin
                        )
                    }
                    IconButton(onClick = onSettingsClick) {
                        Icon(
                            Icons.Default.Settings,
                            contentDescription = "Settings",
                            tint = AmberResin
                        )
                    }
                }
            )
        },
        containerColor = NightVoid
    ) { padding ->
        if (roms.isEmpty()) {
            EmptyState(modifier = Modifier.padding(padding))
        } else {
            Column(modifier = Modifier.padding(padding)) {
                OutlinedTextField(
                    value = query,
                    onValueChange = { query = it },
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(horizontal = 16.dp, vertical = 8.dp),
                    placeholder = { Text("Filter games...") },
                    leadingIcon = { Icon(Icons.Default.Search, contentDescription = null) },
                    trailingIcon = if (query.isNotEmpty()) {
                        {
                            IconButton(onClick = { query = "" }) {
                                Icon(Icons.Default.Close, contentDescription = "Clear filter")
                            }
                        }
                    } else null,
                    singleLine = true,
                    colors = OutlinedTextFieldDefaults.colors(
                        focusedTextColor = PineGlowMist,
                        unfocusedTextColor = PineGlowMist,
                        focusedBorderColor = AmberResin,
                        unfocusedBorderColor = AmberResin.copy(alpha = 0.5f),
                        cursorColor = AmberResin,
                    ),
                )

                if (visibleRoms.isEmpty()) {
                    Text(
                        text = if (favoritesOnly && query.isBlank()) {
                            "No favourites yet. Long press a game to star it."
                        } else {
                            "No games match \"$query\""
                        },
                        color = PineGlowMist.copy(alpha = 0.7f),
                        modifier = Modifier.padding(16.dp),
                    )
                } else {
                    // Clipped on purpose. At rest the spinner is translated
                    // up by its own height, and without a clip it drew as a
                    // dark disc parked on the filter field above.
                    Box(
                        modifier = Modifier
                            .clipToBounds()
                            .nestedScroll(refreshState.nestedScrollConnection)
                    ) {
                    LazyColumn(
                        modifier = Modifier.fillMaxSize(),
                        contentPadding = PaddingValues(16.dp),
                        verticalArrangement = Arrangement.spacedBy(12.dp),
                    ) {
                        val favorites = visibleRoms.filter { it.isFavorite }
                        if (favorites.isNotEmpty()) {
                            item {
                                Text(
                                    text = "Favorites",
                                    fontSize = 16.sp,
                                    fontWeight = FontWeight.SemiBold,
                                    color = AmberResin,
                                    modifier = Modifier.padding(bottom = 4.dp)
                                )
                            }
                            items(favorites) { rom ->
                                RomCard(
                                    rom = rom,
                                    downloadCovers = downloadCovers,
                                    onClick = { onRomClick(rom) },
                                    onLongClick = { menuRom = rom },
                                )
                            }
                        }
                        val rest = visibleRoms.filterNot { it.isFavorite }
                        // The second heading only exists because the first
                        // one does. Without it the favourites ran straight
                        // into the rest of the list, and the first unstarred
                        // game read as though it were starred too.
                        if (favorites.isNotEmpty() && rest.isNotEmpty()) {
                            item {
                                Text(
                                    text = "All games",
                                    fontSize = 16.sp,
                                    fontWeight = FontWeight.SemiBold,
                                    color = AmberResin,
                                    modifier = Modifier.padding(top = 8.dp, bottom = 4.dp)
                                )
                            }
                        }
                        // The rest, not all of them: `favorites` is a subset of
                        // `visibleRoms`, so listing the whole list here drew
                        // every favourite a second time.
                        items(rest) { rom ->
                            RomCard(
                                rom = rom,
                                downloadCovers = downloadCovers,
                                onClick = { onRomClick(rom) },
                                onLongClick = { menuRom = rom },
                            )
                        }
                    }
                    PullToRefreshContainer(
                        state = refreshState,
                        modifier = Modifier.align(Alignment.TopCenter),
                        containerColor = NightPanel,
                        contentColor = GoldenSaplight,
                    )
                    }
                }
            }
        }
    }

    if (refreshState.isRefreshing) {
        LaunchedEffect(Unit) {
            onRefresh()
            kotlinx.coroutines.delay(600)
            refreshState.endRefresh()
        }
    }

    menuRom?.let { rom ->
        RomActionsSheet(
            rom = rom,
            onDismiss = { menuRom = null },
            onPlay = { menuRom = null; if (rom.exists) onRomClick(rom) },
            onToggleFavorite = {
                favorites.toggle(rom.filePath)
                menuRom = null
                onRefresh()
            },
            onInfo = { menuRom = null; infoRom = rom },
            onDeleteSaves = { menuRom = null; confirmDeleteSaves = rom },
            onDeleteRom = { menuRom = null; confirmDeleteRom = rom },
        )
    }

    confirmDeleteSaves?.let { rom ->
        val files = remember(rom.filePath) { RomFiles.saveData(context, rom.filePath) }
        ConfirmDialog(
            title = "Delete save data?",
            // Named, not counted: "3 files" is not something anyone can agree
            // to, and the battery save is the one that cannot be got back.
            body = if (files.isEmpty()) {
                "${rom.name} has no save data on this phone."
            } else {
                "This removes the battery save and every save state for " +
                    "${rom.name}, and cannot be undone:\n\n" +
                    files.joinToString("\n") { it.name }
            },
            confirmLabel = if (files.isEmpty()) "OK" else "Delete",
            destructive = files.isNotEmpty(),
            onConfirm = {
                if (files.isNotEmpty()) RomFiles.deleteSaveData(context, rom.filePath)
                confirmDeleteSaves = null
                onRefresh()
            },
            onDismiss = { confirmDeleteSaves = null },
        )
    }

    confirmDeleteRom?.let { rom ->
        ConfirmDialog(
            title = if (rom.exists) "Delete game?" else "Remove from list?",
            body = if (rom.exists) {
                "This deletes ${rom.fileName} from this phone, along with its " +
                    "save data and any cover art. It cannot be undone."
            } else {
                "${rom.name} is already gone from storage. This only forgets " +
                    "the row, so it stops appearing here."
            },
            confirmLabel = if (rom.exists) "Delete" else "Remove",
            destructive = rom.exists,
            onConfirm = {
                if (rom.exists) RomFiles.deleteRom(context, rom.filePath)
                LastPlayed(context).forget(rom.filePath)
                favorites.forget(rom.filePath)
                confirmDeleteRom = null
                onRefresh()
            },
            onDismiss = { confirmDeleteRom = null },
        )
    }

    infoRom?.let { rom ->
        RomInfoDialog(rom = rom, onDismiss = { infoRom = null })
    }
}

/**
 * What a long press on a row offers.
 *
 * A bottom sheet rather than a dropdown: the list is scrolled with a thumb at
 * the bottom of a tall phone, and a menu anchored to the row lands under the
 * finger that opened it.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun RomActionsSheet(
    rom: RomEntry,
    onDismiss: () -> Unit,
    onPlay: () -> Unit,
    onToggleFavorite: () -> Unit,
    onInfo: () -> Unit,
    onDeleteSaves: () -> Unit,
    onDeleteRom: () -> Unit,
) {
    ModalBottomSheet(onDismissRequest = onDismiss, containerColor = NightPanel) {
        Text(
            text = rom.name,
            fontSize = 16.sp,
            fontWeight = FontWeight.SemiBold,
            color = GoldenSaplight,
            maxLines = 2,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.padding(horizontal = 24.dp, vertical = 8.dp),
        )
        if (rom.exists) {
            SheetItem(Icons.Default.PlayArrow, "Play", onPlay)
        }
        SheetItem(
            if (rom.isFavorite) Icons.Default.Favorite else Icons.Default.FavoriteBorder,
            if (rom.isFavorite) "Remove from favourites" else "Add to favourites",
            onToggleFavorite,
        )
        SheetItem(Icons.Default.Info, "Show information", onInfo)
        SheetItem(Icons.Default.DeleteSweep, "Delete save data", onDeleteSaves)
        SheetItem(
            Icons.Default.Delete,
            if (rom.exists) "Delete game" else "Remove from list",
            onDeleteRom,
            tint = Error,
        )
        Spacer(modifier = Modifier.height(16.dp))
    }
}

@Composable
private fun SheetItem(
    icon: androidx.compose.ui.graphics.vector.ImageVector,
    label: String,
    onClick: () -> Unit,
    tint: Color = AmberResin,
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(onClick = onClick)
            .padding(horizontal = 24.dp, vertical = 14.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Icon(icon, contentDescription = null, tint = tint)
        Spacer(modifier = Modifier.width(20.dp))
        Text(label, color = PineGlowMist, fontSize = 15.sp)
    }
}

/** One prompt shape for both deletions, so they cannot drift apart. */
@Composable
private fun ConfirmDialog(
    title: String,
    body: String,
    confirmLabel: String,
    destructive: Boolean,
    onConfirm: () -> Unit,
    onDismiss: () -> Unit,
) {
    AlertDialog(
        onDismissRequest = onDismiss,
        containerColor = NightPanel,
        titleContentColor = GoldenSaplight,
        textContentColor = PineGlowMist,
        title = { Text(title, fontWeight = FontWeight.Bold) },
        text = { Text(body, fontSize = 13.sp) },
        confirmButton = {
            TextButton(onClick = onConfirm) {
                Text(confirmLabel, color = if (destructive) Error else GoldenSaplight)
            }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) { Text("Cancel", color = PineGlowMist) }
        },
    )
}

private fun formatLastPlayed(millis: Long): String =
    java.text.SimpleDateFormat("d MMM, HH:mm", java.util.Locale.getDefault()).format(millis)

@OptIn(ExperimentalFoundationApi::class)
@Composable
fun RomCard(
    rom: RomEntry,
    downloadCovers: Boolean = false,
    onClick: () -> Unit,
    onLongClick: () -> Unit = {},
) {
    Card(
        modifier = Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(12.dp))
            .combinedClickable(
                // A missing file has nothing to open; the long press still
                // works, which is where "Remove from list" lives.
                onClick = { if (rom.exists) onClick() },
                onLongClick = onLongClick,
            ),
        colors = CardDefaults.cardColors(
            containerColor = NightPanel,
        ),
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(16.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            // Cover art if the player supplied one next to the ROM,
            // otherwise the cartridge placeholder.
            //
            // Loaded in a LaunchedEffect rather than straight in composition:
            // `RomArtwork.load` probes several paths and decodes a bitmap, and
            // doing that synchronously meant every row scrolling into view did
            // disk I/O and a decode on the UI thread. The placeholder shows
            // until it arrives.
            val context = LocalContext.current
            var artwork by remember(rom.filePath) { mutableStateOf<android.graphics.Bitmap?>(null) }
            LaunchedEffect(rom.filePath) {
                artwork = withContext(Dispatchers.IO) {
                    // What is already on disk first, so a row draws without
                    // waiting for the network even when the fetch is on.
                    RomArtwork.load(context, rom.filePath)
                        ?: if (rom.exists && downloadCovers) {
                            CoverArt.fetch(context, rom.filePath)
                            RomArtwork.load(context, rom.filePath)
                        } else {
                            null
                        }
                }
            }
            val art = artwork
            Box(
                modifier = Modifier
                    .size(56.dp)
                    .clip(RoundedCornerShape(8.dp))
                    .background(
                        // The game's own colour when there is no cover art.
                        // Almost nobody has a picture next to their ROMs, so
                        // the old shared placeholder drew the same grey
                        // cartridge down the whole list and gave a thumb
                        // nothing to aim at.
                        if (art != null || !rom.exists) {
                            AmberResin.copy(alpha = 0.3f)
                        } else {
                            Color.hsv(RomPlaceholder.hue(rom.name).toFloat(), 0.55f, 0.42f)
                        }
                    ),
                contentAlignment = Alignment.Center,
            ) {
                when {
                    !rom.exists -> Icon(
                        Icons.Default.SearchOff,
                        contentDescription = null,
                        tint = PineGlowMist.copy(alpha = 0.4f),
                        modifier = Modifier.size(28.dp),
                    )
                    art != null -> Image(
                        bitmap = art.asImageBitmap(),
                        contentDescription = null,
                        contentScale = ContentScale.Crop,
                        modifier = Modifier.fillMaxSize(),
                    )
                    else -> Text(
                        text = RomPlaceholder.initials(rom.name),
                        color = BeeWing,
                        fontSize = 20.sp,
                        fontWeight = FontWeight.Bold,
                        textAlign = TextAlign.Center,
                    )
                }
            }

            Spacer(modifier = Modifier.width(16.dp))

            Column(modifier = Modifier.weight(1f)) {
                Text(
                    text = rom.name,
                    fontSize = 16.sp,
                    fontWeight = FontWeight.SemiBold,
                    color = PineGlowMist,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                Text(
                    text = if (rom.exists) "${rom.fileName} • ${rom.size}" else "File not found",
                    fontSize = 12.sp,
                    color = if (rom.exists) AmberResin else Error,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                if (rom.lastPlayedMillis != null) {
                    Text(
                        // An age, not a date. "6 days ago" is read at a
                        // glance; "3 Sep, 21:41" is read by doing arithmetic
                        // against today. The exact time is still in the info
                        // dialog, one long press away.
                        text = "Last played ${RelativeTime.ago(rom.lastPlayedMillis)}",
                        fontSize = 11.sp,
                        color = PineGlowMist.copy(alpha = 0.6f),
                    )
                }
            }

            // The whole row opens the game, so a play arrow beside the
            // heart said the same thing twice and took width off the title,
            // which is the part a thumb is actually aiming at.
            if (rom.isFavorite) {
                Icon(
                    Icons.Default.Favorite,
                    contentDescription = "Favorite",
                    tint = GoldenSaplight,
                    modifier = Modifier.size(20.dp)
                )
            }
        }
    }
}

@Composable
fun EmptyState(modifier: Modifier = Modifier) {
    Column(
        modifier = modifier.fillMaxSize(),
        verticalArrangement = Arrangement.Center,
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Icon(
            Icons.Default.VideogameAsset,
            contentDescription = null,
            tint = AmberResin.copy(alpha = 0.5f),
            modifier = Modifier.size(80.dp)
        )

        Spacer(modifier = Modifier.height(16.dp))

        Text(
            text = "No ROMs yet!",
            fontSize = 24.sp,
            fontWeight = FontWeight.Bold,
            color = GoldenSaplight,
        )

        Spacer(modifier = Modifier.height(8.dp))

        Text(
            text = "Go to Settings → ROM Folders\nto add your game folders,\nor tap ⬇ for free homebrew.\n\nBzzt! Your games await!",
            fontSize = 14.sp,
            color = PineGlowMist.copy(alpha = 0.7f),
            lineHeight = 20.sp,
        )
    }
}

/**
 * What the app knows about one ROM, on a long press.
 *
 * The cartridge title and game code come from the file's own header rather
 * than its name, because they are what the emulator keys saves on - a file
 * renamed on disk keeps its saves, and this dialog is where that becomes
 * visible.
 */
@Composable
fun RomInfoDialog(rom: RomEntry, onDismiss: () -> Unit) {
    var header by remember(rom.filePath) { mutableStateOf<RomHeader?>(null) }
    // The identity RetroAchievements keys on, for a cart that has one. Read
    // here rather than at scan time: it hashes the whole ROM, which is up to
    // 32 MB, and nothing needs it until somebody asks for this dialog.
    var raHash by remember(rom.filePath) { mutableStateOf<String?>(null) }
    LaunchedEffect(rom.filePath) {
        header = withContext(Dispatchers.IO) { RomHeader.read(File(rom.filePath)) }
        raHash = withContext(Dispatchers.IO) {
            runCatching { RaEngine.hashRom(File(rom.filePath).readBytes()) }.getOrNull()
        }
    }

    AlertDialog(
        onDismissRequest = onDismiss,
        containerColor = NightRaised,
        titleContentColor = GoldenSaplight,
        textContentColor = PineGlowMist,
        title = { Text("Game info", fontWeight = FontWeight.Bold) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                InfoRow("Name", rom.name)
                InfoRow("File", rom.fileName)
                InfoRow("Location", File(rom.filePath).parent ?: "-")
                InfoRow("File size", rom.size)
                InfoRow("Cartridge title", header?.title?.ifBlank { "-" } ?: "Reading...")
                InfoRow("Game code", header?.gameCode?.ifBlank { "-" } ?: "Reading...")
                InfoRow(
                    "Last played",
                    rom.lastPlayedMillis?.let { formatLastPlayed(it) } ?: "Never",
                )
                if (rom.exists) {
                    InfoRow("Achievements hash", raHash ?: "Reading...")
                }
            }
        },
        confirmButton = {
            TextButton(onClick = onDismiss) {
                Text("OK", color = GoldenSaplight)
            }
        },
    )
}

@Composable
private fun InfoRow(label: String, value: String) {
    Column {
        Text(
            text = label,
            fontSize = 12.sp,
            color = AmberResin,
        )
        Text(
            text = value,
            fontSize = 14.sp,
            color = PineGlowMist,
        )
    }
}
