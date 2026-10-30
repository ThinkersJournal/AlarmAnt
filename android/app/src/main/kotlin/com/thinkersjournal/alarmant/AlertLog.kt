package com.thinkersjournal.alarmant

import android.content.Context

/** Remembers which alert ids have been seen, so a repeated push does not re-arm an alarm. */
object AlertLog {
    private const val FILE = "alertlog"
    private const val SEEN = "seen_ordered"

    // ASCII unit separator: cannot appear in a printable alert id, unlike a comma or newline.
    private const val SEP = "\u001F"

    private fun prefs(c: Context) = c.getSharedPreferences(FILE, Context.MODE_PRIVATE)

    /** True the first time an id is seen, false for every repeat. */
    @Synchronized
    fun firstSight(c: Context, alertId: String): Boolean {
        // Stored oldest first: a StringSet is unordered, so it could not evict the oldest.
        val seen = prefs(c).getString(SEEN, "").orEmpty().split(SEP).filter { it.isNotEmpty() }
        if (alertId in seen) return false
        // Keep the newest 200 ids only; the list is a de-duplication window, not a history.
        val next = (seen + alertId).takeLast(200).joinToString(SEP)
        prefs(c).edit().putString(SEEN, next).apply()
        return true
    }
}
