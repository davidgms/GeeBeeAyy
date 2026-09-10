package com.geebeeayy.app.ui

import android.content.Context
import android.os.Build
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
     * Buzz once, unless [strength] is off or the phone has no vibrator.
     *
     * Amplitude is only honoured on hardware that can vary it; elsewhere
     * `createOneShot` falls back to the one intensity the motor has, and the
     * duration still separates a light tap from a strong one.
     */
    fun press(strength: HapticStrength) {
        if (strength == HapticStrength.OFF) return
        val device = vibrator ?: return
        runCatching {
            device.vibrate(
                VibrationEffect.createOneShot(strength.millis, strength.amplitude)
            )
        }
    }
}
