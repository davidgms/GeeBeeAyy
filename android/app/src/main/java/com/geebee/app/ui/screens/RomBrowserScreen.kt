package com.geebee.app.ui.screens

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
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
import com.geebee.app.ui.theme.*

data class RomEntry(
    val name: String,
    val fileName: String,
    val size: String,
    val lastPlayed: String? = null,
    val isFavorite: Boolean = false,
)

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun RomBrowserScreen(
    roms: List<RomEntry>,
    onRomClick: (RomEntry) -> Unit,
    onSettingsClick: () -> Unit,
    onAboutClick: () -> Unit,
) {
    Scaffold(
        topBar = {
            TopAppBar(
                title = {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Text(
                            text = "GeeBee-A!",
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
        floatingActionButton = {
            ExtendedFloatingActionButton(
                onClick = { /* Open file picker */ },
                containerColor = AmberResin,
                contentColor = BurntRoot,
                icon = { Icon(Icons.Default.Add, contentDescription = null) },
                text = { Text("Add ROM") },
            )
        },
        containerColor = BurntRoot
    ) { padding ->
        if (roms.isEmpty()) {
            EmptyState(modifier = Modifier.padding(padding))
        } else {
            LazyColumn(
                modifier = Modifier
                    .fillMaxSize()
                    .padding(padding),
                contentPadding = PaddingValues(16.dp),
                verticalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                item {
                    Text(
                        text = "Your Games",
                        fontSize = 24.sp,
                        fontWeight = FontWeight.Bold,
                        color = GoldenSaplight,
                        modifier = Modifier.padding(bottom = 8.dp)
                    )
                }

                // Favorites section
                val favorites = roms.filter { it.isFavorite }
                if (favorites.isNotEmpty()) {
                    item {
                        Text(
                            text = "Favorites",
                            fontSize = 16.sp,
                            fontWeight = FontWeight.SemiBold,
                            color = AmberResin,
                            modifier = Modifier.padding(vertical = 4.dp)
                        )
                    }
                    items(favorites) { rom ->
                        RomCard(rom = rom, onClick = { onRomClick(rom) })
                    }
                }

                // All ROMs
                item {
                    Text(
                        text = "All ROMs",
                        fontSize = 16.sp,
                        fontWeight = FontWeight.SemiBold,
                        color = AmberResin,
                        modifier = Modifier.padding(top = if (favorites.isNotEmpty()) 16.dp else 0.dp, bottom = 4.dp)
                    )
                }
                items(roms) { rom ->
                    RomCard(rom = rom, onClick = { onRomClick(rom) })
                }
            }
        }
    }
}

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
            // Game icon placeholder
            Box(
                modifier = Modifier
                    .size(56.dp)
                    .clip(RoundedCornerShape(8.dp))
                    .background(AmberResin.copy(alpha = 0.3f)),
                contentAlignment = Alignment.Center,
            ) {
                Icon(
                    Icons.Default.VideogameAsset,
                    contentDescription = null,
                    tint = GoldenSaplight,
                    modifier = Modifier.size(32.dp)
                )
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
                    text = rom.fileName,
                    fontSize = 12.sp,
                    color = AmberResin,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                if (rom.lastPlayed != null) {
                    Text(
                        text = "Last played: ${rom.lastPlayed}",
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
            text = "Tap 'Add ROM' to get started.\nBzzt! Your games await!",
            fontSize = 14.sp,
            color = PineGlowMist.copy(alpha = 0.7f),
            lineHeight = 20.sp,
        )
    }
}
