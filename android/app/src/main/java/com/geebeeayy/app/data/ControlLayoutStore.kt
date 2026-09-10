package com.geebeeayy.app.data

import android.content.Context
import java.util.UUID

/**
 * Named touch-overlay layouts, each game's choice of which one it uses, and
 * the per-button offsets that make up each layout.
 *
 * There is always a Default layout - it cannot be deleted, and any game with
 * no explicit choice of its own uses it. A player can create further layouts,
 * rename or delete them, and pick a different one per game; picking one for
 * a game persists immediately, so a freshly downloaded game that has never
 * been opened still falls back to Default.
 *
 * Plain SharedPreferences, matching [RomFolderManager] and [DisplaySettings] -
 * a handful of small values and short lists, not a case for a real database.
 */
class ControlLayoutStore(context: Context) {
    private val prefs = context.getSharedPreferences("control_layouts", Context.MODE_PRIVATE)

    /** Every layout that exists, Default always first. */
    fun getLayouts(): List<ControlLayout> = layoutIds().map { id -> ControlLayout(id, layoutName(id)) }

    /** Which layout [gameKey] uses - Default unless the player chose another. */
    fun getLayoutForGame(gameKey: String): String =
        prefs.getString("$KEY_GAME_LAYOUT_PREFIX$gameKey", null) ?: getDefaultLayoutId()

    /**
     * The layout a game that has never been given one of its own starts with.
     *
     * Falls back to the built-in layout, and to it again if the chosen one was
     * deleted - a dangling id here would leave a game with no controls at all.
     */
    fun getDefaultLayoutId(): String {
        val saved = prefs.getString(KEY_DEFAULT_LAYOUT, null) ?: return DEFAULT_LAYOUT_ID
        return if (layoutIds().contains(saved)) saved else DEFAULT_LAYOUT_ID
    }

    fun setDefaultLayoutId(layoutId: String) {
        prefs.edit().putString(KEY_DEFAULT_LAYOUT, layoutId).apply()
    }

    fun setLayoutForGame(gameKey: String, layoutId: String) {
        prefs.edit().putString("$KEY_GAME_LAYOUT_PREFIX$gameKey", layoutId).apply()
    }

    /** A new layout, starting as a copy of [copyFrom]'s current offsets so a
     *  player nudges an existing arrangement rather than starting from the
     *  unmodified base layout every time. */
    fun createLayout(name: String, copyFrom: String = DEFAULT_LAYOUT_ID): ControlLayout {
        val id = UUID.randomUUID().toString()
        val edit = prefs.edit().putString(KEY_LAYOUT_IDS, (layoutIds() + id).joinToString(","))
        edit.putString("$KEY_NAME_PREFIX$id", name)
        ControlButton.entries.forEach { button ->
            val (x, y) = getControlOffset(copyFrom, button)
            edit.putFloat("$KEY_OFFSET_PREFIX${id}_${button.name}_x", x)
            edit.putFloat("$KEY_OFFSET_PREFIX${id}_${button.name}_y", y)
        }
        edit.apply()
        return ControlLayout(id, name)
    }

    fun renameLayout(id: String, name: String) {
        prefs.edit().putString("$KEY_NAME_PREFIX$id", name).apply()
    }

    /** No-op for Default: it always exists, so there is always a fallback. */
    fun deleteLayout(id: String) {
        if (id == DEFAULT_LAYOUT_ID) return
        val edit = prefs.edit().putString(KEY_LAYOUT_IDS, (layoutIds() - id).joinToString(","))
        edit.remove("$KEY_NAME_PREFIX$id")
        ControlButton.entries.forEach { button ->
            edit.remove("$KEY_OFFSET_PREFIX${id}_${button.name}_x")
            edit.remove("$KEY_OFFSET_PREFIX${id}_${button.name}_y")
        }
        customButtonIds(id).forEach { buttonId -> removeCustomButton(edit, id, buttonId) }
        edit.remove("$KEY_CUSTOM_IDS_PREFIX$id")
        // Any game pointed at the deleted layout falls back to Default.
        prefs.all.filterValues { it == id }.keys
            .filter { it.startsWith(KEY_GAME_LAYOUT_PREFIX) }
            .forEach { edit.remove(it) }
        edit.apply()
    }

    // ========================================================================
    // Custom buttons - combos, tap sequences and hold toggles built from the
    // real keys, positioned and dragged the same way as the real buttons but
    // keyed by a UUID instead of a [ControlButton].
    // ========================================================================

