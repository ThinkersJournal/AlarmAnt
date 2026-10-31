package com.thinkersjournal.alarmant

import android.content.Context
import androidx.core.content.edit

/** Remembers which alert ids have been seen, so a repeated push does not re-arm an alarm. */
object AlertLog {
    private const val FILE = "alertlog"
    private const val SEEN = "seen_ordered"
    internal const val MAX = 200

    // ASCII unit separator: cannot appear in a printable alert id, unlike a comma or newline.
    private const val SEP = "\u001F"

    private fun prefs(c: Context) = c.getSharedPreferences(FILE, Context.MODE_PRIVATE)

    /** True the first time an id is seen, false for every repeat. */
    @Synchronized
    fun firstSight(c: Context, alertId: String): Boolean {
        // Stored oldest first: a StringSet is unordered, so it could not evict the oldest.
        val seen = prefs(c).getString(SEEN, "").orEmpty().split(SEP).filter { it.isNotEmpty() }
        val next = window(seen, alertId) ?: return false
        prefs(c).edit { putString(SEEN, next.joinToString(SEP)) }
        return true
    }

    /**
     * The window after seeing [alertId], or null if it was already in [seen]. Keeps the newest
     * [MAX] ids only: the list is a de-duplication window, not a history.
     */
    internal fun window(seen: List<String>, alertId: String): List<String>? =
        if (alertId in seen) null else (seen + alertId).takeLast(MAX)
}
