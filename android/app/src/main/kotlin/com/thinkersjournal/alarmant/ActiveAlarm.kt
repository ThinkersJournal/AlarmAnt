package com.thinkersjournal.alarmant

import android.content.Context
import androidx.core.content.edit

/** The alarm that is sounding now, kept on disk so a reboot can bring it back. Cleared only by Acknowledge. */
object ActiveAlarm {
    data class Alert(val id: String, val title: String, val body: String)

    private const val FILE = "activealarm"

    private fun prefs(c: Context) = c.getSharedPreferences(FILE, Context.MODE_PRIVATE)

    fun set(c: Context, alert: Alert) {
        // commit(), not apply(): a reboot or kill right after the push must not lose it.
        prefs(c).edit(commit = true) {
            putString("id", alert.id).putString("title", alert.title).putString("body", alert.body)
        }
    }

    fun get(c: Context): Alert? {
        val p = prefs(c)
        val id = p.getString("id", null) ?: return null
        return Alert(id, p.getString("title", "").orEmpty(), p.getString("body", "").orEmpty())
    }

    fun clear(c: Context) {
        prefs(c).edit(commit = true) { clear() }
    }
}
