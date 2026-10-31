package com.thinkersjournal.alarmant

import android.app.NotificationManager
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.os.Bundle
import android.view.Gravity
import android.view.View
import android.view.ViewGroup
import android.widget.Button
import android.widget.LinearLayout
import android.widget.TextView
import androidx.activity.OnBackPressedCallback
import androidx.activity.ComponentActivity
import androidx.core.content.ContextCompat
import androidx.work.ExistingWorkPolicy
import androidx.work.NetworkType
import androidx.work.OneTimeWorkRequestBuilder
import androidx.work.Constraints
import androidx.work.WorkManager
import androidx.work.workDataOf
import android.os.Build
import android.view.WindowManager

/** Full-screen alert. Acknowledge is the only way out: no back, no swipe, no gesture. */
class AlarmActivity : ComponentActivity() {

    private lateinit var filterReceiver: BroadcastReceiver

    override fun onDestroy() {
        unregisterReceiver(filterReceiver)
        super.onDestroy()
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        // showWhenLocked / turnScreenOn in the manifest need API 27; API 26 takes the window flags.
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O_MR1) {
            @Suppress("DEPRECATION")
            window.addFlags(
                WindowManager.LayoutParams.FLAG_SHOW_WHEN_LOCKED or WindowManager.LayoutParams.FLAG_TURN_SCREEN_ON,
            )
        }
        val alertId = intent.getStringExtra(AlarmService.EXTRA_ALERT_ID).orEmpty()
        // Reboot recovery: the receiver could not start the service from the background; the
        // activity is visible now, so it can. Not on a recreate (savedInstanceState != null).
        if (savedInstanceState == null && intent.getBooleanExtra(AlarmService.EXTRA_RECOVER, false)) {
            ActiveAlarm.get(this)?.let { ContextCompat.startForegroundService(this, AlarmService.startIntent(this, it)) }
        }

        onBackPressedDispatcher.addCallback(this, object : OnBackPressedCallback(true) {
            override fun handleOnBackPressed() = Unit
        })

        val matchWidth = ViewGroup.LayoutParams.MATCH_PARENT
        val column = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            gravity = Gravity.CENTER
            setPadding(48, 48, 48, 48)
        }
        val silenceWarning = TextView(this).apply {
            text = TotalSilence.WARNING
            textSize = 20f
            gravity = Gravity.CENTER
        }
        column.addView(silenceWarning)
        // Total Silence can be switched on while the alarm is sounding: follow the filter live.
        val refresh = { silenceWarning.visibility = if (TotalSilence.isOn(this)) View.VISIBLE else View.GONE }
        refresh()
        filterReceiver = object : BroadcastReceiver() {
            override fun onReceive(context: Context, intent: Intent) = refresh()
        }
        ContextCompat.registerReceiver(
            this, filterReceiver, IntentFilter(NotificationManager.ACTION_INTERRUPTION_FILTER_CHANGED),
            ContextCompat.RECEIVER_NOT_EXPORTED,
        )
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
