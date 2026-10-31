package com.thinkersjournal.alarmant

import android.app.NotificationManager
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import androidx.core.content.ContextCompat

/**
 * Brings an unacknowledged alarm back after a reboot. Android 15 bars a BOOT_COMPLETED receiver
 * from starting a mediaPlayback foreground service, so on refusal the receiver raises the
 * full-screen notification instead and [AlarmActivity] starts the service once it is visible.
 */
class BootReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        if (intent.action != Intent.ACTION_BOOT_COMPLETED) return
        val alert = ActiveAlarm.get(context) ?: return
        try {
            ContextCompat.startForegroundService(context, AlarmService.startIntent(context, alert))
        } catch (e: RuntimeException) {
            // ForegroundServiceStartNotAllowedException is an IllegalStateException on API 31+.
            AlarmService.ensureChannel(context)
            context.getSystemService(NotificationManager::class.java).notify(
                AlarmService.NOTIFICATION_ID,
                AlarmService.notification(context, alert, recover = true),
            )
        }
    }
}
