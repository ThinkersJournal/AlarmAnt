package com.thinkersjournal.alarmant

import android.Manifest
import android.app.NotificationManager
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
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

        // POST_NOTIFICATIONS is a runtime permission from API 33; earlier releases grant it at install.
        // Not a grant the user can give the app: a state to warn about. Shown first, in full.
        if (TotalSilence.isOn(this)) column.addView(TextView(this).apply {
            text = "WARNING: " + TotalSilence.WARNING
            textSize = 20f
        })
        step("Notifications", Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU ||
            ContextCompat.checkSelfPermission(this, Manifest.permission.POST_NOTIFICATIONS) ==
            PackageManager.PERMISSION_GRANTED) {
            requestPermissions(arrayOf(Manifest.permission.POST_NOTIFICATIONS), 1)
        }
        step("Do Not Disturb access", nm.isNotificationPolicyAccessGranted) {
            startActivity(Intent(Settings.ACTION_NOTIFICATION_POLICY_ACCESS_SETTINGS))
        }
        // Full-screen intents are a user-revocable setting only from API 34; earlier releases grant it at install.
        step("Full-screen alerts", Build.VERSION.SDK_INT < Build.VERSION_CODES.UPSIDE_DOWN_CAKE ||
            nm.canUseFullScreenIntent()) {
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
