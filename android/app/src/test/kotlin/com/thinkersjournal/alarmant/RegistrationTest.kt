// SPDX-License-Identifier: MIT OR Apache-2.0
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
        val got = Registration.parseLink("  alarmant://enroll?c=abc&u=https://10.0.0.5:8080/ \n")
        assertEquals(Registration.Enrolment("https://10.0.0.5:8080", "abc"), got)
    }

    @Test
    fun aPinInTheLinkIsCarriedAndAMalformedPinRejectsTheLink() {
        val pin = "ab".repeat(32)
        val got = Registration.parseLink("alarmant://enroll?u=https%3A%2F%2F10.0.0.5%3A8443&c=abc&f=$pin")
        assertEquals(Registration.Enrolment("https://10.0.0.5:8443", "abc", pin), got)
        assertNull(Registration.parseLink("alarmant://enroll?u=https://h&c=abc&f=nothex"))
        assertNull(Registration.parseLink("alarmant://enroll?u=https://h&c=abc&f=" + "AB".repeat(32)))
        assertNull(Registration.parseLink("alarmant://enroll?u=https://h&c=abc&f="))
        assertNull(Registration.parseLink("alarmant://enroll?u=https://h&c=abc")!!.pin)
    }

    @Test
    fun anOldHttpEnrolmentIsNotRegistered() {
        assertFalse(Registration.isRegistered("http://10.0.0.5:8080", "tok"))
        assertFalse(Registration.shouldUpdate("http://10.0.0.5:8080", "tok"))
    }

    @Test
    fun malformedLinksAreRejected() {
        for (bad in listOf(
            "", "alarmant://enroll", "alarmant://enroll?u=http://h", "alarmant://enroll?c=abc",
            "alarmant://enroll?u=ftp://h&c=abc", "alarmant://enroll?u=http://h&c=abc", "alarmant://enroll?u=http://h&c=",
            "https://enroll?u=http://h&c=abc", "alarmant://other?u=http://h&c=abc",
        )) assertNull(bad, Registration.parseLink(bad))
    }

    @Test
    fun registeredNeedsBothValues() {
        assertTrue(Registration.isRegistered("https://h", "tok"))
        assertFalse(Registration.isRegistered(null, "tok"))
        assertFalse(Registration.isRegistered("https://h", null))
        assertFalse(Registration.isRegistered("", "tok"))
        assertFalse(Registration.isRegistered("https://h", ""))
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
        assertTrue(Registration.shouldUpdate("https://h", "tok"))
        assertFalse(Registration.shouldUpdate(null, "tok"))
        assertFalse(Registration.shouldUpdate("https://h", null))
    }
}
