package com.thinkersjournal.alarmant

import android.app.AlarmManager
import android.app.PendingIntent
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.os.Build
import androidx.core.content.ContextCompat

/**
 * While an alarm is unacknowledged, wakes every [PERIOD_MS] and restarts the service if its process
 * has died. Without it Android waits about 30 s before restarting a killed service (measured), and an
 * AlarmManager alarm survives the process that set it. Runs only while an alarm is active.
 */
object AlarmWatchdog {
    const val PERIOD_MS = 5_000L

    private fun pending(c: Context) = PendingIntent.getBroadcast(
        c, 0, Intent(c, WatchdogReceiver::class.java),
        PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
    )

    fun schedule(c: Context) {
        val am = c.getSystemService(AlarmManager::class.java)
        val at = System.currentTimeMillis() + PERIOD_MS
        // Exact alarms need the user's grant from API 31 (SetupActivity asks); without it fall back
        // to the inexact form, which Doze may delay by minutes.
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.S || am.canScheduleExactAlarms()) {
            am.setExactAndAllowWhileIdle(AlarmManager.RTC_WAKEUP, at, pending(c))
        } else {
            am.setAndAllowWhileIdle(AlarmManager.RTC_WAKEUP, at, pending(c))
        }
    }

    fun cancel(c: Context) {
        c.getSystemService(AlarmManager::class.java).cancel(pending(c))
    }
}

class WatchdogReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        val alert = ActiveAlarm.get(context) ?: return
        AlarmWatchdog.schedule(context)
        // playingId is null in a fresh process: the service died (or never came back).
        if (AlarmService.playingId == null) {
            ContextCompat.startForegroundService(context, AlarmService.startIntent(context, alert))
        }
    }
}
