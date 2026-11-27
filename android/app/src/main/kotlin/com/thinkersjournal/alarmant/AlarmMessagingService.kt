// SPDX-License-Identifier: MIT OR Apache-2.0
package com.thinkersjournal.alarmant

import android.content.Intent
import androidx.core.content.ContextCompat
import com.google.firebase.messaging.FirebaseMessagingService
import com.google.firebase.messaging.RemoteMessage
import androidx.core.content.edit
import androidx.work.Constraints
import androidx.work.ExistingWorkPolicy
import androidx.work.NetworkType
import androidx.work.OneTimeWorkRequestBuilder
import androidx.work.WorkManager

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

    // firebase-messaging 26.0.1 marks this deprecated without naming a replacement in the class.
    @Suppress("OVERRIDE_DEPRECATION")
    override fun onNewToken(token: String) {
        getSharedPreferences("config", MODE_PRIVATE).edit { putString("fcm_token", token) }
        // Once registered, tell the sender the new token (DESIGN-REGISTRATION section 4); latest token wins.
        if (Registration.isRegistered(this)) {
            WorkManager.getInstance(this).enqueueUniqueWork(
                "fcm-token",
                ExistingWorkPolicy.REPLACE,
                OneTimeWorkRequestBuilder<TokenWorker>()
                    .setConstraints(Constraints.Builder().setRequiredNetworkType(NetworkType.CONNECTED).build())
                    .build(),
            )
        }
    }
}
