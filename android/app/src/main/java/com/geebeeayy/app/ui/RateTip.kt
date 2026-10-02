package com.geebeeayy.app.ui

/**
 * When to tell the player that the phone lowered the screen rate.
 *
 * MIUI's smart refresh can trap a game at 50 Hz, a sixth of the frames never
 * shown, and no app-side request gets past it. Pressing Home and coming back
 * resets it (`docs/research/miui-smart-refresh.html`, and
 * `docs/research/emulators-research.md`: no emulator has a better answer).
 */
object RateTip {

    /** Below this the panel cannot show every GBA frame. */
    private const val LOW_HZ = 58f

    /**
     * How long the rate must stay low first. Coming back from Home, MIUI walks
     * 144, 120, 60 over about two seconds; that walk must not trigger it.
     */
    const val SETTLE_MS = 5_000L

    fun shouldShow(hz: Float, enabled: Boolean, alreadyShown: Boolean, lowForMs: Long): Boolean =
        enabled && !alreadyShown && hz in 1f..LOW_HZ && lowForMs >= SETTLE_MS

    /** Redmi and POCO phones report Xiaomi as the manufacturer too. */
    fun defaultEnabled(manufacturer: String): Boolean = manufacturer.equals("Xiaomi", ignoreCase = true)

    /** Once per app run, so the tip informs without nagging. */
    @Volatile var shownThisRun = false

    const val MESSAGE = "Your phone lowered the screen to %d Hz. Press Home and come back for smooth 60 Hz."
}