    fun getCustomButtons(layoutId: String): List<CustomButton> =
        customButtonIds(layoutId).mapNotNull { id -> loadCustomButton(layoutId, id) }

    /** Creates a new custom button, or overwrites an existing one with the
     *  same id - the one form serves both create and edit. */
    fun saveCustomButton(layoutId: String, button: CustomButton) {
        val ids = customButtonIds(layoutId)
        val edit = prefs.edit()
        if (button.id !in ids) {
            edit.putString("$KEY_CUSTOM_IDS_PREFIX$layoutId", (ids + button.id).joinToString(","))
        }
        edit.putString("$KEY_CUSTOM_PREFIX${layoutId}_${button.id}_name", button.name)
        edit.putString("$KEY_CUSTOM_PREFIX${layoutId}_${button.id}_mode", button.mode.name)
        edit.putString("$KEY_CUSTOM_PREFIX${layoutId}_${button.id}_keys", button.keys.joinToString(","))
        edit.apply()
    }

    fun deleteCustomButton(layoutId: String, buttonId: String) {
        val edit = prefs.edit()
            .putString(
                "$KEY_CUSTOM_IDS_PREFIX$layoutId",
                (customButtonIds(layoutId) - buttonId).joinToString(","),
            )
        removeCustomButton(edit, layoutId, buttonId)
        edit.apply()
    }

    /** New custom buttons spawn here, in the open, rather than at (0, 0)
     *  under the top bar - the player drags it into place right after
     *  creating it, same as a freshly created layout opens straight into
     *  the position editor. */
    fun getCustomButtonOffset(layoutId: String, buttonId: String): Pair<Float, Float> = Pair(
        prefs.getFloat("$KEY_CUSTOM_OFFSET_PREFIX${layoutId}_${buttonId}_x", DEFAULT_CUSTOM_X),
        prefs.getFloat("$KEY_CUSTOM_OFFSET_PREFIX${layoutId}_${buttonId}_y", DEFAULT_CUSTOM_Y),
    )

    fun setCustomButtonOffset(layoutId: String, buttonId: String, x: Float, y: Float) {
        prefs.edit()
            .putFloat("$KEY_CUSTOM_OFFSET_PREFIX${layoutId}_${buttonId}_x", x)
            .putFloat("$KEY_CUSTOM_OFFSET_PREFIX${layoutId}_${buttonId}_y", y)
            .apply()
    }

    private fun customButtonIds(layoutId: String): List<String> =
        prefs.getString("$KEY_CUSTOM_IDS_PREFIX$layoutId", null)
            ?.split(",")?.filter { it.isNotBlank() } ?: emptyList()

    private fun loadCustomButton(layoutId: String, buttonId: String): CustomButton? {
        val name = prefs.getString("$KEY_CUSTOM_PREFIX${layoutId}_${buttonId}_name", null) ?: return null
        val mode = prefs.getString("$KEY_CUSTOM_PREFIX${layoutId}_${buttonId}_mode", null)
            ?.let { runCatching { CustomButtonMode.valueOf(it) }.getOrNull() } ?: return null
        val keys = prefs.getString("$KEY_CUSTOM_PREFIX${layoutId}_${buttonId}_keys", "")
            .orEmpty().split(",").mapNotNull { it.toIntOrNull() }
        if (keys.isEmpty()) return null
        return CustomButton(buttonId, name, mode, keys)
    }

    private fun removeCustomButton(edit: android.content.SharedPreferences.Editor, layoutId: String, buttonId: String) {
        edit.remove("$KEY_CUSTOM_PREFIX${layoutId}_${buttonId}_name")
        edit.remove("$KEY_CUSTOM_PREFIX${layoutId}_${buttonId}_mode")
        edit.remove("$KEY_CUSTOM_PREFIX${layoutId}_${buttonId}_keys")
        edit.remove("$KEY_CUSTOM_OFFSET_PREFIX${layoutId}_${buttonId}_x")
        edit.remove("$KEY_CUSTOM_OFFSET_PREFIX${layoutId}_${buttonId}_y")
    }

    fun getControlOffset(layoutId: String, button: ControlButton): Pair<Float, Float> = Pair(
        prefs.getFloat("$KEY_OFFSET_PREFIX${layoutId}_${button.name}_x", 0f),
        prefs.getFloat("$KEY_OFFSET_PREFIX${layoutId}_${button.name}_y", 0f),
    )

