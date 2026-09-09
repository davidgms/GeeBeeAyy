package com.geebeeayy.app.ui

import android.app.Activity
import android.content.Context
import android.content.ContextWrapper
import android.content.pm.ActivityInfo

/**
 * The orientation this activity asks for, given the Force Portrait setting.
 *
 * `requestedOrientation` only ever governs this activity's own window, so the
 * lock has always been app-only - it cannot and does not change the phone's
 * system rotation setting.
 *
 * What it did get wrong is the *off* case. `SCREEN_ORIENTATION_UNSPECIFIED`
 * hands the decision back to the system, and the system honours the phone's
 * auto-rotate lock: with auto-rotate off - which is how most phones sit - the
 * app stayed portrait and turning the setting off appeared to do nothing.
 * [ActivityInfo.SCREEN_ORIENTATION_FULL_SENSOR] rotates on the sensor alone,
 * so the app follows the phone in the hand whatever the system lock says.
 */
fun orientationFor(forcePortrait: Boolean): Int =
    if (forcePortrait) {
        ActivityInfo.SCREEN_ORIENTATION_PORTRAIT
    } else {
        ActivityInfo.SCREEN_ORIENTATION_FULL_SENSOR
    }

/**
 * The [Activity] behind a Compose `LocalContext`.
 *
 * `LocalContext.current as? Activity` is not reliable: Compose hands out a
 * `ContextThemeWrapper` in some hosts, and the cast then silently returns null
 * - which is a setting that saves itself and never applies.
 */
fun Context.findActivity(): Activity? {
    var context: Context? = this
    while (context is ContextWrapper) {
        if (context is Activity) return context
        context = context.baseContext
    }
    return null
}
