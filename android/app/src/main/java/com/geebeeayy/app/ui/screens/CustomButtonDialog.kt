package com.geebeeayy.app.ui.screens

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Edit
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.geebeeayy.app.data.CustomButton
import com.geebeeayy.app.data.CustomButtonMode
import com.geebeeayy.app.engine.GbaEngine
import com.geebeeayy.app.ui.theme.*
import java.util.UUID

/** name, GBA key, in the order the picker shows them. */
private val PICKABLE_KEYS = listOf(
    "Up" to GbaEngine.KEY_UP,
    "Down" to GbaEngine.KEY_DOWN,
    "Left" to GbaEngine.KEY_LEFT,
    "Right" to GbaEngine.KEY_RIGHT,
    "L" to GbaEngine.KEY_L,
    "R" to GbaEngine.KEY_R,
    "A" to GbaEngine.KEY_A,
    "B" to GbaEngine.KEY_B,
    "Start" to GbaEngine.KEY_START,
    "Select" to GbaEngine.KEY_SELECT,
)

private fun keyLabel(key: Int): String = PICKABLE_KEYS.firstOrNull { it.second == key }?.first ?: "?"

private val CustomButtonMode.label: String
    get() = when (this) {
        CustomButtonMode.COMBO -> "Combo"
        CustomButtonMode.SEQUENCE -> "Sequence"
        CustomButtonMode.TOGGLE_HOLD -> "Toggle hold"
    }

private val CustomButtonMode.description: String
    get() = when (this) {
        CustomButtonMode.COMBO -> "Presses every picked key together, held while you hold this one"
        CustomButtonMode.SEQUENCE -> "Taps each picked key in order, once each - pick the same key twice for a double press"
        CustomButtonMode.TOGGLE_HOLD -> "Tap to start holding every picked key; tap again to let go"
    }

/**
 * Lists a layout's custom buttons and manages them - create, edit, delete.
 * Positioning and dragging them happens on the layout canvas itself, the
 * same as the real buttons; this dialog only owns what each one is.
 */
@Composable
fun CustomButtonsListDialog(
    buttons: List<CustomButton>,
    onSave: (CustomButton) -> Unit,
    onDelete: (CustomButton) -> Unit,
    onDismiss: () -> Unit,
) {
    var editing by remember { mutableStateOf<CustomButton?>(null) }
    var creating by remember { mutableStateOf(false) }

    AlertDialog(
        onDismissRequest = onDismiss,
        containerColor = HoneyDark,
        titleContentColor = GoldenSaplight,
        textContentColor = PineGlowMist,
        title = { Text("Custom Buttons", fontWeight = FontWeight.Bold) },
        text = {
            Column(modifier = Modifier.verticalScroll(rememberScrollState())) {
                if (buttons.isEmpty()) {
                    Text(
                        "No custom buttons yet in this layout.",
                        color = PineGlowMist.copy(alpha = 0.7f),
                        fontSize = 13.sp,
                    )
                }
                buttons.forEach { button ->
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Column(modifier = Modifier.weight(1f)) {
                            Text(button.name, color = PineGlowMist, fontSize = 14.sp, fontWeight = FontWeight.SemiBold)
                            Text(
                                "${button.mode.label} · ${button.keys.joinToString(" ") { keyLabel(it) }}",
                                color = AmberResin,
                                fontSize = 11.sp,
                            )
                        }
                        IconButton(onClick = { editing = button }) {
                            Icon(Icons.Default.Edit, "Edit", tint = AmberResin)
                        }
                        IconButton(onClick = { onDelete(button) }) {
                            Icon(Icons.Default.Delete, "Delete", tint = AmberResin)
                        }
                    }
                }
            }
        },
        confirmButton = {
            TextButton(onClick = { creating = true }) { Text("+ New Button", color = GoldenSaplight) }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) { Text("Close", color = PineGlowMist) }
        },
    )

    editing?.let { button ->
        CustomButtonEditDialog(
            existing = button,
            onSave = { saved -> onSave(saved); editing = null },
            onDismiss = { editing = null },
        )
    }

    if (creating) {
        CustomButtonEditDialog(
            existing = null,
            onSave = { saved -> onSave(saved); creating = false },
            onDismiss = { creating = false },
        )
    }
}

/**
 * The form for one custom button: a name, a behaviour, and which keys it
 * drives. Serves both create ([existing] null) and edit.
 */
