package com.thinkersjournal.alarmant

import com.thinkersjournal.alarmant.AckWorker.Outcome
import org.junit.Assert.assertEquals
import org.junit.Test

class AckWorkerTest {
    @Test
    fun successIsDone() {
        assertEquals(Outcome.DONE, AckWorker.outcome(200))
        assertEquals(Outcome.DONE, AckWorker.outcome(204))
    }

    @Test
    fun rejectedTokenAndUnknownAlertStop() {
        assertEquals(Outcome.STOP, AckWorker.outcome(401))
        assertEquals(Outcome.STOP, AckWorker.outcome(404))
    }

    @Test
    fun serverErrorsAndOtherCodesRetry() {
        assertEquals(Outcome.RETRY, AckWorker.outcome(500))
        assertEquals(Outcome.RETRY, AckWorker.outcome(503))
        assertEquals(Outcome.RETRY, AckWorker.outcome(403))
        assertEquals(Outcome.RETRY, AckWorker.outcome(429))
    }
}
