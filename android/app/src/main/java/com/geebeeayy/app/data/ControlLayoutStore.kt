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
        prefs.getString("$KEY_GAME_LAYOUT_PREFIX$gameKey", null) ?: DEFAULT_LAYOUT_ID

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
        // Any game pointed at the deleted layout falls back to Default.
        prefs.all.filterValues { it == id }.keys
            .filter { it.startsWith(KEY_GAME_LAYOUT_PREFIX) }
            .forEach { edit.remove(it) }
        edit.apply()
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

    /** Put every button of [layoutId] back at its base position. */
    fun resetLayoutOffsets(layoutId: String) {
        val edit = prefs.edit()
        ControlButton.entries.forEach { button ->
            edit.remove("$KEY_OFFSET_PREFIX${layoutId}_${button.name}_x")
            edit.remove("$KEY_OFFSET_PREFIX${layoutId}_${button.name}_y")
        }
        edit.apply()
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
        private const val KEY_NAME_PREFIX = "layout_name_"
        private const val KEY_OFFSET_PREFIX = "offset_"
        private const val KEY_GAME_LAYOUT_PREFIX = "game_layout_"
    }
}
