package com.geebeeayy.app.ui.screens

import android.content.res.Configuration
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
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.FilterQuality
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.geebeeayy.app.data.ScaleMode
import com.geebeeayy.app.engine.GbaEngine
import com.geebeeayy.app.ui.theme.*
import kotlin.math.floor
import kotlin.math.roundToInt

@Composable
fun EmulationScreen(
    frameBuffer: ByteArray?,
    isLoading: Boolean = false,
    errorMessage: String? = null,
    stateMessage: String? = null,
    scaleMode: ScaleMode = ScaleMode.INTEGER,
    onDismissStateMessage: () -> Unit = {},
    onBack: () -> Unit,
    onPause: () -> Unit,
    onFastForward: () -> Unit,
    onSaveState: (Int) -> Unit,
    onLoadState: (Int) -> Unit,
    onKeyChange: (Int, Boolean) -> Unit = { _, _ -> },
) {
    var isPaused by remember { mutableStateOf(false) }
    var showMenu by remember { mutableStateOf(false) }
    var isFastForward by remember { mutableStateOf(false) }

    val isLandscape =
        LocalConfiguration.current.orientation == Configuration.ORIENTATION_LANDSCAPE

    // A game is watched, not touched: without this the display times out
    // mid-play and the emulator keeps running behind a black screen.
    val view = LocalView.current
    DisposableEffect(view) {
        view.keepScreenOn = true
        onDispose { view.keepScreenOn = false }
    }

    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(BurntRoot)
    ) {
        Column(modifier = Modifier.fillMaxSize()) {
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

            if (stateMessage != null) {
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .background(HoneyDark)
                        .padding(horizontal = 12.dp, vertical = 6.dp),
                    horizontalArrangement = Arrangement.SpaceBetween,
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Text(text = stateMessage, color = PineGlowMist, fontSize = 13.sp)
                    IconButton(onClick = onDismissStateMessage, modifier = Modifier.size(24.dp)) {
                        Icon(Icons.Default.Close, "Dismiss", tint = PineGlowMist)
                    }
                }
            }

            if (isLandscape) {
                // Controls flank the screen rather than sitting under it, so
                // nothing overlaps the play area on a wide/short display.
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .weight(1f),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    DPad(
                        onKeyChange = onKeyChange,
                        modifier = Modifier.padding(start = 16.dp),
                    )
                    ScreenContainer(
                        frameBuffer = frameBuffer,
                        isLoading = isLoading,
                        errorMessage = errorMessage,
                        isPaused = isPaused,
                        scaleMode = scaleMode,
                        modifier = Modifier
                            .weight(1f)
                            .fillMaxHeight()
                            .padding(horizontal = 12.dp, vertical = 8.dp),
                    )
                    Column(
                        horizontalAlignment = Alignment.CenterHorizontally,
                        verticalArrangement = Arrangement.spacedBy(12.dp),
                        modifier = Modifier.padding(end = 16.dp),
                    ) {
                        ActionButtons(onKeyChange = onKeyChange)
                        TransportControls(
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
                }
            } else {
                ScreenContainer(
                    frameBuffer = frameBuffer,
                    isLoading = isLoading,
                    errorMessage = errorMessage,
                    isPaused = isPaused,
                    scaleMode = scaleMode,
                    modifier = Modifier
                        .fillMaxWidth()
                        .weight(1f)
                        // 24.dp a side left only 948 px of a 1080 px screen,
                        // and integer scaling rounds that down to 3x. 8.dp
                        // clears 960 px, which is exactly 4x.
                        .padding(horizontal = 8.dp, vertical = 8.dp),
                )

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
                    onKeyChange = onKeyChange,
                )
            }
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

/**
 * The black play-area box: frame buffer (or a status message) plus the pause
 * overlay. Takes whatever size its [modifier] gives it - callers decide
 * whether that is "rest of a portrait column" or "middle of a landscape
 * row" - and [GbaScreen] letterboxes within it per [scaleMode].
 */
@Composable
private fun ScreenContainer(
    frameBuffer: ByteArray?,
    isLoading: Boolean,
    errorMessage: String?,
    isPaused: Boolean,
    scaleMode: ScaleMode,
    modifier: Modifier = Modifier,
) {
    Box(
        modifier = modifier
            .clip(RoundedCornerShape(8.dp))
            .background(Color.Black),
        contentAlignment = Alignment.Center,
    ) {
        if (frameBuffer != null) {
            GbaScreen(frameBuffer = frameBuffer, scaleMode = scaleMode)
        } else if (errorMessage != null) {
            Text(text = errorMessage, color = Color.Red, fontSize = 14.sp)
        } else if (isLoading) {
            Text(text = "Loading ROM...", color = AmberResin, fontSize = 14.sp)
        } else {
            Text(text = "No ROM loaded", color = PineGlowMist, fontSize = 14.sp)
        }

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
                        contentDescription = null, // labelled by the "PAUSED" text below
                        tint = GoldenSaplight,
                        modifier = Modifier.size(48.dp)
                    )
                    Spacer(modifier = Modifier.height(8.dp))
                    Text("PAUSED", color = GoldenSaplight, fontSize = 20.sp)
                }
            }
        }
    }
}

