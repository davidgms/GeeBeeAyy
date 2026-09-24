package com.geebeeayy.app.engine

/**
 * The RetroAchievements side of the house.
 *
 * Backed by `rcheevos`, the MIT-licensed C library RetroAchievements publishes
 * for emulator authors, built from the submodule at `src/main/cpp/rcheevos`.
 * It lives here rather than in the Rust core on purpose: the core has two
 * dependencies and no C, and deciding what an achievement means is not
 * emulation. See `docs/achievements.md`.
 *
 * So far this only answers *which game is this*. That needs no account and no
 * network, so it is the part that can be proven on its own.
 */
object RaEngine {

    private val available: Boolean = runCatching {
        System.loadLibrary("geebeeayy_ra")
    }.isSuccess

    /** False when the native library is missing, so callers degrade quietly. */
    fun isAvailable(): Boolean = available

    /**
     * The hash RetroAchievements identifies [romData] by.
     *
     * For Game Boy Advance this is an MD5 of the whole file, unmodified -
     * which is why a **renamed** ROM still matches and a **trimmed or
     * patched** one does not. Null when the library is missing or the data
     * cannot be hashed.
     */
    fun hashRom(romData: ByteArray): String? =
        if (available) runCatching { nativeHashRom(romData) }.getOrNull() else null

    /** The rcheevos version built into this APK, so a report can name it. */
    fun version(): String? =
        if (available) runCatching { nativeVersion() }.getOrNull() else null

    @JvmStatic
    private external fun nativeHashRom(data: ByteArray): String?

    @JvmStatic
    private external fun nativeVersion(): String?
}
