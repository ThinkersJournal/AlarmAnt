package com.thinkersjournal.alarmant

import android.content.Context

/** Remembers which alert ids have been seen, so a repeated push does not re-arm an alarm. */
object AlertLog {
    private const val FILE = "alertlog"
    private const val SEEN = "seen"

    private fun prefs(c: Context) = c.getSharedPreferences(FILE, Context.MODE_PRIVATE)

    /** True the first time an id is seen, false for every repeat. */
    @Synchronized
    fun firstSight(c: Context, alertId: String): Boolean {
        val seen = prefs(c).getStringSet(SEEN, emptySet()).orEmpty()
        if (alertId in seen) return false
        // Keep the newest 200 ids only; the set is a de-duplication window, not a history.
        val next = (seen + alertId).toList().takeLast(200).toSet()
        prefs(c).edit().putStringSet(SEEN, next).apply()
        return true
    }
}
