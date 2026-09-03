package com.geebeeayy.app.ui.screens

import android.content.res.Configuration
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.Redo
import androidx.compose.material.icons.automirrored.filled.Undo
import androidx.compose.material.icons.filled.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.FilterQuality
import androidx.compose.ui.graphics.TransformOrigin
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInVertically
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.ui.input.pointer.pointerInput
import kotlinx.coroutines.delay
import androidx.compose.foundation.border
import androidx.compose.foundation.gestures.detectDragGestures
import androidx.compose.foundation.layout.offset
import com.geebeeayy.app.data.ControlButton
import com.geebeeayy.app.data.ControlLayout
import com.geebeeayy.app.data.ControlLayoutStore
import com.geebeeayy.app.data.CustomButton
import com.geebeeayy.app.data.CustomButtonMode
import kotlinx.coroutines.launch
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.geebeeayy.app.data.ScreenFilter
import com.geebeeayy.app.data.StateSlot
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
    screenFilter: ScreenFilter = ScreenFilter.NONE,
    onDismissStateMessage: () -> Unit = {},
    onBack: () -> Unit,
    onPause: () -> Unit,
    onFastForward: () -> Unit,
    fastForwardSpeed: Int = 0,
    isRewinding: Boolean = false,
    onRewind: (Boolean) -> Unit = {},
    controlScale: Float = 1f,
    controlOpacity: Float = 1f,
    onSaveState: (Int) -> Unit,
    onLoadState: (Int) -> Unit,
    stateSlots: () -> List<StateSlot> = { emptyList() },
    onScreenshot: () -> Unit = {},
    onKeyChange: (Int, Boolean) -> Unit = { _, _ -> },
    gameKey: () -> String? = { null },
) {
    var isPaused by remember { mutableStateOf(false) }
    var showMenu by remember { mutableStateOf(false) }

    // Layout editing. The offsets are held here while dragging and written
    // back only on Done, so an abandoned edit leaves the saved layout alone.
    val context = LocalContext.current
    val layoutStore = remember { ControlLayoutStore(context) }
    var layouts by remember { mutableStateOf(layoutStore.getLayouts()) }
    var activeLayoutId by remember { mutableStateOf(ControlLayoutStore.DEFAULT_LAYOUT_ID) }
    var editingLayout by remember { mutableStateOf(false) }
    var layoutsModalOpen by remember { mutableStateOf(false) }
    // The button currently being dragged, so only it gets the outline - a
    // player touches a button, sees it highlight, then drags it, rather than
    // every button being outlined at once.
    var selectedButton by remember { mutableStateOf<ControlButton?>(null) }
    val offsets = remember { mutableStateMapOf<ControlButton, Offset>() }
    // Custom buttons (combos, sequences, hold toggles) belong to the layout
    // the same way the real buttons' positions do. Their drag is separate
    // from the real buttons' undo/redo/Done staging below - each drag saves
    // its new position immediately, since there is no equivalent "abandon
    // this edit" concern for a position with no default to revert to.
    var customButtons by remember { mutableStateOf<List<CustomButton>>(emptyList()) }
    val customOffsets = remember { mutableStateMapOf<String, Offset>() }
    val heldToggles = remember { mutableStateMapOf<String, Boolean>() }
    var selectedCustomId by remember { mutableStateOf<String?>(null) }
    var customButtonsModalOpen by remember { mutableStateOf(false) }
    fun loadLayout(layoutId: String) {
        // A toggle-held custom button's keys are still down in the core -
        // switching layouts out from under it must not leave them stuck.
        heldToggles.forEach { (id, isHeld) ->
            if (isHeld) {
                customButtons.firstOrNull { it.id == id }?.keys?.forEach { key -> onKeyChange(key, false) }
            }
        }
        offsets.clear()
        ControlButton.entries.forEach { button ->
            val (x, y) = layoutStore.getControlOffset(layoutId, button)
            offsets[button] = Offset(x, y)
        }
        customButtons = layoutStore.getCustomButtons(layoutId)
        customOffsets.clear()
        heldToggles.clear()
        customButtons.forEach { button ->
            val (x, y) = layoutStore.getCustomButtonOffset(layoutId, button.id)
            customOffsets[button.id] = Offset(x, y)
        }
    }
    // Leaving the screen with a TOGGLE_HOLD button on used to leave those
    // keys pressed in the ViewModel for the rest of the session: the release
    // only ran on a layout switch or a delete, never on dispose.
    DisposableEffect(Unit) {
        onDispose {
            heldToggles.forEach { (id, isHeld) ->
                if (isHeld) {
                    customButtons.firstOrNull { it.id == id }?.keys?.forEach { key ->
                        onKeyChange(key, false)
                    }
                }
            }
        }
    }

    // The ROM loads asynchronously - its key (and so which layout it uses)
    // is not known on first composition, only once loading finishes.
    LaunchedEffect(gameKey()) {
        val key = gameKey() ?: return@LaunchedEffect
        activeLayoutId = layoutStore.getLayoutForGame(key)
        loadLayout(activeLayoutId)
    }
    // Undo/redo history for the current edit session: a stack of full-layout
    // snapshots taken before each change (a drag gesture or Reset), not one
    // per pixel of movement - a single drag is one undo step, not hundreds.
    val undoStack = remember { mutableStateListOf<Map<ControlButton, Offset>>() }
    val redoStack = remember { mutableStateListOf<Map<ControlButton, Offset>>() }
    val pushUndoSnapshot: () -> Unit = {
        undoStack.add(offsets.toMap())
        redoStack.clear()
    }
    val undoEdit: () -> Unit = {
        undoStack.removeLastOrNull()?.let { previous ->
            redoStack.add(offsets.toMap())
            offsets.clear()
            offsets.putAll(previous)
        }
    }
    val redoEdit: () -> Unit = {
        redoStack.removeLastOrNull()?.let { next ->
            undoStack.add(offsets.toMap())
            offsets.clear()
            offsets.putAll(next)
        }
    }
    val handleDragStart: (ControlButton) -> Unit = { button ->
        pushUndoSnapshot()
        selectedButton = button
    }
    val handleDragEnd: () -> Unit = { selectedButton = null }
    // A custom button's position saves the moment the drag ends - there is
    // no Done to stage it behind, so `selectedCustomId` still names the one
    // that just finished when this fires.
    val handleCustomDragStart: (String) -> Unit = { id -> selectedCustomId = id }
    val handleCustomDragEnd: () -> Unit = {
        selectedCustomId?.let { id ->
            val pos = customOffsets[id] ?: Offset.Zero
            layoutStore.setCustomButtonOffset(activeLayoutId, id, pos.x, pos.y)
        }
        selectedCustomId = null
    }
    // Makes `layout` the current game's layout - a radio pick just switches
    // what is on screen; opening its editor also closes the modal and drops
    // straight into dragging, since there is no point editing a layout that
    // is not the one loaded.
    fun selectLayout(layout: ControlLayout, openEditor: Boolean) {
        activeLayoutId = layout.id
        gameKey()?.let { layoutStore.setLayoutForGame(it, layout.id) }
        loadLayout(layout.id)
        undoStack.clear()
        redoStack.clear()
        selectedButton = null
        if (openEditor) {
            layoutsModalOpen = false
            editingLayout = true
        }
    }
    var showSlots by remember { mutableStateOf(false) }

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
            // Plain black rather than the palette's BurntRoot: this is the
            // in-game screen, so the background should recede and let the
            // palette-coloured icons and buttons be the only colour on it.
            // Dialogs (save states, layouts) and overlay chrome (the state
            // toast, the layout-edit bar) keep BurntRoot/HoneyDark - they are
            // not this background.
            .background(Color.Black)
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

                // Save, load, rewind, fast forward and pause live up here
                // rather than among the game buttons: they are things you do
                // *to* the emulator, and putting them beside A and B is how
                // you fast-forward when you meant to jump.
                IconButton(onClick = { onSaveState(0) }) {
                    Icon(Icons.Default.Save, "Save state", tint = PineGlowMist)
                }
                IconButton(onClick = { onLoadState(0) }) {
                    Icon(Icons.Default.FolderOpen, "Load state", tint = PineGlowMist)
                }
                Box(
                    modifier = Modifier
                        .size(48.dp)
                        .pointerInput(Unit) {
                            detectTapGestures(
                                onPress = {
                                    onRewind(true)
                                    tryAwaitRelease()
                                    onRewind(false)
                                },
                            )
                        },
                    contentAlignment = Alignment.Center,
                ) {
                    Icon(
                        Icons.Default.FastRewind,
                        "Rewind",
                        tint = if (isRewinding) GoldenSaplight else PineGlowMist,
                    )
                }
                // Cycles off -> 2x -> 4x -> 8x -> off. The badge is the only
                // way to tell which: the icon itself does not change.
                Box(modifier = Modifier.size(48.dp), contentAlignment = Alignment.Center) {
                    IconButton(onClick = onFastForward) {
                        Icon(
                            Icons.Default.FastForward,
                            if (fastForwardSpeed > 0) "Fast forward ${fastForwardSpeed}x" else "Fast forward",
                            tint = if (fastForwardSpeed > 0) GoldenSaplight else PineGlowMist,
                        )
                    }
                    if (fastForwardSpeed > 0) {
                        Text(
                            text = "${fastForwardSpeed}x",
                            color = GoldenSaplight,
                            fontSize = 9.sp,
                            fontWeight = FontWeight.Bold,
                            modifier = Modifier
                                .align(Alignment.BottomEnd)
                                .offset(x = (-2).dp, y = (-2).dp),
                        )
                    }
                }
                IconButton(onClick = {
                    isPaused = !isPaused
                    onPause()
                }) {
                    Icon(
                        if (isPaused) Icons.Default.PlayArrow else Icons.Default.Pause,
                        if (isPaused) "Resume" else "Pause",
                        tint = if (isPaused) GoldenSaplight else PineGlowMist,
                    )
                }
                IconButton(onClick = { showMenu = !showMenu }) {
                    Icon(Icons.Default.MoreVert, "Menu", tint = PineGlowMist)
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
                        editingLayout = editingLayout,
                        selectedButton = selectedButton,
                        offsets = offsets,
                        onDragStart = handleDragStart,
                        onDragEnd = handleDragEnd,
                    )
                    ScreenContainer(
                        frameBuffer = frameBuffer,
                        isLoading = isLoading,
                        errorMessage = errorMessage,
                        isPaused = isPaused,
                        scaleMode = scaleMode,
                        screenFilter = screenFilter,
                        modifier = Modifier
                            .weight(1f)
                            .fillMaxHeight()
                            .padding(horizontal = 12.dp, vertical = 8.dp),
                    )
                    // Landscape has the transport in the top bar too, so the
                    // right-hand column is just the face buttons.
                    Column(
                        horizontalAlignment = Alignment.CenterHorizontally,
                        verticalArrangement = Arrangement.spacedBy(12.dp),
                        modifier = Modifier.padding(end = 16.dp),
                    ) {
                        ActionButtons(
                            onKeyChange = onKeyChange,
                            editingLayout = editingLayout,
                            selectedButton = selectedButton,
                            offsets = offsets,
                            onDragStart = handleDragStart,
                            onDragEnd = handleDragEnd,
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
                    screenFilter = screenFilter,
                    modifier = Modifier
                        .fillMaxWidth()
                        .weight(1f)
                        // 24.dp a side left only 948 px of a 1080 px screen,
                        // and integer scaling rounds that down to 3x. 8.dp
                        // clears 960 px, which is exactly 4x.
                        .padding(horizontal = 8.dp, vertical = 8.dp),
                )

                // One transform on the whole block rather than a size
                // multiplier threaded through every button: Compose maps
                // pointer input through the layer, so the touch targets grow
                // with the drawing and stay in register.
                Box(
                    modifier = Modifier.graphicsLayer(
                        scaleX = controlScale,
                        scaleY = controlScale,
                        alpha = controlOpacity,
                        transformOrigin = TransformOrigin(0.5f, 1f),
                    )
                ) {
                GameControls(
                    isPaused = isPaused,
                    isFastForward = fastForwardSpeed > 0,
                    isRewinding = isRewinding,
                    onRewind = onRewind,
                    onTogglePause = {
                        isPaused = !isPaused
                        onPause()
                    },
                    onToggleFastForward = onFastForward,
                    onKeyChange = onKeyChange,
                    editingLayout = editingLayout,
                    offsets = offsets,
                    selectedButton = selectedButton,
                    onDragStart = handleDragStart,
                    onDragEnd = handleDragEnd,
                )
                }
            }
        }

        // Custom buttons float over everything, positioned absolutely rather
        // than nudged from a natural spot like the real buttons - a
        // player-made button has no natural position to nudge from. Visible
        // and live during play, not just while editing; only the drag
        // affordance is gated on that.
        customButtons.forEach { button ->
            Box(
                modifier = Modifier.movableControl(
                    button.id,
                    editingLayout,
                    selectedCustomId,
                    pillShape,
                    customOffsets,
                    handleCustomDragStart,
                    handleCustomDragEnd,
                )
            ) {
                CustomButtonView(
                    button = button,
                    held = heldToggles[button.id] == true,
                    onToggleHeld = {
                        val nowHeld = heldToggles[button.id] != true
                        heldToggles[button.id] = nowHeld
                        button.keys.forEach { key -> onKeyChange(key, nowHeld) }
                    },
                    onKeyChange = onKeyChange,
                )
            }
        }

        if (editingLayout) {
            LayoutEditBar(
                onDone = {
                    ControlButton.entries.forEach { button ->
                        val o = offsets[button] ?: Offset.Zero
                        layoutStore.setControlOffset(activeLayoutId, button, o.x, o.y)
                    }
                    editingLayout = false
                    selectedButton = null
                    undoStack.clear()
                    redoStack.clear()
                },
                onReset = {
                    pushUndoSnapshot()
                    layoutStore.resetLayoutOffsets(activeLayoutId)
                    ControlButton.entries.forEach { offsets[it] = Offset.Zero }
                },
                canUndo = undoStack.isNotEmpty(),
                canRedo = redoStack.isNotEmpty(),
                onUndo = undoEdit,
                onRedo = redoEdit,
                onCustomButtons = { customButtonsModalOpen = true },
                modifier = Modifier.align(Alignment.TopCenter),
            )
        }

        if (customButtonsModalOpen) {
            CustomButtonsListDialog(
                buttons = customButtons,
                onSave = { button ->
                    layoutStore.saveCustomButton(activeLayoutId, button)
                    customButtons = layoutStore.getCustomButtons(activeLayoutId)
                    customOffsets.getOrPut(button.id) {
                        val (x, y) = layoutStore.getCustomButtonOffset(activeLayoutId, button.id)
                        Offset(x, y)
                    }
                },
                onDelete = { button ->
                    if (heldToggles[button.id] == true) {
                        button.keys.forEach { key -> onKeyChange(key, false) }
                    }
                    layoutStore.deleteCustomButton(activeLayoutId, button.id)
                    customButtons = layoutStore.getCustomButtons(activeLayoutId)
                    customOffsets.remove(button.id)
                    heldToggles.remove(button.id)
                },
                onDismiss = { customButtonsModalOpen = false },
            )
        }

        if (layoutsModalOpen) {
            LayoutsDialog(
                layouts = layouts,
                activeLayoutId = activeLayoutId,
                onSelect = { layout -> selectLayout(layout, openEditor = false) },
                onEdit = { layout -> selectLayout(layout, openEditor = true) },
                onRename = { id, name ->
                    layoutStore.renameLayout(id, name)
                    layouts = layoutStore.getLayouts()
                },
                onDelete = { layout ->
                    layoutStore.deleteLayout(layout.id)
                    layouts = layoutStore.getLayouts()
                    if (activeLayoutId == layout.id) {
                        selectLayout(layouts.first { it.isDefault }, openEditor = false)
                    }
                },
                onCreate = { name ->
                    val created = layoutStore.createLayout(name, copyFrom = activeLayoutId)
                    layouts = layoutStore.getLayouts()
                    selectLayout(created, openEditor = true)
                },
                onDismiss = { layoutsModalOpen = false },
            )
        }

        StateToast(
            message = stateMessage,
            onDismiss = onDismissStateMessage,
            modifier = Modifier.align(Alignment.TopCenter),
        )

        if (showSlots) {
            SaveStateDialog(
                slots = stateSlots(),
                onSave = { slot -> showSlots = false; onSaveState(slot) },
                onLoad = { slot -> showSlots = false; onLoadState(slot) },
                onDismiss = { showSlots = false },
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
                DropdownMenuItem(
                    text = { Text("Save States...") },
                    onClick = { showMenu = false; showSlots = true },
                    leadingIcon = { Icon(Icons.Default.Bookmarks, null, tint = AmberResin) }
                )
                HorizontalDivider(color = HoneyMid)
                DropdownMenuItem(
                    text = { Text("Customise Layout") },
                    onClick = {
                        showMenu = false
                        layouts = layoutStore.getLayouts()
                        layoutsModalOpen = true
                    },
                    leadingIcon = { Icon(Icons.Default.OpenWith, contentDescription = null) },
                )
                DropdownMenuItem(
                    text = { Text("Screenshot") },
                    onClick = { showMenu = false; onScreenshot() },
                    leadingIcon = { Icon(Icons.Default.PhotoCamera, contentDescription = null) },
                )
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
    screenFilter: ScreenFilter = ScreenFilter.NONE,
    modifier: Modifier = Modifier,
) {
    Box(
        modifier = modifier
            .clip(RoundedCornerShape(8.dp))
            .background(Color.Black),
        contentAlignment = Alignment.Center,
    ) {
        if (frameBuffer != null) {
            GbaScreen(frameBuffer = frameBuffer, scaleMode = scaleMode, filter = screenFilter)
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
 * The banner shown while the touch overlay is being rearranged.
 *
 * Sits over the game rather than replacing it, so a player can see what the
 * controls are covering while they move them.
 */
@Composable
private fun LayoutEditBar(
    onDone: () -> Unit,
    onReset: () -> Unit,
    canUndo: Boolean,
    canRedo: Boolean,
    onUndo: () -> Unit,
    onRedo: () -> Unit,
    onCustomButtons: () -> Unit,
    modifier: Modifier = Modifier,
) {
    Column(
        modifier = modifier
            .fillMaxWidth()
            .background(BurntRoot.copy(alpha = 0.92f))
            .padding(horizontal = 16.dp, vertical = 10.dp),
    ) {
        Row(
            modifier = Modifier.fillMaxWidth(),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.SpaceBetween,
        ) {
            Text(
                text = "Touch a button, then drag it",
                color = PineGlowMist,
                fontSize = 14.sp,
            )
            Row {
                IconButton(onClick = onCustomButtons) {
                    Icon(
                        Icons.Default.AddCircle,
                        contentDescription = "Custom buttons",
                        tint = AmberResin,
                    )
                }
                IconButton(onClick = onUndo, enabled = canUndo) {
                    Icon(
                        Icons.AutoMirrored.Filled.Undo,
                        contentDescription = "Undo",
                        tint = if (canUndo) AmberResin else PineGlowMist.copy(alpha = 0.3f),
                    )
                }
                IconButton(onClick = onRedo, enabled = canRedo) {
                    Icon(
                        Icons.AutoMirrored.Filled.Redo,
                        contentDescription = "Redo",
                        tint = if (canRedo) AmberResin else PineGlowMist.copy(alpha = 0.3f),
                    )
                }
                TextButton(onClick = onReset) {
                    Text("Reset", color = AmberResin)
                }
                TextButton(onClick = onDone) {
                    Text("Done", color = GoldenSaplight, fontWeight = FontWeight.Bold)
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
fun GbaScreen(
    frameBuffer: ByteArray,
    scaleMode: ScaleMode = ScaleMode.INTEGER,
    filter: ScreenFilter = ScreenFilter.NONE,
) {
    val doubled = filter == ScreenFilter.SAI_2X
    val width = if (doubled) GbaEngine.SCREEN_WIDTH * 2 else GbaEngine.SCREEN_WIDTH
    val height = if (doubled) GbaEngine.SCREEN_HEIGHT * 2 else GbaEngine.SCREEN_HEIGHT
    // Reused across frames: the bitmap and its pixel staging buffer are each
    // allocated once and mutated in place, not recreated 60 times a second.
    // Keyed on the filter because 2xSaI needs a bitmap four times the size.
    val bitmap = remember(doubled) {
        android.graphics.Bitmap.createBitmap(width, height, android.graphics.Bitmap.Config.ARGB_8888)
    }
    val pixels = remember { IntArray(GbaEngine.SCREEN_WIDTH * GbaEngine.SCREEN_HEIGHT) }
    val scaled = remember(doubled) {
        if (doubled) IntArray(width * height) else IntArray(0)
    }
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
    if (doubled) {
        Sai2x.scale(pixels, scaled, GbaEngine.SCREEN_WIDTH, GbaEngine.SCREEN_HEIGHT)
        bitmap.setPixels(scaled, 0, width, 0, 0, width, height)
    } else {
        bitmap.setPixels(pixels, 0, width, 0, 0, width, height)
    }

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
            // A 240x160 source stretched onto a phone screen is many times
            // its native size; bilinear turns crisp pixel art to mush, so
            // None is the default and Smooth is the opt-in.
            filterQuality = if (filter == ScreenFilter.SMOOTH) {
                FilterQuality.Low
            } else {
                FilterQuality.None
            },
        )

        if (filter == ScreenFilter.SCANLINES) {
            // Darken every other output row. Drawn as rectangles rather than
            // a per-pixel pass so the GPU does the work: the cost is one draw
            // list, not 38,400 multiplies a frame.
            val rowHeight = (dstHeight / GbaEngine.SCREEN_HEIGHT).coerceAtLeast(1f)
            if (rowHeight >= 2f) {
                var y = dstOffsetY + rowHeight / 2f
                while (y < dstOffsetY + dstHeight) {
                    drawRect(
                        color = Color.Black.copy(alpha = 0.35f),
                        topLeft = Offset(dstOffsetX, y),
                        size = Size(dstWidth, rowHeight / 2f),
                    )
                    y += rowHeight
                }
            }
        }
    }
}

/**
 * Applies a button's saved nudge, and in [editing] mode lets it be dragged to
 * a new one.
 *
 * The outline only shows on [selected] - the one the player currently has a
 * finger on - not on every button at once: touch a button, see it highlight,
 * then drag it.
 *
 * The drag is consumed here, so while editing the button underneath does not
 * also fire - you are moving it, not pressing it.
 */
@Composable
private fun <T> Modifier.movableControl(
    button: T,
    editing: Boolean,
    selected: T?,
    shape: Shape,
    offsets: MutableMap<T, Offset>,
    onDragStart: (T) -> Unit = {},
    onDragEnd: () -> Unit = {},
): Modifier {
    val offset = offsets[button] ?: Offset.Zero
    return this
        .offset { IntOffset(offset.x.roundToInt(), offset.y.roundToInt()) }
        .then(
            if (!editing) {
                Modifier
            } else {
                Modifier
                    .then(
                        if (selected == button) {
                            Modifier.border(2.dp, GoldenSaplight, shape)
                        } else {
                            Modifier
                        }
                    )
                    .pointerInput(button) {
                        detectDragGestures(
                            onDragStart = { onDragStart(button) },
                            onDragEnd = onDragEnd,
                            onDragCancel = onDragEnd,
                        ) { change, drag ->
                            change.consume()
                            val current = offsets[button] ?: Offset.Zero
                            offsets[button] = current + drag
                        }
                    }
            }
        )
}

@Composable
fun GameControls(
    isPaused: Boolean,
    isFastForward: Boolean,
    isRewinding: Boolean,
    onTogglePause: () -> Unit,
    onToggleFastForward: () -> Unit,
    onRewind: (Boolean) -> Unit,
    onKeyChange: (Int, Boolean) -> Unit,
    editingLayout: Boolean = false,
    offsets: MutableMap<ControlButton, Offset> = mutableMapOf(),
    selectedButton: ControlButton? = null,
    onDragStart: (ControlButton) -> Unit = {},
    onDragEnd: () -> Unit = {},
) {
    Column(modifier = Modifier.fillMaxWidth()) {
        // Shoulder buttons sit above the rest, where the real hardware puts
        // them: L on the far left, R on the far right.
        ShoulderRow(onKeyChange, editingLayout, selectedButton, offsets, onDragStart, onDragEnd)

        // D-pad hard left, face buttons hard right, nothing between them -
        // the thumbs rest at the edges of the phone, not in the middle.
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 24.dp, vertical = 8.dp),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            DPad(
                onKeyChange = onKeyChange,
                editingLayout = editingLayout,
                selectedButton = selectedButton,
                offsets = offsets,
                onDragStart = onDragStart,
                onDragEnd = onDragEnd,
            )
            ActionButtons(onKeyChange, editingLayout, selectedButton, offsets, onDragStart, onDragEnd)
        }

        // Start and Select. Without these most games cannot get past a title
        // screen, so they are not optional extras.
        StartSelectRow(onKeyChange, editingLayout, selectedButton, offsets, onDragStart, onDragEnd)
    }
}

private val pillShape = RoundedCornerShape(24.dp)

/** L and R, pushed to the outer edges. */
@Composable
fun ShoulderRow(
    onKeyChange: (Int, Boolean) -> Unit,
    editingLayout: Boolean = false,
    selectedButton: ControlButton? = null,
    offsets: MutableMap<ControlButton, Offset> = mutableMapOf(),
    onDragStart: (ControlButton) -> Unit = {},
    onDragEnd: () -> Unit = {},
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 16.dp, vertical = 4.dp),
        horizontalArrangement = Arrangement.SpaceBetween,
    ) {
        Box(
            modifier = Modifier.movableControl(
                ControlButton.SHOULDER_L, editingLayout, selectedButton, pillShape, offsets, onDragStart, onDragEnd
            )
        ) {
            PillButton("L", GbaEngine.KEY_L, onKeyChange)
        }
        Box(
            modifier = Modifier.movableControl(
                ControlButton.SHOULDER_R, editingLayout, selectedButton, pillShape, offsets, onDragStart, onDragEnd
            )
        ) {
            PillButton("R", GbaEngine.KEY_R, onKeyChange)
        }
    }
}

/** Start and Select, centred under the main controls. */
@Composable
fun StartSelectRow(
    onKeyChange: (Int, Boolean) -> Unit,
    editingLayout: Boolean = false,
    selectedButton: ControlButton? = null,
    offsets: MutableMap<ControlButton, Offset> = mutableMapOf(),
    onDragStart: (ControlButton) -> Unit = {},
    onDragEnd: () -> Unit = {},
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 16.dp, vertical = 8.dp),
        horizontalArrangement = Arrangement.Center,
    ) {
        Box(
            modifier = Modifier.movableControl(
                ControlButton.SELECT, editingLayout, selectedButton, pillShape, offsets, onDragStart, onDragEnd
            )
        ) {
            PillButton("SELECT", GbaEngine.KEY_SELECT, onKeyChange)
        }
        Spacer(modifier = Modifier.width(24.dp))
        Box(
            modifier = Modifier.movableControl(
                ControlButton.START, editingLayout, selectedButton, pillShape, offsets, onDragStart, onDragEnd
            )
        ) {
            PillButton("START", GbaEngine.KEY_START, onKeyChange)
        }
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

/**
 * A player-defined [CustomButton]. Its gesture depends on [CustomButton.mode]:
 * a combo presses and releases every key together like a real button would,
 * a sequence fires once per tap and is not held, and a hold toggle flips
 * [held] on tap rather than tracking the finger at all.
 */
@Composable
fun CustomButtonView(
    button: CustomButton,
    held: Boolean,
    onToggleHeld: () -> Unit,
    onKeyChange: (Int, Boolean) -> Unit,
) {
    var isPressed by remember { mutableStateOf(false) }
    val scope = rememberCoroutineScope()

    Button(
        onClick = { /* Handled via pointerInput; see below per mode. */ },
        modifier = Modifier
            .height(48.dp)
            .widthIn(min = 64.dp)
            .pointerInput(button.id, button.mode, button.keys) {
                when (button.mode) {
                    // The `finally` is not decoration: this loop is cancelled
                    // whenever the pointerInput key changes or the button
                    // leaves the composition, and it can be cancelled with the
                    // combo's keys still pressed.
                    CustomButtonMode.COMBO -> try {
                        awaitPointerEventScope {
                            while (true) {
                                val event = awaitPointerEvent()
                                val down = event.changes.any { it.pressed }
                                if (down != isPressed) {
                                    isPressed = down
                                    button.keys.forEach { key -> onKeyChange(key, down) }
                                }
                            }
                        }
                    } finally {
                        if (isPressed) {
                            isPressed = false
                            button.keys.forEach { key -> onKeyChange(key, false) }
                        }
                    }
                    CustomButtonMode.TOGGLE_HOLD -> detectTapGestures(onTap = { onToggleHeld() })
                    CustomButtonMode.SEQUENCE -> detectTapGestures(
                        onTap = {
                            scope.launch {
                                try {
                                    for (key in button.keys) {
                                        onKeyChange(key, true)
                                        delay(SEQUENCE_PRESS_MS)
                                        onKeyChange(key, false)
                                        delay(SEQUENCE_GAP_MS)
                                    }
                                } finally {
                                    // Cancelled between a press and its
                                    // release - the two `delay`s are the
                                    // cancellation points - would otherwise
                                    // leave that key down for good.
                                    button.keys.forEach { key -> onKeyChange(key, false) }
                                }
                            }
                        },
                    )
                }
            },
        colors = ButtonDefaults.buttonColors(
            containerColor = when {
                held -> GoldenSaplight
                isPressed -> AmberResin
                else -> HoneyDark
            },
        ),
        shape = pillShape,
        contentPadding = PaddingValues(horizontal = 12.dp),
    ) {
        Text(
            button.name,
            color = if (held) BurntRoot else PineGlowMist,
            fontSize = 12.sp,
            fontWeight = FontWeight.Bold,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
    }
}

/** How long a scripted key-tap in a [CustomButtonMode.SEQUENCE] stays down,
 *  and the gap before the next one - long enough for the core to register
 *  each press as its own frame of input, short enough to still read as one
 *  tap of the button. */
private const val SEQUENCE_PRESS_MS = 60L
private const val SEQUENCE_GAP_MS = 60L

/** Pause/resume and fast-forward toggle buttons, shared by the portrait and landscape layouts. */
@Composable
fun DPad(
    onKeyChange: (Int, Boolean) -> Unit,
    modifier: Modifier = Modifier,
    editingLayout: Boolean = false,
    selectedButton: ControlButton? = null,
    offsets: MutableMap<ControlButton, Offset> = mutableMapOf(),
    onDragStart: (ControlButton) -> Unit = {},
    onDragEnd: () -> Unit = {},
) {
    val buttonColor = HoneyDark
    val pressColor = AmberResin

    Column(
        modifier = modifier,
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        // Up
        Box(
            modifier = Modifier.movableControl(
                ControlButton.DPAD_UP, editingLayout, selectedButton, CircleShape, offsets, onDragStart, onDragEnd
            )
        ) {
            DPadButton(Icons.Default.KeyboardArrowUp, "Up", GbaEngine.KEY_UP, buttonColor, pressColor, onKeyChange)
        }
        // Left, Center, Right
        Row {
            Box(
                modifier = Modifier.movableControl(
                    ControlButton.DPAD_LEFT, editingLayout, selectedButton, CircleShape, offsets, onDragStart, onDragEnd
                )
            ) {
                DPadButton(Icons.Default.KeyboardArrowLeft, "Left", GbaEngine.KEY_LEFT, buttonColor, pressColor, onKeyChange)
            }
            Box(modifier = Modifier.size(48.dp))
            Box(
                modifier = Modifier.movableControl(
                    ControlButton.DPAD_RIGHT, editingLayout, selectedButton, CircleShape, offsets, onDragStart, onDragEnd
                )
            ) {
                DPadButton(Icons.Default.KeyboardArrowRight, "Right", GbaEngine.KEY_RIGHT, buttonColor, pressColor, onKeyChange)
            }
        }
        // Down
        Box(
            modifier = Modifier.movableControl(
                ControlButton.DPAD_DOWN, editingLayout, selectedButton, CircleShape, offsets, onDragStart, onDragEnd
            )
        ) {
            DPadButton(Icons.Default.KeyboardArrowDown, "Down", GbaEngine.KEY_DOWN, buttonColor, pressColor, onKeyChange)
        }
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
fun ActionButtons(
    onKeyChange: (Int, Boolean) -> Unit,
    editingLayout: Boolean = false,
    selectedButton: ControlButton? = null,
    offsets: MutableMap<ControlButton, Offset> = mutableMapOf(),
    onDragStart: (ControlButton) -> Unit = {},
    onDragEnd: () -> Unit = {},
) {
    val buttonColor = HoneyDark
    val pressColor = GoldenSaplight

    // A above B in a single column. Side by side reads left-to-right as "B
    // then A", which is the wrong way round from the hardware and puts the
    // button you press most under the weaker part of the thumb's arc.
    Column(
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        val aModifier = Modifier.movableControl(
            ControlButton.BUTTON_A, editingLayout, selectedButton, CircleShape, offsets, onDragStart, onDragEnd
        )
        val bModifier = Modifier.movableControl(
            ControlButton.BUTTON_B, editingLayout, selectedButton, CircleShape, offsets, onDragStart, onDragEnd
        )
        ActionButton("A", GbaEngine.KEY_A, buttonColor, pressColor, aModifier, onKeyChange)
        ActionButton("B", GbaEngine.KEY_B, buttonColor, pressColor, bModifier, onKeyChange)
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

/**
 * A floating confirmation over the play area.
 *
 * It replaces a full-width banner that sat in the layout column: that pushed
 * the screen down every time it appeared, and stayed until someone tapped an
 * X. This one floats, fades, and clears itself.
 */
@Composable
private fun StateToast(
    message: String?,
    onDismiss: () -> Unit,
    modifier: Modifier = Modifier,
) {
    // Held locally so the pill can finish fading out after the message is
    // already gone from the ViewModel.
    var shown by remember { mutableStateOf<String?>(null) }
    LaunchedEffect(message) {
        if (message != null) {
            shown = message
            delay(2200)
            onDismiss()
        }
    }

    AnimatedVisibility(
        visible = message != null,
        enter = fadeIn(tween(150)) + slideInVertically(tween(180)) { -it / 2 },
        exit = fadeOut(tween(220)),
        modifier = modifier.padding(top = 56.dp),
    ) {
        Row(
            modifier = Modifier
                .clip(RoundedCornerShape(50))
                .background(HoneyDark)
                .border(1.dp, GoldenSaplight.copy(alpha = 0.45f), RoundedCornerShape(50))
                .padding(horizontal = 16.dp, vertical = 9.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Icon(
                Icons.Default.CheckCircle,
                contentDescription = null,
                tint = GoldenSaplight,
                modifier = Modifier.size(16.dp),
            )
            Spacer(modifier = Modifier.width(8.dp))
            Text(
                text = shown.orEmpty(),
                color = PineGlowMist,
                fontSize = 13.sp,
                fontWeight = FontWeight.Medium,
            )
        }
    }
}

/**
 * The slot list behind the menu's "Save States...".
 *
 * Slot 0 is the same state the toolbar's save and load buttons use, so a
 * quick save shows up here and a slot written here can be loaded from the
 * toolbar. The rest are only reachable from this list.
 */
@Composable
private fun SaveStateDialog(
    slots: List<StateSlot>,
    onSave: (Int) -> Unit,
    onLoad: (Int) -> Unit,
    onDismiss: () -> Unit,
) {
    val formatter = remember {
        java.text.SimpleDateFormat("d MMM, HH:mm", java.util.Locale.getDefault())
    }
    AlertDialog(
        onDismissRequest = onDismiss,
        containerColor = HoneyDark,
        titleContentColor = GoldenSaplight,
        textContentColor = PineGlowMist,
        title = { Text("Save states", fontWeight = FontWeight.Bold) },
        text = {
            Column(
                modifier = Modifier.verticalScroll(rememberScrollState()),
                verticalArrangement = Arrangement.spacedBy(4.dp),
            ) {
                slots.forEach { slot ->
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Column(modifier = Modifier.weight(1f)) {
                            Text(
                                text = if (slot.index == 0) "Quick save" else "Slot ${slot.index}",
                                fontSize = 14.sp,
                                fontWeight = FontWeight.SemiBold,
                                color = PineGlowMist,
                            )
                            Text(
                                text = if (slot.exists) {
                                    formatter.format(java.util.Date(slot.savedAt))
                                } else {
                                    "Empty"
                                },
                                fontSize = 11.sp,
                                color = AmberResin,
                            )
                        }
                        TextButton(onClick = { onSave(slot.index) }) {
                            Text("Save", color = GoldenSaplight, fontSize = 13.sp)
                        }
                        TextButton(
                            onClick = { onLoad(slot.index) },
                            enabled = slot.exists,
                        ) {
                            Text(
                                "Load",
                                color = if (slot.exists) GoldenSaplight else AmberResin,
                                fontSize = 13.sp,
                            )
                        }
                    }
                }
            }
        },
        confirmButton = {
            TextButton(onClick = onDismiss) { Text("Close", color = GoldenSaplight) }
        },
    )
}

/**
 * Manages the saved touch-overlay layouts: which one plays this game, and
 * creating, renaming or deleting any of them. Default can never be deleted -
 * it is the fallback every game with no layout of its own uses.
 *
 * Picking a layout's radio button just switches to it; its edit icon does
 * the same and also opens the drag editor, since editing only ever makes
 * sense on the layout that is actually loaded.
 */
@Composable
private fun LayoutsDialog(
    layouts: List<ControlLayout>,
    activeLayoutId: String,
    onSelect: (ControlLayout) -> Unit,
    onEdit: (ControlLayout) -> Unit,
    onRename: (String, String) -> Unit,
    onDelete: (ControlLayout) -> Unit,
    onCreate: (String) -> Unit,
    onDismiss: () -> Unit,
) {
    var renamingLayout by remember { mutableStateOf<ControlLayout?>(null) }
    var creatingLayout by remember { mutableStateOf(false) }

    AlertDialog(
        onDismissRequest = onDismiss,
        containerColor = HoneyDark,
        titleContentColor = GoldenSaplight,
        textContentColor = PineGlowMist,
        title = { Text("Control Layouts", fontWeight = FontWeight.Bold) },
        text = {
            Column(
                modifier = Modifier.verticalScroll(rememberScrollState()),
                verticalArrangement = Arrangement.spacedBy(2.dp),
            ) {
                layouts.forEach { layout ->
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        RadioButton(
                            selected = layout.id == activeLayoutId,
                            onClick = { onSelect(layout) },
                            colors = RadioButtonDefaults.colors(
                                selectedColor = GoldenSaplight,
                                unselectedColor = AmberResin,
                            ),
                        )
                        Text(
                            text = layout.name,
                            color = PineGlowMist,
                            fontSize = 14.sp,
                            modifier = Modifier
                                .weight(1f)
                                .clickable { onSelect(layout) },
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                        IconButton(onClick = { onEdit(layout) }) {
                            Icon(Icons.Default.OpenWith, "Edit positions", tint = AmberResin)
                        }
                        IconButton(onClick = { renamingLayout = layout }) {
                            Icon(Icons.Default.Edit, "Rename", tint = AmberResin)
                        }
                        IconButton(onClick = { onDelete(layout) }, enabled = !layout.isDefault) {
                            Icon(
                                Icons.Default.Delete,
                                "Delete",
                                tint = if (layout.isDefault) PineGlowMist.copy(alpha = 0.3f) else AmberResin,
                            )
                        }
                    }
                }
            }
        },
        confirmButton = {
            TextButton(onClick = { creatingLayout = true }) {
                Text("+ New Layout", color = GoldenSaplight)
            }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) { Text("Close", color = PineGlowMist) }
        },
    )

    renamingLayout?.let { layout ->
        LayoutNameDialog(
            title = "Rename layout",
            initialName = layout.name,
            onConfirm = { name -> onRename(layout.id, name); renamingLayout = null },
            onDismiss = { renamingLayout = null },
        )
    }

    if (creatingLayout) {
        LayoutNameDialog(
            title = "New layout",
            initialName = "Layout ${layouts.size + 1}",
            onConfirm = { name -> onCreate(name); creatingLayout = false },
            onDismiss = { creatingLayout = false },
        )
    }
}

/** A single text field prompt, shared by renaming a layout and naming a new one. */
@Composable
private fun LayoutNameDialog(
    title: String,
    initialName: String,
    onConfirm: (String) -> Unit,
    onDismiss: () -> Unit,
) {
    var name by remember { mutableStateOf(initialName) }
    AlertDialog(
        onDismissRequest = onDismiss,
        containerColor = HoneyDark,
        titleContentColor = GoldenSaplight,
        textContentColor = PineGlowMist,
        title = { Text(title) },
        text = {
            OutlinedTextField(
                value = name,
                onValueChange = { name = it },
                singleLine = true,
                colors = OutlinedTextFieldDefaults.colors(
                    focusedTextColor = PineGlowMist,
                    unfocusedTextColor = PineGlowMist,
                    focusedBorderColor = AmberResin,
                    unfocusedBorderColor = AmberResin.copy(alpha = 0.5f),
                    cursorColor = AmberResin,
                ),
            )
        },
        confirmButton = {
            TextButton(
                onClick = { onConfirm(name.trim().ifBlank { initialName }) },
            ) { Text("Save", color = GoldenSaplight) }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) { Text("Cancel", color = PineGlowMist) }
        },
    )
}
