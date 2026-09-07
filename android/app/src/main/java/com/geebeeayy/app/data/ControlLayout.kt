package com.geebeeayy.app.data

/** A named, saved arrangement of the touch overlay. */
data class ControlLayout(val id: String, val name: String) {
    val isDefault: Boolean get() = id == ControlLayoutStore.DEFAULT_LAYOUT_ID
}
