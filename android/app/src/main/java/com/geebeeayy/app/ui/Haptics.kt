package com.geebeeayy.app.ui

import android.content.Context
import android.os.Build
import android.os.SystemClock
import android.os.VibrationAttributes
import android.os.VibrationEffect
import android.os.Vibrator
import android.os.VibratorManager
import com.geebeeayy.app.data.HapticStrength

/**
 * The one-shot buzz that answers a button press.
 *
 * Held once and reused: fetching the system service costs a binder call, and
 * this fires on every press of every control.
 */
class Haptics(context: Context) {

    private val vibrator: Vibrator? = runCatching {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            context.getSystemService(VibratorManager::class.java)?.defaultVibrator
        } else {
            @Suppress("DEPRECATION")
            context.getSystemService(Vibrator::class.java)
        }
    }.getOrNull()?.takeIf { it.hasVibrator() }

    /**
     * Tells the system this is touch feedback rather than a notification.
     *
     * Without it the vibration is filed under `USAGE_UNKNOWN`, and the
     * system then judges it by the wrong intensity setting and silences it
     * in modes where touch feedback should still be felt - which reads as
     * the Vibration setting being a dead switch.
     */
    private val attributes = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
        VibrationAttributes.createForUsage(VibrationAttributes.USAGE_TOUCH)
    } else {
        null
    }

    private var lastBuzzMs = 0L

    /**
     * Buzz once, unless [strength] is off or the phone has no vibrator.
     *
     * Amplitude is only honoured on hardware that can vary it; elsewhere
     * `createOneShot` falls back to the one intensity the motor has, and the
     * duration still separates a light tap from a strong one.
     */
    fun press(strength: HapticStrength) {
        if (strength == HapticStrength.OFF) return
        val device = vibrator ?: return
        // One press is one buzz, however many keys it carries. A COMBO button
        // changes three keys in the same frame and the D-pad re-evaluates its
        // set on every pointer sample, so a thumb resting on a sector
        // boundary would otherwise retrigger the motor at touch-sampling
        // rate. Each `vibrate` cancels the effect still running, so that is
        // heard as one continuous buzz, not as separate taps.
        val now = SystemClock.uptimeMillis()
        if (now - lastBuzzMs < MIN_INTERVAL_MS) return
        lastBuzzMs = now
        runCatching {
            val effect = VibrationEffect.createOneShot(strength.millis, strength.amplitude)
            if (attributes != null) {
                device.vibrate(effect, attributes)
            } else {
                device.vibrate(effect)
            }
        }
    }

    private companion object {
        /**
         * Shortest gap between two buzzes. Long enough to swallow the extra
         * keys of one gesture, short enough that deliberate mashing still
         * answers every press.
         */
        const val MIN_INTERVAL_MS = 40L
    }
}
