package com.thinkersjournal.alarmant

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class ActiveAlarmTest {
    private val context = InstrumentationRegistry.getInstrumentation().targetContext

    @Test
    fun roundTripsAndClears() {
        ActiveAlarm.clear(context)
        assertNull(ActiveAlarm.get(context))
        ActiveAlarm.set(context, ActiveAlarm.Alert("a1", "Title", "Body"))
        assertEquals(ActiveAlarm.Alert("a1", "Title", "Body"), ActiveAlarm.get(context))
        ActiveAlarm.clear(context)
        assertNull(ActiveAlarm.get(context))
    }
}
