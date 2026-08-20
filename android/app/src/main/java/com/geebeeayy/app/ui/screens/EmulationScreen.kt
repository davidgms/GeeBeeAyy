package com.geebeeayyayy.app.ui.screens

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.geebeeayyayy.app.ui.theme.*
import kotlin.math.floor

@Composable
fun EmulationScreen(
    frameBuffer: ByteArray?,
    onBack: () -> Unit,
    onPause: () -> Unit,
    onFastForward: () -> Unit,
    onSaveState: (Int) -> Unit,
    onLoadState: (Int) -> Unit,
) {
    var isPaused by remember { mutableStateOf(false) }
    var showMenu by remember { mutableStateOf(false) }
    var isFastForward by remember { mutableStateOf(false) }

    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(BurntRoot)
    ) {
        Column(
            modifier = Modifier.fillMaxSize(),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            // Top bar
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 8.dp, vertical = 4.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                IconButton(onClick = onBack) {
                    Icon(Icons.Default.ArrowBack, "Back", tint = PineGlowMist)
                }
                Spacer(modifier = Modifier.weight(1f))
                Text(
                    text = "GeeBeeAyy!",
                    color = GoldenSaplight,
                    fontSize = 16.sp,
                )
                Spacer(modifier = Modifier.weight(1f))
                IconButton(onClick = { showMenu = !showMenu }) {
                    Icon(Icons.Default.MoreVert, "Menu", tint = PineGlowMist)
                }
            }

            // GBA Screen
            Box(
                modifier = Modifier
                    .fillMaxWidth()
                    .aspectRatio(240f / 160f)
                    .padding(horizontal = 24.dp)
                    .clip(RoundedCornerShape(8.dp))
                    .background(Color.Black),
                contentAlignment = Alignment.Center,
            ) {
                // Frame buffer rendering
                if (frameBuffer != null) {
                    GbaScreen(frameBuffer = frameBuffer)
                } else {
                    Text(
                        text = "Loading...",
                        color = AmberResin,
                        fontSize = 14.sp,
                    )
                }

                // Pause overlay
                if (isPaused) {
                    Box(
                        modifier = Modifier
                            .fillMaxSize()
                            .background(Color.Black.copy(alpha = 0.7f)),
                        contentAlignment = Alignment.Center,
                    ) {
                        Column(horizontalAlignment = Alignment.CenterHorizontally) {
                            Icon(
                                Icons.Default.Pause,
                                contentDescription = null,
                                tint = GoldenSaplight,
                                modifier = Modifier.size(48.dp)
                            )
                            Spacer(modifier = Modifier.height(8.dp))
                            Text("PAUSED", color = GoldenSaplight, fontSize = 20.sp)
                        }
                    }
                }
            }

            Spacer(modifier = Modifier.weight(1f))

            // Controls
            GameControls(
                isPaused = isPaused,
                isFastForward = isFastForward,
                onTogglePause = {
                    isPaused = !isPaused
                    onPause()
                },
                onToggleFastForward = {
                    isFastForward = !isFastForward
                    onFastForward()
                },
            )
        }

        // Dropdown menu
        if (showMenu) {
            DropdownMenu(
                expanded = showMenu,
                onDismissRequest = { showMenu = false },
            ) {
                DropdownMenuItem(
                    text = { Text("Save State") },
                    onClick = { showMenu = false; onSaveState(0) },
                    leadingIcon = { Icon(Icons.Default.Save, null, tint = AmberResin) }
                )
                DropdownMenuItem(
                    text = { Text("Load State") },
                    onClick = { showMenu = false; onLoadState(0) },
                    leadingIcon = { Icon(Icons.Default.FolderOpen, null, tint = AmberResin) }
                )
                HorizontalDivider(color = HoneyMid)
                DropdownMenuItem(
                    text = { Text("Settings") },
                    onClick = { showMenu = false },
                    leadingIcon = { Icon(Icons.Default.Settings, null, tint = AmberResin) }
                )
            }
        }
    }
}

@Composable
fun GbaScreen(frameBuffer: ByteArray) {
    // Render the 240x160 RGB frame buffer to screen
    val bitmap = remember(frameBuffer) {
        try {
            val width = 240
            val height = 160
            val pixels = IntArray(width * height)
            for (i in 0 until width * height) {
                val r = frameBuffer[i * 3].toInt() and 0xFF
                val g = frameBuffer[i * 3 + 1].toInt() and 0xFF
                val b = frameBuffer[i * 3 + 2].toInt() and 0xFF
                pixels[i] = (0xFF shl 24) or (r shl 16) or (g shl 8) or b
            }
            android.graphics.Bitmap.createBitmap(pixels, width, height, android.graphics.Bitmap.Config.ARGB_8888)
        } catch (e: Exception) {
            null
        }
    }

    if (bitmap != null) {
        Canvas(modifier = Modifier.fillMaxSize()) {
            drawImage(
                image = bitmap.asImageBitmap(),
                dstSize = size,
            )
        }
    }
}