    fun setControlOffset(layoutId: String, button: ControlButton, x: Float, y: Float) {
        prefs.edit()
            .putFloat("$KEY_OFFSET_PREFIX${layoutId}_${button.name}_x", x)
            .putFloat("$KEY_OFFSET_PREFIX${layoutId}_${button.name}_y", y)
            .apply()
    }

    /**
     * How much bigger or smaller than default one control is drawn, per
     * layout.
     *
     * The whole block already has a global size slider in Settings. This is
     * the other half: a thumb that wants a big D-pad usually wants the same
     * Start and Select it always had, and one multiplier over everything
     * cannot say that.
     */
    fun getControlScale(layoutId: String, button: ControlButton): Float =
        prefs.getFloat("$KEY_SCALE_PREFIX${layoutId}_${button.name}", 1f)
            .coerceIn(MIN_CONTROL_SCALE, MAX_CONTROL_SCALE)

    fun setControlScale(layoutId: String, button: ControlButton, scale: Float) {
        prefs.edit()
            .putFloat(
                "$KEY_SCALE_PREFIX${layoutId}_${button.name}",
                scale.coerceIn(MIN_CONTROL_SCALE, MAX_CONTROL_SCALE),
            )
            .apply()
    }

    fun getCustomButtonScale(layoutId: String, buttonId: String): Float =
        prefs.getFloat("$KEY_CUSTOM_SCALE_PREFIX${layoutId}_$buttonId", 1f)
            .coerceIn(MIN_CONTROL_SCALE, MAX_CONTROL_SCALE)

    fun setCustomButtonScale(layoutId: String, buttonId: String, scale: Float) {
        prefs.edit()
            .putFloat(
                "$KEY_CUSTOM_SCALE_PREFIX${layoutId}_$buttonId",
                scale.coerceIn(MIN_CONTROL_SCALE, MAX_CONTROL_SCALE),
            )
            .apply()
    }

    /** Put every button of [layoutId] back at its base position. */
    fun resetLayoutOffsets(layoutId: String) {
        val edit = prefs.edit()
        ControlButton.entries.forEach { button ->
            edit.remove("$KEY_OFFSET_PREFIX${layoutId}_${button.name}_x")
            edit.remove("$KEY_OFFSET_PREFIX${layoutId}_${button.name}_y")
        }
        edit.apply()
        // Sizes are part of a layout too, so Reset has to put them back as
        // well - otherwise Reset leaves a giant D-pad in its default spot.
        prefs.edit().apply {
            ControlButton.entries.forEach { remove("$KEY_SCALE_PREFIX${layoutId}_${it.name}") }
            customButtonIds(layoutId).forEach {
                remove("$KEY_CUSTOM_SCALE_PREFIX${layoutId}_$it")
            }
        }.apply()
    }

    private fun layoutIds(): List<String> {
        val stored = prefs.getString(KEY_LAYOUT_IDS, null)?.split(",")?.filter { it.isNotBlank() }
            ?: emptyList()
        return if (DEFAULT_LAYOUT_ID in stored) stored else listOf(DEFAULT_LAYOUT_ID) + stored
    }

    private fun layoutName(id: String): String =
        prefs.getString("$KEY_NAME_PREFIX$id", null) ?: if (id == DEFAULT_LAYOUT_ID) "Default" else id

    companion object {
        const val DEFAULT_LAYOUT_ID = "default"
        private const val KEY_LAYOUT_IDS = "layout_ids"
        private const val KEY_DEFAULT_LAYOUT = "default_layout"
        private const val KEY_NAME_PREFIX = "layout_name_"
        private const val KEY_OFFSET_PREFIX = "offset_"
        private const val KEY_SCALE_PREFIX = "scale_"
        private const val KEY_CUSTOM_SCALE_PREFIX = "custom_scale_"

        /** How far a single control may be shrunk or grown, as a multiplier. */
        const val MIN_CONTROL_SCALE = 0.6f
        const val MAX_CONTROL_SCALE = 1.8f

        /** One press of the smaller/bigger buttons in the layout editor. */
        const val CONTROL_SCALE_STEP = 0.1f
        private const val KEY_GAME_LAYOUT_PREFIX = "game_layout_"
        private const val KEY_CUSTOM_IDS_PREFIX = "custom_ids_"
        private const val KEY_CUSTOM_PREFIX = "custom_"
        private const val KEY_CUSTOM_OFFSET_PREFIX = "custom_offset_"
        private const val DEFAULT_CUSTOM_X = 120f
        private const val DEFAULT_CUSTOM_Y = 500f
    }
}
