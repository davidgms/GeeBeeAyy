package com.geebeeayy.app.data

import java.util.concurrent.TimeUnit

/**
 * "6 days ago" rather than "3 Sep, 21:41".
 *
 * A date has to be read by doing arithmetic against today; an age is read at a
 * glance, which is the only thing this line is for. The exact timestamp is
 * still one long press away in the info dialog.
 */
object RelativeTime {

    /**
     * How long ago [thenMillis] was, relative to [nowMillis].
     *
     * Deliberately coarse and deliberately not localised beyond English: this
     * is one subtitle in a list, and `DateUtils.getRelativeTimeSpanString`
     * would drag in a `Context` for a string a test cannot then pin.
     */
    fun ago(thenMillis: Long, nowMillis: Long = System.currentTimeMillis()): String {
        val delta = nowMillis - thenMillis
        if (delta < 0) return "just now" // a clock that moved backwards, not the future
        val minutes = TimeUnit.MILLISECONDS.toMinutes(delta)
        if (minutes < 1) return "just now"
        if (minutes < 60) return plural(minutes, "minute")
        val hours = TimeUnit.MILLISECONDS.toHours(delta)
        if (hours < 24) return plural(hours, "hour")
        val days = TimeUnit.MILLISECONDS.toDays(delta)
        if (days < 7) return plural(days, "day")
        if (days < 30) return plural(days / 7, "week")
        // Capped at 11: a 30-day month runs out before a 365-day year does,
        // so days 360 to 364 would otherwise read "12 months ago" - a phrase
        // that means a year while insisting it is not one.
        if (days < 365) return plural((days / 30).coerceAtMost(11), "month")
        return plural(days / 365, "year")
    }

    private fun plural(value: Long, unit: String): String =
        if (value == 1L) "1 $unit ago" else "$value ${unit}s ago"
}
