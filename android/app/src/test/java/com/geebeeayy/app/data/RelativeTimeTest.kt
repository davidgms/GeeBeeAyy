package com.geebeeayy.app.data

import org.junit.Assert.assertEquals
import org.junit.Test

/** "6 days ago" in the list, instead of a date to do arithmetic against. */
class RelativeTimeTest {

    private val now = 1_700_000_000_000L
    private fun ago(millis: Long) = RelativeTime.ago(now - millis, now)

    private val minute = 60_000L
    private val hour = 60 * minute
    private val day = 24 * hour

    @Test
    fun `under a minute is just now`() {
        assertEquals("just now", ago(0))
        assertEquals("just now", ago(59_000))
    }

    @Test
    fun `minutes hours and days`() {
        assertEquals("1 minute ago", ago(minute))
        assertEquals("45 minutes ago", ago(45 * minute))
        assertEquals("1 hour ago", ago(hour))
        assertEquals("23 hours ago", ago(23 * hour))
        assertEquals("1 day ago", ago(day))
        assertEquals("6 days ago", ago(6 * day))
    }

    @Test
    fun `a week in, it counts weeks`() {
        assertEquals("1 week ago", ago(7 * day))
        assertEquals("4 weeks ago", ago(29 * day))
    }

    @Test
    fun `a month in, it counts months`() {
        assertEquals("1 month ago", ago(30 * day))
        assertEquals("11 months ago", ago(364 * day))
    }

    @Test
    fun `a year in, it counts years`() {
        assertEquals("1 year ago", ago(365 * day))
        assertEquals("2 years ago", ago(800 * day))
    }

    /**
     * A phone whose clock moved backwards - a manual change, or a timezone
     * rollback - must not print "-3 hours ago".
     */
    @Test
    fun `a timestamp in the future reads as just now`() {
        assertEquals("just now", RelativeTime.ago(now + day, now))
    }
}
