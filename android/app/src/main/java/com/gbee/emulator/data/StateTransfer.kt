package com.gbee.emulator.data

import java.io.File

/**
 * Save states travelling with the battery saves: the names they carry in a
 * folder, the check that a file is ours, and the backup-then-replace write.
 * Only the core's own format (`GBAS` magic) is accepted; other emulators'
 * states are not parsed. Pure parts are pinned by JVM tests.
 */
internal const val STATE_MAX_BYTES = 16L shl 20

private val STATE_MAGIC = "GBAS".toByteArray(Charsets.US_ASCII)

/** The state versions `core/src/savestate.rs` restores (`6..=SAVE_VERSION`); keep in step with it. */
internal val STATE_VERSIONS = 6..11

/** Bytes [isGbeeState] needs: the magic and the little-endian version after it. */
internal const val STATE_HEADER_BYTES = 8

/**
 * True when [bytes] start with the magic `core/src/savestate.rs` writes and a
 * version this core can restore, so a state from a newer build is refused at
 * import instead of replacing a working one and failing on load.
 */
internal fun isGbeeState(bytes: ByteArray): Boolean {
    if (bytes.size < STATE_HEADER_BYTES || STATE_MAGIC.indices.any { bytes[it] != STATE_MAGIC[it] }) return false
    val version = (4 until 8).fold(0L) { acc, i -> acc or ((bytes[i].toLong() and 0xFF) shl (8 * (i - 4))) }
    return version in STATE_VERSIONS.first.toLong()..STATE_VERSIONS.last.toLong()
}

/** A state file name taken apart: the game's name, the slot, and whether it is the `.ra` sidecar. */
internal data class StateName(val base: String, val slot: Int, val ra: Boolean)

private val STATE_NAME = Regex("""^(.+)\.slot(\d+)\.(gbstate|gbra)$""", RegexOption.IGNORE_CASE)

internal fun parseStateName(name: String): StateName? =
    STATE_NAME.matchEntire(name)?.let { StateName(it.groupValues[1], it.groupValues[2].toIntOrNull() ?: return null, it.groupValues[3].equals("gbra", true)) }

/**
 * The name slot [slot] of [romFileName] gets in a folder holding [existing]:
 * the one state name there for the same game, slot and kind is reused,
 * otherwise `<rom name>.slotN.gbstate` (`.gbra` for the sidecar).
 */
internal fun stateExportName(romFileName: String, slot: Int, ra: Boolean, existing: List<String>): String {
    val wanted = normaliseSaveName(romFileName).ifEmpty { null }
    val same = if (wanted == null) emptyList() else existing.filter {
        parseStateName(it)?.let { n -> n.slot == slot && n.ra == ra && normaliseSaveName(n.base + ".gba") == wanted } == true
    }
    return same.singleOrNull() ?: (RomBytes.displayBaseName(romFileName) + ".slot$slot." + if (ra) "gbra" else "gbstate")
}

/** The slot files in [statesDir] for [key]: slot to (state, sidecar or null). */
internal fun stateSlotFiles(statesDir: File, key: String): Map<Int, Pair<File, File?>> {
    val re = Regex("""^${Regex.escape(key)}_slot(\d+)\.state$""")
    return statesDir.listFiles().orEmpty().filter { it.isFile && it.length() > 0 }.mapNotNull { f ->
        re.matchEntire(f.name)?.let { it.groupValues[1].toInt() to (f to File(statesDir, f.name.removeSuffix(".state") + ".ra").takeIf { r -> r.isFile }) }
    }.toMap().toSortedMap()
}

/** Replace [target] with [bytes]; a file already there is kept as `.bak1`. False if either write failed. */
internal fun importStateFile(target: File, bytes: ByteArray): Boolean {
    if (target.isFile && target.length() > 0) {
        val old = runCatching { target.readBytes() }.getOrNull() ?: return false
        if (!atomicWrite(File(target.parentFile, "${target.name}.bak1"), old)) return false
    }
    return atomicWrite(target, bytes)
}
