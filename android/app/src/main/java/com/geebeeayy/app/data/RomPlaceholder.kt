package com.geebeeayy.app.data

/**
 * The tile a ROM gets when the player supplied no cover art.
 *
 * Almost nobody has a picture sitting next to their ROMs, so in practice the
 * list was the same grey cartridge icon repeated down the screen - which is
 * worse than either having art or not having a thumbnail at all. The game's
 * own initials over a colour taken from its name give every row something to
 * recognise, and it is the same tile every time because it comes from the
 * name rather than from play.
 *
 * Pure functions with no Android types on purpose: this is the part with the
 * arithmetic, and `RomPlaceholderTest` pins it.
 */
object RomPlaceholder {

    /**
     * Up to two initials for [name].
     *
     * Words that are only decoration in a ROM dump - a region tag, a revision,
     * a dump-group suffix - are skipped, so "Mario Tennis Advance (U) [!]"
     * gives MT rather than MA.
     */
    fun initials(name: String): String {
        val words = name
            .split(' ', '-', '_', '.')
            .map { it.trim() }
            .filter { it.isNotEmpty() && it.first().isLetterOrDigit() }
            .filterNot { it.first() == '(' || it.first() == '[' }
        val letters = words.mapNotNull { word -> word.firstOrNull { it.isLetterOrDigit() } }
        return when {
            letters.isEmpty() -> "?"
            letters.size == 1 -> letters.first().uppercaseChar().toString()
            else -> "${letters[0].uppercaseChar()}${letters[1].uppercaseChar()}"
        }
    }

    /**
     * A hue in 0..359 for [name], stable across runs.
     *
     * Not `String.hashCode()`: it is stable in practice but not contractually,
     * and a tile whose colour changed after a platform update would look like
     * a bug. This is an explicit FNV-1a over the name's code points.
     */
    fun hue(name: String): Int {
        var hash = 2166136261u
        for (ch in name.lowercase()) {
            hash = hash xor ch.code.toUInt()
            hash *= 16777619u
        }
        return (hash % 360u).toInt()
    }
}
