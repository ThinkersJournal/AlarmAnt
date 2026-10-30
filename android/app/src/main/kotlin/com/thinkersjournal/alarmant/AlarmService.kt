package com.thinkersjournal.alarmant

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.media.AudioAttributes
import android.media.AudioManager
import android.media.MediaPlayer
import android.media.RingtoneManager
import android.os.IBinder
import android.os.PowerManager
import androidx.core.app.NotificationCompat
import androidx.core.app.ServiceCompat

/** Plays the alarm on the alarm stream at maximum volume until [ACTION_STOP]. */
class AlarmService : Service() {
    private var player: MediaPlayer? = null
    private var wakeLock: PowerManager.WakeLock? = null
    private var savedVolume: Int? = null

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        when (intent?.action) {
            ACTION_STOP -> {
                stopAlarm()
                stopSelf()
            }
            else -> start(intent)
        }
        return START_NOT_STICKY
    }

    private fun start(intent: Intent?) {
        val alertId = intent?.getStringExtra(EXTRA_ALERT_ID).orEmpty()
        val title = intent?.getStringExtra(EXTRA_TITLE).orEmpty().ifEmpty { "Alert" }
        val body = intent?.getStringExtra(EXTRA_BODY).orEmpty()

        val channel = NotificationChannel(CHANNEL, "Alarms", NotificationManager.IMPORTANCE_HIGH)
        // Honoured only while notification policy access is granted (SetupActivity asks for it).
        channel.setBypassDnd(true)
        channel.setSound(null, null)
        getSystemService(NotificationManager::class.java).createNotificationChannel(channel)

        val open = Intent(this, AlarmActivity::class.java)
            .putExtra(EXTRA_ALERT_ID, alertId)
            .putExtra(EXTRA_TITLE, title)
            .putExtra(EXTRA_BODY, body)
            .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        val pending = PendingIntent.getActivity(
            this, 0, open, PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
        )
        val notification = NotificationCompat.Builder(this, CHANNEL)
            .setSmallIcon(android.R.drawable.ic_lock_idle_alarm)
            .setContentTitle(title)
            .setContentText(body)
            .setCategory(NotificationCompat.CATEGORY_ALARM)
            .setPriority(NotificationCompat.PRIORITY_MAX)
            .setOngoing(true)
            .setContentIntent(pending)
            .setFullScreenIntent(pending, true)
            .build()
        ServiceCompat.startForeground(this, NOTIFICATION_ID, notification, ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PLAYBACK)

        wakeLock?.takeIf { it.isHeld }?.release()
        wakeLock = getSystemService(PowerManager::class.java)
            .newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "alarmant:alarm")
            .apply { acquire(WAKE_LOCK_MS) }

        val audio = getSystemService(Context.AUDIO_SERVICE) as AudioManager
        savedVolume = audio.getStreamVolume(AudioManager.STREAM_ALARM)
        audio.setStreamVolume(AudioManager.STREAM_ALARM, audio.getStreamMaxVolume(AudioManager.STREAM_ALARM), 0)

        player?.release()
        player = null
        // The default alarm tone can be unset or unreadable; fall back to the ringtone, then the
        // notification tone, rather than leaving a critical alert silent.
        for (type in listOf(RingtoneManager.TYPE_ALARM, RingtoneManager.TYPE_RINGTONE, RingtoneManager.TYPE_NOTIFICATION)) {
            player = tryPlay(RingtoneManager.getDefaultUri(type)) ?: continue
            break
        }
    }

    private fun tryPlay(uri: android.net.Uri?): MediaPlayer? {
        if (uri == null) return null
        val p = MediaPlayer()
        return try {
            p.setAudioAttributes(
                AudioAttributes.Builder()
                    .setUsage(AudioAttributes.USAGE_ALARM)
                    .setContentType(AudioAttributes.CONTENT_TYPE_SONIFICATION)
                    .build(),
            )
            p.setDataSource(this, uri)
            p.isLooping = true
            p.prepare()
            p.start()
            p
        } catch (e: java.io.IOException) {
            p.release()
            null
        } catch (e: IllegalStateException) {
            p.release()
            null
        }
    }

    private fun stopAlarm() {
        player?.release()
        player = null
        savedVolume?.let {
            (getSystemService(Context.AUDIO_SERVICE) as AudioManager).setStreamVolume(AudioManager.STREAM_ALARM, it, 0)
        }
        savedVolume = null
        wakeLock?.takeIf { it.isHeld }?.release()
        wakeLock = null
        stopForeground(STOP_FOREGROUND_REMOVE)
    }

    override fun onDestroy() {
        stopAlarm()
        super.onDestroy()
    }

    companion object {
        const val ACTION_START = "com.thinkersjournal.alarmant.START"
        const val ACTION_STOP = "com.thinkersjournal.alarmant.STOP"
        const val EXTRA_ALERT_ID = "alert_id"
        const val EXTRA_TITLE = "title"
        const val EXTRA_BODY = "body"
        private const val CHANNEL = "alarms"
        private const val NOTIFICATION_ID = 1
        private const val WAKE_LOCK_MS = 10L * 60 * 1000
    }
}
