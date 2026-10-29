package com.thinkersjournal.alarmant

import android.content.Context
import androidx.work.CoroutineWorker
import androidx.work.WorkerParameters
import java.net.HttpURLConnection
import java.net.URL

/**
 * Calls POST {ack_base_url}/ack/{alert_id} with the device token (A4, DESIGN-A2 flow step 4).
 * WorkManager supplies the retry; a non-2xx answer or an I/O error asks for another attempt.
 * The base URL and token come from "config" preferences; registration is not designed yet.
 */
class AckWorker(context: Context, params: WorkerParameters) : CoroutineWorker(context, params) {

    override suspend fun doWork(): Result {
        val alertId = inputData.getString(AlarmService.EXTRA_ALERT_ID) ?: return Result.failure()
        val config = applicationContext.getSharedPreferences("config", Context.MODE_PRIVATE)
        val base = config.getString("ack_base_url", null) ?: return Result.retry()
        val token = config.getString("device_token", null) ?: return Result.retry()
        return try {
            val conn = URL("$base/ack/$alertId").openConnection() as HttpURLConnection
            try {
                conn.requestMethod = "POST"
                conn.setRequestProperty("Authorization", "Bearer $token")
                conn.connectTimeout = 10_000
                conn.readTimeout = 10_000
                if (conn.responseCode in 200..299) Result.success() else Result.retry()
            } finally {
                conn.disconnect()
            }
        } catch (_: java.io.IOException) {
            Result.retry()
        }
    }
}