@Composable
fun GameControls(
    isPaused: Boolean,
    isFastForward: Boolean,
    onTogglePause: () -> Unit,
    onToggleFastForward: () -> Unit,
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(16.dp),
        horizontalArrangement = Arrangement.SpaceEvenly,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        // D-Pad (left side)
        DPad()

        // Action buttons (right side)
        ActionButtons()

        // Control buttons
        Column(
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            // Pause/Play
            Button(
                onClick = onTogglePause,
                colors = ButtonDefaults.buttonColors(
                    containerColor = if (isPaused) GoldenSaplight else AmberResin,
                ),
                modifier = Modifier.size(48.dp),
                shape = CircleShape,
                contentPadding = PaddingValues(0.dp),
            ) {
                Icon(
                    if (isPaused) Icons.Default.PlayArrow else Icons.Default.Pause,
                    contentDescription = if (isPaused) "Resume" else "Pause",
                    tint = BurntRoot,
                )
            }

            // Fast Forward
            Button(
                onClick = onToggleFastForward,
                colors = ButtonDefaults.buttonColors(
                    containerColor = if (isFastForward) GoldenSaplight else HoneyMid,
                ),
                modifier = Modifier.size(48.dp),
                shape = CircleShape,
                contentPadding = PaddingValues(0.dp),
            ) {
                Icon(
                    Icons.Default.FastForward,
                    contentDescription = "Fast Forward",
                    tint = BurntRoot,
                )
            }
        }
    }
}

@Composable
fun DPad() {
    val buttonColor = HoneyDark
    val pressColor = AmberResin

    Column(
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        // Up
        DPadButton(Icons.Default.KeyboardArrowUp, "Up", buttonColor, pressColor)
        // Left, Center, Right
        Row {
            DPadButton(Icons.Default.KeyboardArrowLeft, "Left", buttonColor, pressColor)
            Box(modifier = Modifier.size(48.dp))
            DPadButton(Icons.Default.KeyboardArrowRight, "Right", buttonColor, pressColor)
        }
        // Down
        DPadButton(Icons.Default.KeyboardArrowDown, "Down", buttonColor, pressColor)
    }
}

@Composable
fun DPadButton(
    icon: androidx.compose.ui.graphics.vector.ImageVector,
    label: String,
    backgroundColor: Color,
    pressColor: Color,
) {
    var isPressed by remember { mutableStateOf(false) }

    Button(
        onClick = { /* Handle press */ },
        modifier = Modifier
            .size(48.dp)
            .pointerInput(Unit) {
                awaitPointerEventScope {
                    while (true) {
                        val event = awaitPointerEvent()
                        isPressed = event.changes.any { it.pressed }
                    }
                }
            },
        colors = ButtonDefaults.buttonColors(
            containerColor = if (isPressed) pressColor else backgroundColor,
        ),
        shape = CircleShape,
        contentPadding = PaddingValues(0.dp),
    ) {
        Icon(icon, contentDescription = label, tint = PineGlowMist, modifier = Modifier.size(24.dp))
    }
}

@Composable
fun ActionButtons() {
    val buttonColor = HoneyDark
    val pressColor = GoldenSaplight

    Box(modifier = Modifier.size(120.dp)) {
        // B button (left)
        ActionButton("B", buttonColor, pressColor, Modifier.align(Alignment.CenterStart))
        // A button (right)
        ActionButton("A", buttonColor, pressColor, Modifier.align(Alignment.CenterEnd))
    }
}

@Composable
fun ActionButton(
    label: String,
    backgroundColor: Color,
    pressColor: Color,
    modifier: Modifier = Modifier,
) {
    var isPressed by remember { mutableStateOf(false) }

    Button(
        onClick = { /* Handle press */ },
        modifier = modifier
            .size(56.dp)
            .pointerInput(Unit) {
                awaitPointerEventScope {
                    while (true) {
                        val event = awaitPointerEvent()
                        isPressed = event.changes.any { it.pressed }
                    }
                }
            },
        colors = ButtonDefaults.buttonColors(
            containerColor = if (isPressed) pressColor else backgroundColor,
        ),
        shape = CircleShape,
        contentPadding = PaddingValues(0.dp),
    ) {
        Text(
            text = label,
            color = PineGlowMist,
            fontSize = 20.sp,
            fontWeight = FontWeight.Bold,
        )
    }
}
