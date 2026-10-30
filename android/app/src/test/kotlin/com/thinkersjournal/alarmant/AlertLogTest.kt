package com.thinkersjournal.alarmant

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class AlertLogTest {
    @Test
    fun firstSightAppends() {
        assertEquals(listOf("a", "b"), AlertLog.window(listOf("a"), "b"))
    }

    @Test
    fun repeatIsRejected() {
        assertNull(AlertLog.window(listOf("a", "b"), "a"))
    }

    @Test
    fun emptyWindowAcceptsFirstId() {
        assertEquals(listOf("a"), AlertLog.window(emptyList(), "a"))
    }

    @Test
    fun evictsOldestNotArbitrary() {
        val full = (1..AlertLog.MAX).map { "id$it" }
        val next = AlertLog.window(full, "new")!!
        assertEquals(AlertLog.MAX, next.size)
        assertEquals("id2", next.first())
        assertEquals("new", next.last())
        // The evicted id is accepted again; every retained id is still rejected.
        assertEquals(listOf("id1"), listOf("id1").filter { AlertLog.window(next, it) != null })
        assertNull(AlertLog.window(next, "id2"))
    }
}
