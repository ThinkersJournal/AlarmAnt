package com.thinkersjournal.alarmant

import android.app.NotificationManager
import android.content.Context

/**
 * Total Silence mutes the alarm stream and no app can bypass it, with or without policy access
 * (measured on API 36: player started, mutedState opPlayAudio). Reading the filter needs no grant.
 */
object TotalSilence {
    const val WARNING = "Do Not Disturb is set to Total Silence: the alarm will make NO sound. Switch it to Alarms only."

    fun isOn(c: Context): Boolean =
        c.getSystemService(NotificationManager::class.java).currentInterruptionFilter ==
            NotificationManager.INTERRUPTION_FILTER_NONE
}