/**
 * Largest width/height preserving the source aspect ratio that still fits
 * within [availW] x [availH].
 */
private fun fitSize(availW: Float, availH: Float, srcW: Int, srcH: Int): Pair<Float, Float> {
    val scale = minOf(availW / srcW, availH / srcH)
    return (srcW * scale) to (srcH * scale)
}

/**
 * Largest whole-number multiple of the source size that fits within
 * [availW] x [availH]. Falls back to [fitSize] when even a 1x scale would
 * overflow - i.e. the window is smaller than the native resolution.
 */
private fun integerSize(availW: Float, availH: Float, srcW: Int, srcH: Int): Pair<Float, Float> {
    val maxScale = minOf(availW / srcW, availH / srcH)
    val intScale = floor(maxScale).toInt()
    if (intScale < 1) return fitSize(availW, availH, srcW, srcH)
    return (srcW * intScale).toFloat() to (srcH * intScale).toFloat()
}

@Composable
fun GbaScreen(frameBuffer: ByteArray, scaleMode: ScaleMode = ScaleMode.INTEGER) {
    val width = GbaEngine.SCREEN_WIDTH
    val height = GbaEngine.SCREEN_HEIGHT
    // Reused across frames: the bitmap and its pixel staging buffer are each
    // allocated once and mutated in place, not recreated 60 times a second.
    val bitmap = remember {
        android.graphics.Bitmap.createBitmap(width, height, android.graphics.Bitmap.Config.ARGB_8888)
    }
    val pixels = remember { IntArray(width * height) }
    // `asImageBitmap()` wraps the bitmap in a new object each call, so hoist it
    // out of the per-frame draw rather than allocating a wrapper 60 times a
    // second. It stays valid because the bitmap itself is mutated in place.
    val image = remember(bitmap) { bitmap.asImageBitmap() }

    // A short buffer would throw out of the draw path and take the UI down; the
    // core has simply not produced a frame yet.
    if (frameBuffer.size < width * height * 3) {
        return
    }

    for (i in pixels.indices) {
        val o = i * 3
        val r = frameBuffer[o].toInt() and 0xFF
        val g = frameBuffer[o + 1].toInt() and 0xFF
        val b = frameBuffer[o + 2].toInt() and 0xFF
        pixels[i] = (0xFF shl 24) or (r shl 16) or (g shl 8) or b
    }
    bitmap.setPixels(pixels, 0, width, 0, 0, width, height)

    Canvas(modifier = Modifier.fillMaxSize()) {
        val (dstWidth, dstHeight) = when (scaleMode) {
            ScaleMode.STRETCH -> size.width to size.height
            ScaleMode.FIT -> fitSize(size.width, size.height, width, height)
            ScaleMode.INTEGER -> integerSize(size.width, size.height, width, height)
        }
        val dstOffsetX = (size.width - dstWidth) / 2f
        val dstOffsetY = (size.height - dstHeight) / 2f
        drawImage(
            image = image,
            dstOffset = IntOffset(dstOffsetX.roundToInt(), dstOffsetY.roundToInt()),
            dstSize = IntSize(dstWidth.roundToInt().coerceAtLeast(0), dstHeight.roundToInt().coerceAtLeast(0)),
            // A 240x160 source stretched onto a phone screen is many times its
            // native size; the default filter is bilinear and turns crisp
            // pixel art into mush. FilterQuality.None disables that sampling
            // so the game keeps its hard pixel edges.
            filterQuality = FilterQuality.None,
        )
    }
}