@Composable
private fun CustomButtonEditDialog(
    existing: CustomButton?,
    onSave: (CustomButton) -> Unit,
    onDismiss: () -> Unit,
) {
    var name by remember { mutableStateOf(existing?.name ?: "") }
    var mode by remember { mutableStateOf(existing?.mode ?: CustomButtonMode.COMBO) }
    var keys by remember { mutableStateOf(existing?.keys ?: emptyList()) }

    fun tapKey(key: Int) {
        keys = when (mode) {
            // A sequence is order- and repeat-sensitive - tapping always
            // appends. A combo or a hold toggle is just a set of keys.
            CustomButtonMode.SEQUENCE -> keys + key
            else -> if (key in keys) keys - key else keys + key
        }
    }

    val canSave = name.isNotBlank() && keys.isNotEmpty()

    AlertDialog(
        onDismissRequest = onDismiss,
        containerColor = HoneyDark,
        titleContentColor = GoldenSaplight,
        textContentColor = PineGlowMist,
        title = { Text(if (existing == null) "New custom button" else "Edit custom button") },
        text = {
            Column(
                modifier = Modifier.verticalScroll(rememberScrollState()),
                verticalArrangement = Arrangement.spacedBy(10.dp),
            ) {
                OutlinedTextField(
                    value = name,
                    onValueChange = { name = it },
                    singleLine = true,
                    label = { Text("Name") },
                    colors = OutlinedTextFieldDefaults.colors(
                        focusedTextColor = PineGlowMist,
                        unfocusedTextColor = PineGlowMist,
                        focusedBorderColor = AmberResin,
                        unfocusedBorderColor = AmberResin.copy(alpha = 0.5f),
                        cursorColor = AmberResin,
                    ),
                )

                Column {
                    Text("Behaviour", color = AmberResin, fontSize = 12.sp)
                    CustomButtonMode.entries.forEach { m ->
                        Row(
                            verticalAlignment = Alignment.CenterVertically,
                            modifier = Modifier
                                .fillMaxWidth()
                                .clickable {
                                    if (mode != m) {
                                        mode = m
                                        // A sequence's repeats and a set's
                                        // membership do not mean the same
                                        // thing - switching modes clears the
                                        // pick rather than carry over a list
                                        // that would misread as the other.
                                        keys = emptyList()
                                    }
                                },
                        ) {
                            RadioButton(
                                selected = mode == m,
                                onClick = {
                                    if (mode != m) {
                                        // Only a move across the SEQUENCE
                                        // boundary invalidates the picks:
                                        // SEQUENCE keeps an ordered list that
                                        // may repeat a key, the other two an
                                        // unordered set. Clearing on every
                                        // change meant editing a button just
                                        // to switch COMBO to TOGGLE_HOLD
                                        // silently dropped its keys and left
                                        // Save disabled.
                                        val crossesSequence =
                                            (mode == CustomButtonMode.SEQUENCE) !=
                                                (m == CustomButtonMode.SEQUENCE)
                                        mode = m
                                        if (crossesSequence) {
                                            keys = emptyList()
                                        }
                                    }
                                },
                                colors = RadioButtonDefaults.colors(
                                    selectedColor = GoldenSaplight,
                                    unselectedColor = AmberResin,
                                ),
                            )
                            Column {
                                Text(m.label, color = PineGlowMist, fontSize = 13.sp)
                                Text(m.description, color = PineGlowMist.copy(alpha = 0.6f), fontSize = 10.sp)
                            }
                        }
                    }
                }

                Column {
                    Text(
                        if (mode == CustomButtonMode.SEQUENCE) "Tap keys in order" else "Keys",
                        color = AmberResin,
                        fontSize = 12.sp,
                    )
                    Spacer(modifier = Modifier.height(4.dp))
                    KeyPickerGrid(
                        selected = keys.toSet(),
                        highlightSelection = mode != CustomButtonMode.SEQUENCE,
                        onTap = ::tapKey,
                    )
                }

                if (mode == CustomButtonMode.SEQUENCE) {
                    Text(
                        text = if (keys.isEmpty()) "Sequence: (empty)" else "Sequence: ${keys.joinToString(" → ") { keyLabel(it) }}",
                        color = PineGlowMist,
                        fontSize = 12.sp,
                    )
                    Row {
                        TextButton(onClick = { keys = keys.dropLast(1) }, enabled = keys.isNotEmpty()) {
                            Text("Undo last", color = AmberResin, fontSize = 12.sp)
                        }
                        TextButton(onClick = { keys = emptyList() }, enabled = keys.isNotEmpty()) {
                            Text("Clear", color = AmberResin, fontSize = 12.sp)
                        }
                    }
                }
            }
        },
        confirmButton = {
            TextButton(
                enabled = canSave,
                onClick = {
                    onSave(
                        CustomButton(
                            id = existing?.id ?: UUID.randomUUID().toString(),
                            name = name.trim(),
                            mode = mode,
                            keys = keys,
                        )
                    )
                },
            ) { Text("Save", color = if (canSave) GoldenSaplight else PineGlowMist.copy(alpha = 0.4f)) }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) { Text("Cancel", color = PineGlowMist) }
        },
    )
}

/** The ten real keys as tappable chips, two rows so it works without a
 *  flow-layout dependency. */
@Composable
private fun KeyPickerGrid(
    selected: Set<Int>,
    highlightSelection: Boolean,
    onTap: (Int) -> Unit,
) {
    Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
        PICKABLE_KEYS.chunked(5).forEach { row ->
            Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                row.forEach { (label, key) ->
                    val isSelected = highlightSelection && key in selected
                    Box(
                        modifier = Modifier
                            .clip(RoundedCornerShape(8.dp))
                            .background(if (isSelected) GoldenSaplight else BurntRoot)
                            .clickable { onTap(key) }
                            .padding(horizontal = 10.dp, vertical = 8.dp),
                        contentAlignment = Alignment.Center,
                    ) {
                        Text(
                            label,
                            color = if (isSelected) BurntRoot else PineGlowMist,
                            fontSize = 12.sp,
                            fontWeight = FontWeight.SemiBold,
                        )
                    }
                }
            }
        }
    }
}
