package com.thinkersjournal.alarmant

import android.Manifest
import android.app.NotificationManager
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Bundle
import android.os.PowerManager
import android.provider.Settings
import android.widget.Button
import android.widget.LinearLayout
import android.widget.TextView
import androidx.activity.ComponentActivity
import androidx.core.content.ContextCompat

/** One-time walkthrough. Re-reads every grant on each resume and lists what is still missing. */
class SetupActivity : ComponentActivity() {
    private lateinit var column: LinearLayout

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        column = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(48, 48, 48, 48)
        }
        setContentView(column)
    }

    override fun onResume() {
        super.onResume()
        column.removeAllViews()
        val nm = getSystemService(NotificationManager::class.java)
        val pm = getSystemService(PowerManager::class.java)

        step("Notifications", ContextCompat.checkSelfPermission(this, Manifest.permission.POST_NOTIFICATIONS) ==
            PackageManager.PERMISSION_GRANTED) {
            requestPermissions(arrayOf(Manifest.permission.POST_NOTIFICATIONS), 1)
        }
        step("Do Not Disturb access", nm.isNotificationPolicyAccessGranted) {
            startActivity(Intent(Settings.ACTION_NOTIFICATION_POLICY_ACCESS_SETTINGS))
        }
        step("Full-screen alerts", nm.canUseFullScreenIntent()) {
            startActivity(
                Intent(Settings.ACTION_MANAGE_APP_USE_FULL_SCREEN_INTENT, Uri.parse("package:$packageName")),
            )
        }
        step("Battery optimisation off", pm.isIgnoringBatteryOptimizations(packageName)) {
            startActivity(
                Intent(Settings.ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS, Uri.parse("package:$packageName")),
            )
        }
    }

    private fun step(name: String, granted: Boolean, fix: () -> Unit) {
        column.addView(TextView(this).apply {
            text = (if (granted) "OK: " else "MISSING: ") + name
            textSize = 20f
        })
        if (!granted) column.addView(Button(this).apply {
            text = "Fix: $name"
            setOnClickListener { fix() }
        })
    }
}
