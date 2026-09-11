package com.geebeeayy.app.ui

import android.app.Activity
import android.content.Context
import android.content.ContextWrapper
import android.content.pm.ActivityInfo
import com.geebeeayy.app.data.ScreenOrientation

/**
 * The orientation this activity asks for.
 *
 * `requestedOrientation` only ever governs this activity's own window, so the
 * lock has always been app-only - it cannot and does not change the phone's
 * system rotation setting.
 *
 * [ScreenOrientation.AUTO] is `SCREEN_ORIENTATION_FULL_SENSOR` and not
 * `UNSPECIFIED`. Unspecified hands the decision to the system, and the system
 * honours the phone's auto-rotate lock: with auto-rotate off - which is how
 * most phones sit - the app stayed upright and "automatic" appeared to do
 * nothing at all.
 */
fun orientationFor(orientation: ScreenOrientation): Int = when (orientation) {
    ScreenOrientation.AUTO -> ActivityInfo.SCREEN_ORIENTATION_FULL_SENSOR
    ScreenOrientation.PORTRAIT -> ActivityInfo.SCREEN_ORIENTATION_PORTRAIT
    // Sensor landscape, not plain landscape: a phone turned the other way
    // round should still end up the right way up.
    ScreenOrientation.LANDSCAPE -> ActivityInfo.SCREEN_ORIENTATION_SENSOR_LANDSCAPE
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
