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
                ActiveAlarm.clear(this)
                stopAlarm()
                stopSelf()
                return START_NOT_STICKY
            }
            else -> start(intent)
        }
        // If the process dies mid-alarm the system restarts the service with this same intent;
        // only Acknowledge (ACTION_STOP) ends it.
        return START_REDELIVER_INTENT
    }

    private fun start(intent: Intent?) {
        // A restart with a null intent (never expected with REDELIVER) falls back to the saved alert.
        val alert = if (intent?.hasExtra(EXTRA_ALERT_ID) == true) {
            ActiveAlarm.Alert(
                intent.getStringExtra(EXTRA_ALERT_ID).orEmpty(),
                intent.getStringExtra(EXTRA_TITLE).orEmpty().ifEmpty { "Alert" },
                intent.getStringExtra(EXTRA_BODY).orEmpty(),
            )
        } else {
            ActiveAlarm.get(this) ?: ActiveAlarm.Alert("", "Alert", "")
        }
        ActiveAlarm.set(this, alert)
        ensureChannel(this)
        ServiceCompat.startForeground(this, NOTIFICATION_ID, notification(this, alert), ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PLAYBACK)

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
        fun startIntent(c: Context, alert: ActiveAlarm.Alert): Intent =
            Intent(c, AlarmService::class.java)
                .setAction(ACTION_START)
                .putExtra(EXTRA_ALERT_ID, alert.id)
                .putExtra(EXTRA_TITLE, alert.title)
                .putExtra(EXTRA_BODY, alert.body)

        fun ensureChannel(c: Context) {
            val channel = NotificationChannel(CHANNEL, "Alarms", NotificationManager.IMPORTANCE_HIGH)
            // Honoured only while notification policy access is granted (SetupActivity asks for it).
            channel.setBypassDnd(true)
            channel.setSound(null, null)
            c.getSystemService(NotificationManager::class.java).createNotificationChannel(channel)
        }

        /** [recover]: the activity must start the service itself (see BootReceiver). */
        fun notification(c: Context, alert: ActiveAlarm.Alert, recover: Boolean = false): Notification {
            val open = Intent(c, AlarmActivity::class.java)
                .putExtra(EXTRA_ALERT_ID, alert.id)
                .putExtra(EXTRA_TITLE, alert.title)
                .putExtra(EXTRA_BODY, alert.body)
                .putExtra(EXTRA_RECOVER, recover)
                .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
            val pending = PendingIntent.getActivity(
                c, 0, open, PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
            )
            return NotificationCompat.Builder(c, CHANNEL)
                .setSmallIcon(android.R.drawable.ic_lock_idle_alarm)
                .setContentTitle(alert.title)
                .setContentText(alert.body)
                .setCategory(NotificationCompat.CATEGORY_ALARM)
                .setPriority(NotificationCompat.PRIORITY_MAX)
                .setOngoing(true)
                .setContentIntent(pending)
                .setFullScreenIntent(pending, true)
                .build()
        }

        const val ACTION_START = "com.thinkersjournal.alarmant.START"
        const val ACTION_STOP = "com.thinkersjournal.alarmant.STOP"
        const val EXTRA_ALERT_ID = "alert_id"
        const val EXTRA_TITLE = "title"
        const val EXTRA_BODY = "body"
        const val EXTRA_RECOVER = "recover"
        private const val CHANNEL = "alarms"
        const val NOTIFICATION_ID = 1
        private const val WAKE_LOCK_MS = 10L * 60 * 1000
    }
}
