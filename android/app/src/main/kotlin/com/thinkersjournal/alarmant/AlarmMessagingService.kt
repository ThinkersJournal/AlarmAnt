package com.thinkersjournal.alarmant

import android.content.Intent
import androidx.core.content.ContextCompat
import com.google.firebase.messaging.FirebaseMessagingService
import com.google.firebase.messaging.RemoteMessage

/**
 * Receives the A2 data push (priority HIGH, data only). Nothing in the message text is parsed:
 * only the three named fields are read (PATENT-SWEEP constraint 8).
 */
class AlarmMessagingService : FirebaseMessagingService() {

    override fun onMessageReceived(message: RemoteMessage) {
        val alertId = message.data["alert_id"] ?: return
        if (!AlertLog.firstSight(this, alertId)) return
        val intent = Intent(this, AlarmService::class.java)
            .setAction(AlarmService.ACTION_START)
            .putExtra(AlarmService.EXTRA_ALERT_ID, alertId)
            .putExtra(AlarmService.EXTRA_TITLE, message.data["title"].orEmpty())
            .putExtra(AlarmService.EXTRA_BODY, message.data["body"].orEmpty())
        ContextCompat.startForegroundService(this, intent)
    }

    override fun onNewToken(token: String) {
        // Device registration with the sender is not designed yet (DESIGN-A2 open question: ack auth).
        getSharedPreferences("config", MODE_PRIVATE).edit().putString("fcm_token", token).apply()
    }
}
