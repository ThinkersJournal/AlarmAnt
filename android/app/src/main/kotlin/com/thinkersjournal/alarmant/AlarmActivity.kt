package com.thinkersjournal.alarmant

import android.content.Intent
import android.os.Bundle
import android.view.Gravity
import android.view.ViewGroup
import android.widget.Button
import android.widget.LinearLayout
import android.widget.TextView
import androidx.activity.OnBackPressedCallback
import androidx.activity.ComponentActivity
import androidx.work.ExistingWorkPolicy
import androidx.work.NetworkType
import androidx.work.OneTimeWorkRequestBuilder
import androidx.work.Constraints
import androidx.work.WorkManager
import androidx.work.workDataOf

/** Full-screen alert. Acknowledge is the only way out: no back, no swipe, no gesture. */
class AlarmActivity : ComponentActivity() {

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val alertId = intent.getStringExtra(AlarmService.EXTRA_ALERT_ID).orEmpty()

        onBackPressedDispatcher.addCallback(this, object : OnBackPressedCallback(true) {
            override fun handleOnBackPressed() = Unit
        })

        val matchWidth = ViewGroup.LayoutParams.MATCH_PARENT
        val column = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            gravity = Gravity.CENTER
            setPadding(48, 48, 48, 48)
        }
        column.addView(TextView(this).apply {
            text = intent.getStringExtra(AlarmService.EXTRA_TITLE).orEmpty()
            textSize = 32f
            gravity = Gravity.CENTER
        })
        column.addView(TextView(this).apply {
            text = intent.getStringExtra(AlarmService.EXTRA_BODY).orEmpty()
            textSize = 20f
            gravity = Gravity.CENTER
        })
        column.addView(
            Button(this).apply {
                text = "Acknowledge"
                textSize = 24f
                setOnClickListener { acknowledge(alertId) }
            },
            LinearLayout.LayoutParams(matchWidth, ViewGroup.LayoutParams.WRAP_CONTENT),
        )
        setContentView(column)
    }

    private fun acknowledge(alertId: String) {
        // Queue the call back first so a dead network cannot lose the acknowledgement.
        val request = OneTimeWorkRequestBuilder<AckWorker>()
            .setInputData(workDataOf(AlarmService.EXTRA_ALERT_ID to alertId))
            .setConstraints(Constraints.Builder().setRequiredNetworkType(NetworkType.CONNECTED).build())
            .build()
        WorkManager.getInstance(this).enqueueUniqueWork("ack-$alertId", ExistingWorkPolicy.KEEP, request)
        startService(Intent(this, AlarmService::class.java).setAction(AlarmService.ACTION_STOP))
        finish()
    }
}