@Composable
fun GameControls(
    isPaused: Boolean,
    isFastForward: Boolean,
    onTogglePause: () -> Unit,
    onToggleFastForward: () -> Unit,
    onKeyChange: (Int, Boolean) -> Unit,
) {
    Column(modifier = Modifier.fillMaxWidth()) {
        // Shoulder buttons sit above the rest, where the real hardware puts
        // them: L on the far left, R on the far right.
        ShoulderRow(onKeyChange = onKeyChange)

        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 16.dp),
            horizontalArrangement = Arrangement.SpaceEvenly,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            DPad(onKeyChange = onKeyChange)
            ActionButtons(onKeyChange = onKeyChange)
            TransportControls(
                isPaused = isPaused,
                isFastForward = isFastForward,
                onTogglePause = onTogglePause,
                onToggleFastForward = onToggleFastForward,
            )
        }

        // Start and Select. Without these most games cannot get past a title
        // screen, so they are not optional extras.
        StartSelectRow(onKeyChange = onKeyChange)
    }
}

/** L and R, pushed to the outer edges. */
@Composable
fun ShoulderRow(onKeyChange: (Int, Boolean) -> Unit) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 16.dp, vertical = 4.dp),
        horizontalArrangement = Arrangement.SpaceBetween,
    ) {
        PillButton("L", GbaEngine.KEY_L, onKeyChange)
        PillButton("R", GbaEngine.KEY_R, onKeyChange)
    }
}

/** Start and Select, centred under the main controls. */
@Composable
fun StartSelectRow(onKeyChange: (Int, Boolean) -> Unit) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 16.dp, vertical = 8.dp),
        horizontalArrangement = Arrangement.Center,
    ) {
        PillButton("SELECT", GbaEngine.KEY_SELECT, onKeyChange)
        Spacer(modifier = Modifier.width(24.dp))
        PillButton("START", GbaEngine.KEY_START, onKeyChange)
    }
}

/**
 * A wide, short button for the controls that are pressed deliberately rather
 * than held during play. Height stays at 48dp so the touch target clears the
 * Android minimum even though the shape is not circular.
 */
@Composable
fun PillButton(label: String, key: Int, onKeyChange: (Int, Boolean) -> Unit) {
    var isPressed by remember { mutableStateOf(false) }

    Button(
        onClick = { /* Handled via pointerInput; a tap needs press+release reported. */ },
        modifier = Modifier
            .height(48.dp)
            .widthIn(min = 72.dp)
            .pointerInput(key) {
                awaitPointerEventScope {
                    while (true) {
                        val event = awaitPointerEvent()
                        val down = event.changes.any { it.pressed }
                        if (down != isPressed) {
                            isPressed = down
                            onKeyChange(key, down)
                        }
                    }
                }
            },
        colors = ButtonDefaults.buttonColors(
            containerColor = if (isPressed) AmberResin else HoneyDark,
        ),
        shape = RoundedCornerShape(24.dp),
        contentPadding = PaddingValues(horizontal = 16.dp),
    ) {
        Text(label, color = PineGlowMist, fontSize = 13.sp, fontWeight = FontWeight.Bold)
    }
}

