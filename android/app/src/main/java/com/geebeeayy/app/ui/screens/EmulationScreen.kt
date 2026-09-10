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
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.geometry.RoundRect
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Outline
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.clipPath
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
import android.os.BatteryManager
import com.geebeeayy.app.data.ControlButton
import com.geebeeayy.app.data.ControlPalette
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
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.LayoutDirection
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.geebeeayy.app.data.ScreenFilter
import com.geebeeayy.app.data.StateSlot
import com.geebeeayy.app.data.ScaleMode
import com.geebeeayy.app.engine.GbaEngine
import com.geebeeayy.app.ui.theme.*
import kotlin.math.abs
import kotlin.math.atan2
import kotlin.math.floor
import kotlin.math.hypot
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
    fastForward: Boolean = false,
    isRewinding: Boolean = false,
    onRewind: (Boolean) -> Unit = {},
    controlScale: Float = 1f,
    controlOpacity: Float = 1f,
    onSaveState: (Int) -> Unit,
    onLoadState: (Int) -> Unit,
    stateSlots: () -> List<StateSlot> = { emptyList() },
    onScreenshot: () -> Unit = {},
    onSettings: () -> Unit = {},
    soundEnabled: Boolean = true,
    onToggleSound: () -> Unit = {},
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
    /** Per-control size multipliers, alongside [offsets] and saved with them. */
    val scales = remember { mutableStateMapOf<ControlButton, Float>() }
    // Custom buttons (combos, sequences, hold toggles) belong to the layout
    // the same way the real buttons' positions do. Their drag is separate
    // from the real buttons' undo/redo/Done staging below - each drag saves
    // its new position immediately, since there is no equivalent "abandon
    // this edit" concern for a position with no default to revert to.
    var customButtons by remember { mutableStateOf<List<CustomButton>>(emptyList()) }
    val customOffsets = remember { mutableStateMapOf<String, Offset>() }
    val customScales = remember { mutableStateMapOf<String, Float>() }
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
        scales.clear()
        ControlButton.entries.forEach { button ->
            val (x, y) = layoutStore.getControlOffset(layoutId, button)
            offsets[button] = Offset(x, y)
            scales[button] = layoutStore.getControlScale(layoutId, button)
        }
        customButtons = layoutStore.getCustomButtons(layoutId)
        customOffsets.clear()
        customScales.clear()
        heldToggles.clear()
        customButtons.forEach { button ->
            val (x, y) = layoutStore.getCustomButtonOffset(layoutId, button.id)
            customOffsets[button.id] = Offset(x, y)
            customScales[button.id] = layoutStore.getCustomButtonScale(layoutId, button.id)
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
    val undoStack = remember { mutableStateListOf<LayoutSnapshot>() }
    val redoStack = remember { mutableStateListOf<LayoutSnapshot>() }
    fun snapshot() = LayoutSnapshot(offsets.toMap(), scales.toMap(), customScales.toMap())
    fun restore(state: LayoutSnapshot) {
        offsets.clear(); offsets.putAll(state.offsets)
        scales.clear(); scales.putAll(state.scales)
        customScales.clear(); customScales.putAll(state.customScales)
    }
    val pushUndoSnapshot: () -> Unit = {
        undoStack.add(snapshot())
        redoStack.clear()
    }
    val undoEdit: () -> Unit = {
        undoStack.removeLastOrNull()?.let { previous ->
            redoStack.add(snapshot())
            restore(previous)
        }
    }
    val redoEdit: () -> Unit = {
        redoStack.removeLastOrNull()?.let { next ->
            undoStack.add(snapshot())
            restore(next)
        }
    }
    // One press of smaller/bigger, applied to whatever is selected. Each
    // press is its own undo step, which is what a stepper should be: a slider
    // would bury a whole gesture's worth of change behind one undo.
    val resizeSelected: (Float) -> Unit = { delta ->
        pushUndoSnapshot()
        val customId = selectedCustomId
        val button = selectedButton
        if (customId != null) {
            customScales[customId] = ((customScales[customId] ?: 1f) + delta)
                .coerceIn(ControlLayoutStore.MIN_CONTROL_SCALE, ControlLayoutStore.MAX_CONTROL_SCALE)
        } else if (button != null) {
            scales[button] = ((scales[button] ?: 1f) + delta)
                .coerceIn(ControlLayoutStore.MIN_CONTROL_SCALE, ControlLayoutStore.MAX_CONTROL_SCALE)
        }
    }
    val handleDragStart: (ControlButton) -> Unit = { button ->
        pushUndoSnapshot()
        selectedButton = button
        selectedCustomId = null
    }
    // The selection outlives the drag on purpose: the size buttons in the
    // edit bar act on whatever is selected, and clearing it here left nothing
    // to act on the moment the finger came up.
    val handleDragEnd: () -> Unit = {}
    // A custom button's position saves the moment the drag ends - there is
    // no Done to stage it behind, so `selectedCustomId` still names the one
    // that just finished when this fires.
    val handleCustomDragStart: (String) -> Unit = { id ->
        selectedCustomId = id
        selectedButton = null
    }
    val handleCustomDragEnd: () -> Unit = {
        selectedCustomId?.let { id ->
            val pos = customOffsets[id] ?: Offset.Zero
            layoutStore.setCustomButtonOffset(activeLayoutId, id, pos.x, pos.y)
        }
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
            // Plain black rather than the palette's NightVoid: this is the
            // in-game screen, so the background should recede and let the
            // palette-coloured icons and buttons be the only colour on it.
            // Dialogs (save states, layouts) and overlay chrome (the state
            // toast, the layout-edit bar) keep NightVoid/NightPanel - they are
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
                IconButton(onClick = onToggleSound) {
                    Icon(
                        if (soundEnabled) Icons.Default.VolumeUp
                        else Icons.Default.VolumeOff,
                        contentDescription = if (soundEnabled) "Mute" else "Unmute",
                        // Lit only when muted: the interesting state is the one
                        // that explains why a game went quiet.
                        tint = if (soundEnabled) PineGlowMist else GoldenSaplight,
                    )
                }
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
                // On or off, with no speed to pick: fast forward is
                // unthrottled and runs at whatever the device can manage.
                IconButton(onClick = onFastForward) {
                    Icon(
                        Icons.Default.FastForward,
                        "Fast forward",
                        tint = if (fastForward) GoldenSaplight else PineGlowMist,
                    )
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
                // The menu has to be declared inside a Box wrapping its own
                // button: `DropdownMenu` anchors to whatever it is nested in,
                // not to whatever opened it, so sitting at the root of the
                // screen put it in the bottom-left corner, a phone's width
                // away from the icon that summons it.
                Box {
                    IconButton(onClick = { showMenu = !showMenu }) {
                        Icon(Icons.Default.MoreVert, "Menu", tint = PineGlowMist)
                    }
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
                        HorizontalDivider(color = NightEdge)
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
                            onClick = { showMenu = false; onSettings() },
                            leadingIcon = { Icon(Icons.Default.Settings, null, tint = AmberResin) }
                        )
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
                        editingLayout = editingLayout,
                        selectedButton = selectedButton,
                        offsets = offsets,
                        scales = scales,
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
                            scales = scales,
                            onDragStart = handleDragStart,
                            onDragEnd = handleDragEnd,
                        )
                    }
                }
            } else {
                // The controls float over the picture rather than sitting
                // under it. Stacked, the screen gets the whole height instead
                // of whatever the button block left over, which on a 20:9
                // phone is most of the difference between a 3x and a 4x
                // picture. The buttons are translucent so what they cover is
                // still readable - see the opacity setting.
                Box(
                    modifier = Modifier
                        .fillMaxWidth()
                        .weight(1f)
                ) {
                ScreenContainer(
                    frameBuffer = frameBuffer,
                    isLoading = isLoading,
                    errorMessage = errorMessage,
                    isPaused = isPaused,
                    scaleMode = scaleMode,
                    screenFilter = screenFilter,
                    modifier = Modifier
                        .fillMaxSize()
                        // 24.dp a side left only 948 px of a 1080 px screen,
                        // and integer scaling rounds that down to 3x. 8.dp
                        // clears 960 px, which is exactly 4x.
                        .padding(horizontal = 8.dp, vertical = 8.dp),
                    // Pinned to the top, not centred. A 3:2 picture on a 20:9
                    // phone is limited by width, never by height, so the extra
                    // height this layout hands it buys nothing - it only
                    // decides where the leftover black goes. Putting all of it
                    // below the picture is what leaves room for the controls
                    // to float without covering the game.
                    pictureVerticalBias = 0f,
                )

                // One transform on the whole block rather than a size
                // multiplier threaded through every button: Compose maps
                // pointer input through the layer, so the touch targets grow
                // with the drawing and stay in register.
                Box(
                    modifier = Modifier
                        .align(Alignment.BottomCenter)
                        .graphicsLayer(
                            scaleX = controlScale,
                            scaleY = controlScale,
                            alpha = controlOpacity,
                            transformOrigin = TransformOrigin(0.5f, 1f),
                        )
                ) {
                GameControls(
                    isPaused = isPaused,
                    isFastForward = fastForward,
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
                    scales = scales,
                    selectedButton = selectedButton,
                    onDragStart = handleDragStart,
                    onDragEnd = handleDragEnd,
                )
                }
                }
            }
        }

        // Battery and clock, in the two bottom corners. Small and dim, and in
        // the corners on purpose: Start and Select sit centred at the bottom,
        // so this is the one strip of screen no control wants.
        if (!editingLayout) {
            StatusStrip(modifier = Modifier.align(Alignment.BottomCenter))
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
                    customScales,
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
                selectionLabel = selectedCustomId
                    ?.let { id -> customButtons.firstOrNull { it.id == id }?.name }
                    ?: selectedButton?.label,
                onResize = resizeSelected,
                onDone = {
                    ControlButton.entries.forEach { button ->
                        val o = offsets[button] ?: Offset.Zero
                        layoutStore.setControlOffset(activeLayoutId, button, o.x, o.y)
                        layoutStore.setControlScale(activeLayoutId, button, scales[button] ?: 1f)
                    }
                    customScales.forEach { (id, scale) ->
                        layoutStore.setCustomButtonScale(activeLayoutId, id, scale)
                    }
                    editingLayout = false
                    selectedButton = null
                    undoStack.clear()
                    redoStack.clear()
                },
                onReset = {
                    pushUndoSnapshot()
                    layoutStore.resetLayoutOffsets(activeLayoutId)
                    ControlButton.entries.forEach {
                        offsets[it] = Offset.Zero
                        scales[it] = 1f
                    }
                    customScales.keys.toList().forEach { customScales[it] = 1f }
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
                    // Release first, like onDelete does. Editing a held
                    // TOGGLE_HOLD button replaces its key list and restarts
                    // its pointerInput, so whatever it was holding would never
                    // be released - rebinding a held button from L to R left L
                    // down for the session.
                    if (heldToggles[button.id] == true) {
                        customButtons.firstOrNull { it.id == button.id }?.keys
                            ?.forEach { key -> onKeyChange(key, false) }
                        heldToggles.remove(button.id)
                    }
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
    pictureVerticalBias: Float = 0.5f,
) {
    Box(
        modifier = modifier
            .clip(RoundedCornerShape(8.dp))
            .background(Color.Black),
        contentAlignment = Alignment.Center,
    ) {
        if (frameBuffer != null) {
            GbaScreen(
                frameBuffer = frameBuffer,
                scaleMode = scaleMode,
                filter = screenFilter,
                verticalBias = pictureVerticalBias,
            )
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
    selectionLabel: String?,
    onResize: (Float) -> Unit,
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
            .background(NightVoid.copy(alpha = 0.92f))
            .padding(horizontal = 16.dp, vertical = 10.dp),
    ) {
        // The hint and the controls used to share one row with SpaceBetween.
        // The text takes its full intrinsic width first, which left too little
        // for five controls: "Reset" wrapped to two lines and "Done" was
        // pushed off the right edge entirely, so there was no way out of edit
        // mode. Giving the controls a row of their own fits them at any width.
        // The hint and the size stepper share a row: the bar floats over the
        // toolbar, and a third row buried the back button behind it.
        Row(
            modifier = Modifier.fillMaxWidth(),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(
                text = selectionLabel?.let { "$it selected" } ?: "Touch a button, then drag it",
                color = PineGlowMist,
                fontSize = 14.sp,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.weight(1f),
            )
            // Greyed rather than hidden: a stepper that appears and vanishes
            // makes the bar jump under the finger that is still dragging.
            val canResize = selectionLabel != null
            Text(
                text = "Size",
                color = if (canResize) PineGlowMist else PineGlowMist.copy(alpha = 0.3f),
                fontSize = 14.sp,
            )
            IconButton(
                onClick = { onResize(-ControlLayoutStore.CONTROL_SCALE_STEP) },
                enabled = canResize,
            ) {
                Icon(
                    Icons.Default.Remove,
                    contentDescription = "Smaller",
                    tint = if (canResize) AmberResin else PineGlowMist.copy(alpha = 0.3f),
                )
            }
            IconButton(
                onClick = { onResize(ControlLayoutStore.CONTROL_SCALE_STEP) },
                enabled = canResize,
            ) {
                Icon(
                    Icons.Default.Add,
                    contentDescription = "Bigger",
                    tint = if (canResize) AmberResin else PineGlowMist.copy(alpha = 0.3f),
                )
            }
        }
        Row(
            modifier = Modifier.fillMaxWidth(),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.End,
        ) {
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
    /**
     * Where the picture sits in the space it is given, 0 for the top edge and
     * 1 for the bottom. Only matters when the space is taller than the
     * picture, which on a portrait phone it always is: a 3:2 picture is
     * limited by width there, never by height.
     */
    verticalBias: Float = 0.5f,
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
    //
    // Measured against the *source* frame, not `width`/`height`: those are
    // already doubled for 2xSaI, so this asked for 460800 bytes of a buffer
    // that is always 115200 and returned before drawing anything. Picking the
    // 2xSaI filter left the play area black for the whole session.
    if (frameBuffer.size < GbaEngine.FRAME_BUFFER_SIZE) {
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
        val dstOffsetY = (size.height - dstHeight) * verticalBias.coerceIn(0f, 1f)
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
    scales: Map<T, Float> = emptyMap(),
): Modifier {
    val offset = offsets[button] ?: Offset.Zero
    val scale = scales[button] ?: 1f
    return this
        .offset { IntOffset(offset.x.roundToInt(), offset.y.roundToInt()) }
        // A layer, not a size change. Compose maps pointer input back through
        // it, so the touch target grows with the drawing and stays in
        // register - and the neighbours do not move to make room, which is
        // the point of a layout the player positioned by hand.
        .graphicsLayer(scaleX = scale, scaleY = scale)
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
    scales: Map<ControlButton, Float> = emptyMap(),
    selectedButton: ControlButton? = null,
    onDragStart: (ControlButton) -> Unit = {},
    onDragEnd: () -> Unit = {},
) {
    Column(modifier = Modifier.fillMaxWidth()) {
        // Shoulder buttons sit above the rest, where the real hardware puts
        // them: L on the far left, R on the far right.
        ShoulderRow(onKeyChange, editingLayout, selectedButton, offsets, scales, onDragStart, onDragEnd)

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
                scales = scales,
                onDragStart = onDragStart,
                onDragEnd = onDragEnd,
            )
            ActionButtons(onKeyChange, editingLayout, selectedButton, offsets, scales, onDragStart, onDragEnd)
        }

        // Start and Select. Without these most games cannot get past a title
        // screen, so they are not optional extras.
        StartSelectRow(onKeyChange, editingLayout, selectedButton, offsets, scales, onDragStart, onDragEnd)
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
    scales: Map<ControlButton, Float> = emptyMap(),
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
                ControlButton.SHOULDER_L, editingLayout, selectedButton, pillShape, offsets, onDragStart, onDragEnd, scales
            )
        ) {
            PillButton("L", GbaEngine.KEY_L, onKeyChange)
        }
        Box(
            modifier = Modifier.movableControl(
                ControlButton.SHOULDER_R, editingLayout, selectedButton, pillShape, offsets, onDragStart, onDragEnd, scales
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
    scales: Map<ControlButton, Float> = emptyMap(),
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
                ControlButton.SELECT, editingLayout, selectedButton, pillShape, offsets, onDragStart, onDragEnd, scales
            )
        ) {
            PillButton("SELECT", GbaEngine.KEY_SELECT, onKeyChange)
        }
        Spacer(modifier = Modifier.width(24.dp))
        Box(
            modifier = Modifier.movableControl(
                ControlButton.START, editingLayout, selectedButton, pillShape, offsets, onDragStart, onDragEnd, scales
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
    val controls = LocalControlPalette.current
    var isPressed by remember { mutableStateOf(false) }

    Button(
        onClick = { /* Handled via pointerInput; a tap needs press+release reported. */ },
        modifier = Modifier
            .height(48.dp)
            .widthIn(min = 72.dp)
            .pointerInput(key) {
                // The `finally` is what stops a key sticking down: this
                // loop is cancelled when the pointerInput key changes or the
                // button leaves the composition, and it can be cancelled with
                // a finger still on the button. Rotating the screen with the
                // D-pad held disposes the whole control block, so without
                // this the key stayed pressed for the rest of the session.
                try {
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
                } finally {
                    if (isPressed) {
                        isPressed = false
                        onKeyChange(key, false)
                    }
                }
            },
        colors = ButtonDefaults.buttonColors(
            containerColor = if (isPressed) controls.pressed else controls.fill,
        ),
        shape = RoundedCornerShape(24.dp),
        contentPadding = PaddingValues(horizontal = 16.dp),
    ) {
        Text(
            label,
            color = if (isPressed) controls.labelPressed else controls.label,
            fontSize = (13 * LocalControlFontScale.current).sp,
            fontWeight = FontWeight.Bold,
        )
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
    val controls = LocalControlPalette.current
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
                held -> controls.pressed
                isPressed -> controls.pressed
                else -> controls.fill
            },
        ),
        shape = pillShape,
        contentPadding = PaddingValues(horizontal = 12.dp),
    ) {
        Text(
            button.name,
            color = if (held || isPressed) controls.labelPressed else controls.label,
            fontSize = (12 * LocalControlFontScale.current).sp,
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

/**
 * The D-pad: one cross with one touch area, not four buttons.
 *
 * It was four independent buttons, and four buttons can never produce a
 * diagonal from a single finger - there is no shared corner to press. Half
 * the GBA library assumes otherwise: every isometric map, every Mode 7 racer,
 * every cursor that has to reach a tile at 45 degrees.
 *
 * The touch area is the whole square, not only the drawn arms. A finger in
 * the top-right corner - outside the cross, on the hardware's dead plastic -
 * reads as Up+Right. That is what makes a diagonal reachable on glass, where
 * there is no ridge to feel for. Sliding a thumb from one sector to the next
 * re-reads on every move, so Up to Up+Right is a slide, not a lift and a tap.
 */
@Composable
fun DPad(
    onKeyChange: (Int, Boolean) -> Unit,
    modifier: Modifier = Modifier,
    editingLayout: Boolean = false,
    selectedButton: ControlButton? = null,
    offsets: MutableMap<ControlButton, Offset> = mutableMapOf(),
    scales: Map<ControlButton, Float> = emptyMap(),
    onDragStart: (ControlButton) -> Unit = {},
    onDragEnd: () -> Unit = {},
) {
    val controls = LocalControlPalette.current
    var held by remember { mutableStateOf(emptySet<Int>()) }

    Box(
        modifier = modifier.movableControl(
            ControlButton.DPAD, editingLayout, selectedButton, DPadCrossShape, offsets, onDragStart, onDragEnd, scales
        )
    ) {
        Canvas(
            modifier = Modifier
                .size(DPAD_SIZE)
                .semantics { contentDescription = "Direction pad" }
                // Keyed on editingLayout so flipping into the position editor
                // cancels this block, which runs the `finally` below and
                // releases anything still down. Dragging the pad must not
                // also press it.
                .pointerInput(editingLayout) {
                    if (editingLayout) return@pointerInput
                    try {
                        awaitPointerEventScope {
                            while (true) {
                                val event = awaitPointerEvent()
                                val pointer = event.changes.firstOrNull { it.pressed }
                                val next = if (pointer == null) {
                                    emptySet()
                                } else {
                                    dpadKeysAt(pointer.position, size)
                                }
                                if (next != held) {
                                    (held - next).forEach { onKeyChange(it, false) }
                                    (next - held).forEach { onKeyChange(it, true) }
                                    held = next
                                }
                                event.changes.forEach { if (it.pressed) it.consume() }
                            }
                        }
                    } finally {
                        // Rotating the screen with a direction held disposes
                        // this whole control block. Without the release the
                        // key stayed down for the rest of the session.
                        held.forEach { onKeyChange(it, false) }
                        held = emptySet()
                    }
                }
        ) {
            val cross = crossPath(size)
            drawPath(cross, controls.fill)

            // Held arms are clipped to the cross, so the outer end keeps the
            // cross's rounded cap and the inner end disappears under the dish.
            if (held.isNotEmpty()) {
                val arm = size.minDimension / 3f
                val left = (size.width - arm) / 2f
                val top = (size.height - arm) / 2f
                clipPath(cross) {
                    if (GbaEngine.KEY_UP in held) {
                        drawRect(controls.pressed, Offset(left, 0f), Size(arm, size.height / 2f))
                    }
                    if (GbaEngine.KEY_DOWN in held) {
                        drawRect(controls.pressed, Offset(left, size.height / 2f), Size(arm, size.height / 2f))
                    }
                    if (GbaEngine.KEY_LEFT in held) {
                        drawRect(controls.pressed, Offset(0f, top), Size(size.width / 2f, arm))
                    }
                    if (GbaEngine.KEY_RIGHT in held) {
                        drawRect(controls.pressed, Offset(size.width / 2f, top), Size(size.width / 2f, arm))
                    }
                }
            }

            // An arrow at the end of each arm, the way the moulded ones sit
            // on an AGB-001 pad. They also say which way a diagonal is,
            // which the plain cross could not: the corner between two arms
            // presses both, and nothing on the shape hinted at that.
            drawDpadArrow(GbaEngine.KEY_UP, held, controls)
            drawDpadArrow(GbaEngine.KEY_DOWN, held, controls)
            drawDpadArrow(GbaEngine.KEY_LEFT, held, controls)
            drawDpadArrow(GbaEngine.KEY_RIGHT, held, controls)

            // The dish is drawn at exactly the dead zone's radius, so what a
            // player sees as the thumb rest is the region that reports nothing.
            drawCircle(controls.dish, radius = size.minDimension / 2f * DPAD_DEAD_ZONE, center = center)
        }
    }
}

/**
 * One moulded arrow, pointing out along its arm.
 *
 * Ink on the honey fill when that direction is held and a light mark on the
 * dark panel when it is not - a single colour would vanish against one of the
 * two, and the arrow is most worth seeing at the moment it lights up.
 */
private fun DrawScope.drawDpadArrow(key: Int, held: Set<Int>, controls: ControlPalette) {
    val arm = size.minDimension / 3f
    val half = arm * DPAD_ARROW_HALF_WIDTH
    val depth = arm * DPAD_ARROW_DEPTH
    val inset = arm * DPAD_ARROW_INSET
    val cx = size.width / 2f
    val cy = size.height / 2f

    // Apex first, then the two base corners, walked clockwise.
    val points = when (key) {
        GbaEngine.KEY_UP -> listOf(
            Offset(cx, inset),
            Offset(cx + half, inset + depth),
            Offset(cx - half, inset + depth),
        )
        GbaEngine.KEY_DOWN -> listOf(
            Offset(cx, size.height - inset),
            Offset(cx - half, size.height - inset - depth),
            Offset(cx + half, size.height - inset - depth),
        )
        GbaEngine.KEY_LEFT -> listOf(
            Offset(inset, cy),
            Offset(inset + depth, cy - half),
            Offset(inset + depth, cy + half),
        )
        else -> listOf(
            Offset(size.width - inset, cy),
            Offset(size.width - inset - depth, cy + half),
            Offset(size.width - inset - depth, cy - half),
        )
    }

    val path = Path().apply {
        moveTo(points[0].x, points[0].y)
        lineTo(points[1].x, points[1].y)
        lineTo(points[2].x, points[2].y)
        close()
    }
    drawPath(path, if (key in held) controls.labelPressed else controls.label)
}

/** Three 48dp arms, the same footprint the four separate buttons occupied. */
private val DPAD_SIZE = 144.dp

/** Arrow geometry, as fractions of one arm's width. */
private const val DPAD_ARROW_HALF_WIDTH = 0.22f
private const val DPAD_ARROW_DEPTH = 0.30f
private const val DPAD_ARROW_INSET = 0.24f

/**
 * Half-width of a cardinal's sector, in degrees. At 30 each cardinal owns 60
 * degrees and each diagonal 30, so a straight Up is hard to fumble into
 * Up+Right while the diagonal is still there when it is aimed for. Raise it
 * to make diagonals harder to hit, lower it to make them easier - this is the
 * one number worth tuning against a real thumb on real glass.
 */
private const val DPAD_CARDINAL_HALF_DEGREES = 30f

/**
 * Fraction of the cross's half-width that reports nothing. The centre is a
 * thumb rest; without a dead zone the smallest wobble there flips between
 * opposite directions, which reads as the pad fighting you.
 */
private const val DPAD_DEAD_ZONE = 0.22f

/**
 * Which directions a touch at [position] means, given a pad of [size].
 *
 * Internal rather than private so DPadDirectionTest can drive it directly:
 * this is the part with the arithmetic, and it needs a test more than the
 * drawing does.
 */
internal fun dpadKeysAt(position: Offset, size: IntSize): Set<Int> {
    val half = minOf(size.width, size.height) / 2f
    if (half <= 0f) return emptySet()

    val dx = position.x - size.width / 2f
    val dy = position.y - size.height / 2f
    if (hypot(dx, dy) < half * DPAD_DEAD_ZONE) return emptySet()

    // Screen y grows downward, so it is negated to get ordinary maths angles:
    // 0 right, 90 up, 180 left, 270 down.
    var degrees = Math.toDegrees(atan2(-dy.toDouble(), dx.toDouble())).toFloat()
    if (degrees < 0f) degrees += 360f

    val cardinal = listOf(
        0f to GbaEngine.KEY_RIGHT,
        90f to GbaEngine.KEY_UP,
        180f to GbaEngine.KEY_LEFT,
        270f to GbaEngine.KEY_DOWN,
    ).firstOrNull { (centre, _) ->
        // Shortest angular distance to the sector's centre.
        abs(((degrees - centre + 540f) % 360f) - 180f) <= DPAD_CARDINAL_HALF_DEGREES
    }
    if (cardinal != null) return setOf(cardinal.second)

    return when {
        degrees < 90f -> setOf(GbaEngine.KEY_UP, GbaEngine.KEY_RIGHT)
        degrees < 180f -> setOf(GbaEngine.KEY_UP, GbaEngine.KEY_LEFT)
        degrees < 270f -> setOf(GbaEngine.KEY_DOWN, GbaEngine.KEY_LEFT)
        else -> setOf(GbaEngine.KEY_DOWN, GbaEngine.KEY_RIGHT)
    }
}

/** Two rounded bars crossing - the AGB-001 rocker, which carries no arrows. */
private fun crossPath(size: Size): Path {
    val arm = size.minDimension / 3f
    val radius = CornerRadius(arm * 0.28f)
    return Path().apply {
        addRoundRect(
            RoundRect(Rect((size.width - arm) / 2f, 0f, (size.width + arm) / 2f, size.height), radius)
        )
        addRoundRect(
            RoundRect(Rect(0f, (size.height - arm) / 2f, size.width, (size.height + arm) / 2f), radius)
        )
    }
}

/** Lets the position editor's selection border trace the cross rather than a
 *  box around it, so what is highlighted is what will move. */
private val DPadCrossShape = object : Shape {
    override fun createOutline(size: Size, layoutDirection: LayoutDirection, density: Density): Outline =
        Outline.Generic(crossPath(size))
}

@Composable
fun ActionButtons(
    onKeyChange: (Int, Boolean) -> Unit,
    editingLayout: Boolean = false,
    selectedButton: ControlButton? = null,
    offsets: MutableMap<ControlButton, Offset> = mutableMapOf(),
    scales: Map<ControlButton, Float> = emptyMap(),
    onDragStart: (ControlButton) -> Unit = {},
    onDragEnd: () -> Unit = {},
) {
    val controls = LocalControlPalette.current
    val buttonColor = controls.fill
    val pressColor = controls.pressed

    // A above B in a single column. Side by side reads left-to-right as "B
    // then A", which is the wrong way round from the hardware and puts the
    // button you press most under the weaker part of the thumb's arc.
    Column(
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        val aModifier = Modifier.movableControl(
            ControlButton.BUTTON_A, editingLayout, selectedButton, CircleShape, offsets, onDragStart, onDragEnd, scales
        )
        val bModifier = Modifier.movableControl(
            ControlButton.BUTTON_B, editingLayout, selectedButton, CircleShape, offsets, onDragStart, onDragEnd, scales
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
    val controls = LocalControlPalette.current
    var isPressed by remember { mutableStateOf(false) }

    Button(
        onClick = { /* Handled via pointerInput below; a tap needs press+release reported. */ },
        modifier = modifier
            .size(56.dp)
            .pointerInput(key) {
                // The `finally` is what stops a key sticking down: this
                // loop is cancelled when the pointerInput key changes or the
                // button leaves the composition, and it can be cancelled with
                // a finger still on the button. Rotating the screen with the
                // D-pad held disposes the whole control block, so without
                // this the key stayed pressed for the rest of the session.
                try {
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
                } finally {
                    if (isPressed) {
                        isPressed = false
                        onKeyChange(key, false)
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
            color = if (isPressed) controls.labelPressed else controls.label,
            fontSize = (20 * LocalControlFontScale.current).sp,
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
                .background(NightPanel)
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
        containerColor = NightPanel,
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
                                text = if (slot.index == 0) "Quick save slot" else "Slot ${slot.index}",
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
        containerColor = NightPanel,
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
        containerColor = NightPanel,
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

/**
 * One step of the layout editor's undo history.
 *
 * Positions and sizes travel together because they are edited together: undo
 * after "move it, then make it bigger" has to put back the size the move was
 * made at, not just the position.
 */
private data class LayoutSnapshot(
    val offsets: Map<ControlButton, Offset>,
    val scales: Map<ControlButton, Float>,
    val customScales: Map<String, Float>,
)

/**
 * The phone's battery level and the time, along the bottom edge.
 *
 * A game covers the system status bar, and the two things a player still
 * wants from it during a long session are how much battery is left and how
 * late it is.
 *
 * Ticks once a minute. The clock only shows minutes, and a battery percentage
 * that moved in under a minute would be a bigger problem than the readout.
 */
@Composable
private fun StatusStrip(modifier: Modifier = Modifier) {
    val context = LocalContext.current
    var battery by remember { mutableIntStateOf(-1) }
    var clock by remember { mutableStateOf("") }

    LaunchedEffect(Unit) {
        val formatter = java.text.SimpleDateFormat("HH:mm", java.util.Locale.getDefault())
        while (true) {
            battery = context.getSystemService(BatteryManager::class.java)
                ?.getIntProperty(BatteryManager.BATTERY_PROPERTY_CAPACITY)
                ?: -1
            clock = formatter.format(java.util.Date())
            // Wake on the minute rather than every 60 s from whenever this
            // started, or the clock changes a random number of seconds after
            // the minute it is showing.
            kotlinx.coroutines.delay(60_000L - System.currentTimeMillis() % 60_000L)
        }
    }

    Row(
        modifier = modifier
            .fillMaxWidth()
            .padding(horizontal = 12.dp, vertical = 4.dp),
        horizontalArrangement = Arrangement.SpaceBetween,
    ) {
        Text(
            text = if (battery >= 0) "$battery%" else "",
            color = PineGlowMist.copy(alpha = 0.45f),
            fontSize = 11.sp,
        )
        Text(
            text = clock,
            color = PineGlowMist.copy(alpha = 0.45f),
            fontSize = 11.sp,
        )
    }
}
