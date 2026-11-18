package com.thinkersjournal.alarmant

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class RegistrationTest {
    @Test
    fun parsesTheEnrolmentLink() {
        val got = Registration.parseLink("alarmant://enroll?u=https%3A%2F%2Fhost.example%3A8443&c=abc123")
        assertEquals(Registration.Enrolment("https://host.example:8443", "abc123"), got)
    }

    @Test
    fun trailingSlashAndSurroundingSpaceAreTolerated() {
        val got = Registration.parseLink("  alarmant://enroll?c=abc&u=http://10.0.0.5:8080/ \n")
        assertEquals(Registration.Enrolment("http://10.0.0.5:8080", "abc"), got)
    }

    @Test
    fun malformedLinksAreRejected() {
        for (bad in listOf(
            "", "alarmant://enroll", "alarmant://enroll?u=http://h", "alarmant://enroll?c=abc",
            "alarmant://enroll?u=ftp://h&c=abc", "alarmant://enroll?u=http://h&c=",
            "https://enroll?u=http://h&c=abc", "alarmant://other?u=http://h&c=abc",
        )) assertNull(bad, Registration.parseLink(bad))
    }

    @Test
    fun registeredNeedsBothValues() {
        assertTrue(Registration.isRegistered("http://h", "tok"))
        assertFalse(Registration.isRegistered(null, "tok"))
        assertFalse(Registration.isRegistered("http://h", null))
        assertFalse(Registration.isRegistered("", "tok"))
        assertFalse(Registration.isRegistered("http://h", ""))
    }

    @Test
    fun theAckTokenIsReadFromTheRegisterAnswer() {
        assertEquals("t0k", Registration.ackTokenFrom("""{"ack_token":"t0k"}"""))
        assertNull(Registration.ackTokenFrom("""{"ack_token":""}"""))
        assertNull(Registration.ackTokenFrom("not json"))
    }

    @Test
    fun onlyCreatedIsSuccess() {
        assertEquals(Registration.Outcome.DONE, Registration.outcome(201))
        assertEquals(Registration.Outcome.REJECTED, Registration.outcome(401))
        assertEquals(Registration.Outcome.RETRY_LATER, Registration.outcome(429))
        assertEquals(Registration.Outcome.RETRY_LATER, Registration.outcome(500))
    }
}

class TokenUpdateTest {
    @Test
    fun answersMapToDoneStopOrRetry() {
        assertEquals(Registration.Outcome.DONE, Registration.tokenOutcome(200))
        assertEquals(Registration.Outcome.REJECTED, Registration.tokenOutcome(401))
        assertEquals(Registration.Outcome.RETRY_LATER, Registration.tokenOutcome(500))
        assertEquals(Registration.Outcome.RETRY_LATER, Registration.tokenOutcome(429))
        // 201 is the register answer, not this one.
        assertEquals(Registration.Outcome.RETRY_LATER, Registration.tokenOutcome(201))
    }

    @Test
    fun updatesAreOnlyAttemptedWhenRegistered() {
        assertTrue(Registration.shouldUpdate("http://h", "tok"))
        assertFalse(Registration.shouldUpdate(null, "tok"))
        assertFalse(Registration.shouldUpdate("http://h", null))
    }
}
