package com.geebeeayy.app.ui.screens

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.Sort
import androidx.compose.material.icons.filled.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
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
import com.geebeeayy.app.data.RomEntry
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

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun RomBrowserScreen(
    roms: List<RomEntry>,
    onRomClick: (RomEntry) -> Unit,
    onSettingsClick: () -> Unit,
    onDownloadClick: () -> Unit,
    onAboutClick: () -> Unit,
) {
    var query by remember { mutableStateOf("") }
    var sortOrder by remember { mutableStateOf(RomSortOrder.LAST_PLAYED_DESC) }
    var sortMenuOpen by remember { mutableStateOf(false) }

    val visibleRoms = remember(roms, query, sortOrder) {
        val filtered = if (query.isBlank()) {
            roms
        } else {
            roms.filter { it.name.contains(query, ignoreCase = true) }
        }
        sortOrder.sort(filtered)
    }
    Scaffold(
        topBar = {
            TopAppBar(
                title = {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Text(
                            text = "GeeBeeAyy!",
                            fontWeight = FontWeight.Bold,
                            color = GoldenSaplight,
                        )
                        Spacer(modifier = Modifier.width(8.dp))
                        Text(
                            text = "ROMs",
                            fontWeight = FontWeight.Light,
                            color = PineGlowMist,
                        )
                    }
                },
                colors = TopAppBarDefaults.topAppBarColors(
                    containerColor = BurntRoot,
                    titleContentColor = PineGlowMist,
                ),
                actions = {
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
                    IconButton(onClick = onAboutClick) {
                        Icon(
                            Icons.Default.Info,
                            contentDescription = "About",
                            tint = AmberResin
                        )
                    }
                }
            )
        },
        containerColor = BurntRoot
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
                        text = "No games match \"$query\"",
                        color = PineGlowMist.copy(alpha = 0.7f),
                        modifier = Modifier.padding(16.dp),
                    )
                } else {
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
                                RomCard(rom = rom, onClick = { onRomClick(rom) })
                            }
                        }
                        items(visibleRoms) { rom ->
                            RomCard(rom = rom, onClick = { onRomClick(rom) })
                        }
                    }
                }
            }
        }
    }
}

private fun formatLastPlayed(millis: Long): String =
    java.text.SimpleDateFormat("d MMM, HH:mm", java.util.Locale.getDefault()).format(millis)

@Composable
fun RomCard(rom: RomEntry, onClick: () -> Unit) {
    Card(
        modifier = Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(12.dp))
            .clickable(onClick = onClick),
        colors = CardDefaults.cardColors(
            containerColor = HoneyDark,
        ),
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(16.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            // Cover art if the player supplied one next to the ROM,
            // otherwise the cartridge placeholder. Decoding is keyed on the
            // path so scrolling does not re-read the file every frame.
            val artwork = remember(rom.filePath) { RomArtwork.load(rom.filePath) }
            Box(
                modifier = Modifier
                    .size(56.dp)
                    .clip(RoundedCornerShape(8.dp))
                    .background(AmberResin.copy(alpha = 0.3f)),
                contentAlignment = Alignment.Center,
            ) {
                if (artwork != null) {
                    Image(
                        bitmap = artwork.asImageBitmap(),
                        contentDescription = null,
                        contentScale = ContentScale.Crop,
                        modifier = Modifier.fillMaxSize(),
                    )
                } else {
                    Icon(
                        Icons.Default.VideogameAsset,
                        contentDescription = null,
                        tint = GoldenSaplight,
                        modifier = Modifier.size(32.dp)
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
                    text = "${rom.fileName} • ${rom.size}",
                    fontSize = 12.sp,
                    color = AmberResin,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                if (rom.lastPlayedMillis != null) {
                    Text(
                        text = "Last played: ${formatLastPlayed(rom.lastPlayedMillis)}",
                        fontSize = 11.sp,
                        color = PineGlowMist.copy(alpha = 0.6f),
                    )
                }
            }

            if (rom.isFavorite) {
                Icon(
                    Icons.Default.Favorite,
                    contentDescription = "Favorite",
                    tint = GoldenSaplight,
                    modifier = Modifier.size(20.dp)
                )
            }

            Icon(
                Icons.Default.PlayArrow,
                contentDescription = "Play",
                tint = AmberResin,
                modifier = Modifier.size(24.dp)
            )
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