/** Pause/resume and fast-forward toggle buttons, shared by the portrait and landscape layouts. */
@Composable
fun TransportControls(
    isPaused: Boolean,
    isFastForward: Boolean,
    onTogglePause: () -> Unit,
    onToggleFastForward: () -> Unit,
) {
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
                // The container changes with state, so the tint has to as
                // well: BurntRoot on HoneyMid is 2.34:1, under the 3:1 that
                // WCAG 2.1 SC 1.4.11 requires of a graphical control.
                // PineGlowMist on HoneyMid is 7.52:1 and BurntRoot on
                // GoldenSaplight is 12.33:1.
                tint = if (isFastForward) BurntRoot else PineGlowMist,
            )
        }
    }
}

@Composable
fun DPad(onKeyChange: (Int, Boolean) -> Unit, modifier: Modifier = Modifier) {
    val buttonColor = HoneyDark
    val pressColor = AmberResin

    Column(
        modifier = modifier,
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        // Up
        DPadButton(Icons.Default.KeyboardArrowUp, "Up", GbaEngine.KEY_UP, buttonColor, pressColor, onKeyChange)
        // Left, Center, Right
        Row {
            DPadButton(Icons.Default.KeyboardArrowLeft, "Left", GbaEngine.KEY_LEFT, buttonColor, pressColor, onKeyChange)
            Box(modifier = Modifier.size(48.dp))
            DPadButton(Icons.Default.KeyboardArrowRight, "Right", GbaEngine.KEY_RIGHT, buttonColor, pressColor, onKeyChange)
        }
        // Down
        DPadButton(Icons.Default.KeyboardArrowDown, "Down", GbaEngine.KEY_DOWN, buttonColor, pressColor, onKeyChange)
    }
}

// Each direction is its own touch target, so two fingers on adjacent
// buttons (e.g. Up + Right) OR their bits together into a diagonal. A
// single finger cannot express a diagonal - there is no shared corner zone.
@Composable
fun DPadButton(
    icon: androidx.compose.ui.graphics.vector.ImageVector,
    label: String,
    key: Int,
    backgroundColor: Color,
    pressColor: Color,
    onKeyChange: (Int, Boolean) -> Unit,
) {
    var isPressed by remember { mutableStateOf(false) }

    Button(
        onClick = { /* Handled via pointerInput below; a tap needs press+release reported. */ },
        modifier = Modifier
            .size(48.dp)
            .pointerInput(key) {
                awaitPointerEventScope {
                    while (true) {
                        val event = awaitPointerEvent()
                        val pressed = event.changes.any { it.pressed }
                        if (pressed != isPressed) {
                            isPressed = pressed
                            onKeyChange(key, pressed)
                        }
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
fun ActionButtons(onKeyChange: (Int, Boolean) -> Unit) {
    val buttonColor = HoneyDark
    val pressColor = GoldenSaplight

    Box(modifier = Modifier.size(120.dp)) {
        // B button (left)
        ActionButton("B", GbaEngine.KEY_B, buttonColor, pressColor, Modifier.align(Alignment.CenterStart), onKeyChange)
        // A button (right)
        ActionButton("A", GbaEngine.KEY_A, buttonColor, pressColor, Modifier.align(Alignment.CenterEnd), onKeyChange)
    }
}

@Composable
fun ActionButton(
    label: String,
    key: Int,
    backgroundColor: Color,
    pressColor: Color,
    modifier: Modifier = Modifier,
    onKeyChange: (Int, Boolean) -> Unit,
) {
    var isPressed by remember { mutableStateOf(false) }

    Button(
        onClick = { /* Handled via pointerInput below; a tap needs press+release reported. */ },
        modifier = modifier
            .size(56.dp)
            .pointerInput(key) {
                awaitPointerEventScope {
                    while (true) {
                        val event = awaitPointerEvent()
                        val pressed = event.changes.any { it.pressed }
                        if (pressed != isPressed) {
                            isPressed = pressed
                            onKeyChange(key, pressed)
                        }
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
