package com.thinkersjournal.alarmant

import android.app.NotificationManager
import android.content.Context
import android.content.Intent
import android.media.AudioAttributes
import android.media.AudioManager
import androidx.core.content.ContextCompat
import androidx.test.core.app.ActivityScenario
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import android.widget.Button
import android.view.View
import android.view.ViewGroup
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith

/**
 * The alarm path on a device or emulator: service -> alarm-stream playback -> Acknowledge.
 * Process death, reboot, and Do Not Disturb need the OS to act on the process, so they stay manual
 * (android/README "Emulator run").
 */
@RunWith(AndroidJUnit4::class)
class AlarmPathTest {
    private val context: Context = InstrumentationRegistry.getInstrumentation().targetContext
    private val audio = context.getSystemService(AudioManager::class.java)
    private var volumeBefore = 0

    @Before
    fun setUp() {
        // Without this the notification (and so the foreground service on API 33+) is not shown.
        InstrumentationRegistry.getInstrumentation().uiAutomation
            .grantRuntimePermission(context.packageName, android.Manifest.permission.POST_NOTIFICATIONS)
        volumeBefore = audio.getStreamVolume(AudioManager.STREAM_ALARM)
        ActiveAlarm.clear(context)
    }

    @After
    fun tearDown() {
        stopService()
        ActiveAlarm.clear(context)
    }

    private fun startService(id: String) =
        ContextCompat.startForegroundService(context, AlarmService.startIntent(context, ActiveAlarm.Alert(id, "T", "b")))

    private fun stopService() {
        context.startService(Intent(context, AlarmService::class.java).setAction(AlarmService.ACTION_STOP))
    }

    private fun alarmPlayers() = audio.activePlaybackConfigurations.count {
        it.audioAttributes.usage == AudioAttributes.USAGE_ALARM
    }

    private fun waitFor(timeoutMs: Long = 20_000, cond: () -> Boolean): Boolean {
        val end = System.currentTimeMillis() + timeoutMs
        while (System.currentTimeMillis() < end) {
            if (cond()) return true
            Thread.sleep(250)
        }
        return cond()
    }

    @Test
    fun startPlaysOnAlarmStreamAtMaxAndRecordsTheAlert() {
        startService("it-1")
        assertTrue("alarm player never started", waitFor { alarmPlayers() >= 1 })
        assertEquals(audio.getStreamMaxVolume(AudioManager.STREAM_ALARM), audio.getStreamVolume(AudioManager.STREAM_ALARM))
        assertEquals("it-1", ActiveAlarm.get(context)?.id)
        assertEquals("it-1", AlarmService.playingId)
    }

    @Test
    fun repeatedStartDoesNotRestartTheAudio() {
        startService("it-2")
        assertTrue(waitFor { alarmPlayers() == 1 })
        val before = audio.activePlaybackConfigurations.first { it.audioAttributes.usage == AudioAttributes.USAGE_ALARM }.toString()
        startService("it-2")
        Thread.sleep(3_000)
        assertEquals(1, alarmPlayers())
        val after = audio.activePlaybackConfigurations.first { it.audioAttributes.usage == AudioAttributes.USAGE_ALARM }.toString()
        assertEquals("same player id expected", before.substringBefore(" "), after.substringBefore(" "))
    }

    @Test
    fun stopSilencesClearsAndRestoresVolume() {
        startService("it-3")
        assertTrue(waitFor { alarmPlayers() >= 1 })
        stopService()
        assertTrue("alarm still playing after STOP", waitFor { alarmPlayers() == 0 })
        assertNull(ActiveAlarm.get(context))
        assertNull(AlarmService.playingId)
        assertEquals(volumeBefore, audio.getStreamVolume(AudioManager.STREAM_ALARM))
    }

    @Test
    fun notificationUsesTheAlarmChannelAndFullScreenIntent() {
        startService("it-4")
        assertTrue(waitFor { AlarmService.playingId == "it-4" })
        val nm = context.getSystemService(NotificationManager::class.java)
        assertTrue(waitFor { nm.activeNotifications.any { it.id == AlarmService.NOTIFICATION_ID } })
        val n = nm.activeNotifications.first { it.id == AlarmService.NOTIFICATION_ID }.notification
        assertEquals("alarms", n.channelId)
        assertNotNull(n.fullScreenIntent)
    }

    @Test
    fun alarmActivityBlocksBackAndAcknowledgeFinishesAndStopsTheAlarm() {
        startService("it-5")
        assertTrue(waitFor { alarmPlayers() >= 1 })
        val intent = Intent(context, AlarmActivity::class.java)
            .putExtra(AlarmService.EXTRA_ALERT_ID, "it-5").putExtra(AlarmService.EXTRA_TITLE, "T").putExtra(AlarmService.EXTRA_BODY, "b")
        ActivityScenario.launch<AlarmActivity>(intent).use { scenario ->
            scenario.onActivity { it.onBackPressedDispatcher.onBackPressed() }
            scenario.onActivity { assertFalse("Back must not leave the alarm screen", it.isFinishing) }
            scenario.onActivity { activity ->
                fun find(v: View): Button? = if (v is Button && v.text == "Acknowledge") v
                    else (v as? ViewGroup)?.let { g -> (0 until g.childCount).firstNotNullOfOrNull { find(g.getChildAt(it)) } }
                find(activity.window.decorView)!!.performClick()
            }
            assertTrue("Acknowledge did not stop the alarm", waitFor { alarmPlayers() == 0 })
            assertNull(ActiveAlarm.get(context))
            scenario.onActivity { assertTrue(it.isFinishing) }
        }
    }
}
