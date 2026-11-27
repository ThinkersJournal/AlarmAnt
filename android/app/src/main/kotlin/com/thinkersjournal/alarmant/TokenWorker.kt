// SPDX-License-Identifier: MIT OR Apache-2.0
package com.thinkersjournal.alarmant

import android.content.Context
import androidx.work.CoroutineWorker
import androidx.work.WorkerParameters

/** Sends the rotated FCM token to the sender; WorkManager supplies the retry. A rejection stops quietly. */
class TokenWorker(context: Context, params: WorkerParameters) : CoroutineWorker(context, params) {
    override suspend fun doWork(): Result = when (Registration.updateToken(applicationContext)) {
        Registration.Outcome.DONE, Registration.Outcome.REJECTED -> Result.success()
        Registration.Outcome.RETRY_LATER -> Result.retry()
    }
}
