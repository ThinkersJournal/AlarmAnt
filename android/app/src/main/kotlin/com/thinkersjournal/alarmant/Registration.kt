package com.thinkersjournal.alarmant

import android.content.Context
import androidx.core.content.edit
import org.json.JSONObject
import java.io.IOException
import java.net.URL
import java.net.URLDecoder

/**
 * One-time enrolment with the sender (DESIGN-REGISTRATION section 2, slice h): POST /register with the
 * enrolment code as the bearer, keep the ack token it answers with. Nothing here reports a status other
 * than the phone's own call (PATENT-SWEEP constraint 13); the code, FCM token and ack token are never
 * logged, and a failure shows fixed text only.
 */
object Registration {
    data class Enrolment(val baseUrl: String, val code: String, val pin: String? = null) {
        override fun toString() = "Enrolment(<redacted>)"
    }

    enum class Outcome { DONE, REJECTED, RETRY_LATER }

    /** `alarmant://enroll?u=<https base url>&c=<code>[&f=<sha256 hex of the server certificate>]`; a malformed `f` rejects the link; null for anything else, `http://` included (the code and ack token must not cross in the clear). */
    fun parseLink(link: String): Enrolment? {
        val text = link.trim()
        val prefix = "alarmant://enroll?"
        if (!text.startsWith(prefix)) return null
        val params = text.removePrefix(prefix).split('&').mapNotNull {
            val i = it.indexOf('=')
            if (i < 0) null else it.substring(0, i) to URLDecoder.decode(it.substring(i + 1), "UTF-8")
        }.toMap()
        val base = params["u"]?.trimEnd('/') ?: return null
        val code = params["c"] ?: return null
        if (code.isEmpty() || !base.startsWith("https://")) return null
        val pin = params["f"]
        if (pin != null && !PinnedTls.isPin(pin)) return null
        return Enrolment(base, code, pin)
    }

    /** Registered needs an `https://` sender address: an enrolment made over `http://` (before 0.3.0) is no longer usable and must be redone. */
    fun isRegistered(baseUrl: String?, token: String?) =
        !baseUrl.isNullOrEmpty() && baseUrl.startsWith("https://") && !token.isNullOrEmpty()

    fun isRegistered(context: Context): Boolean {
        val config = context.getSharedPreferences("config", Context.MODE_PRIVATE)
        return isRegistered(config.getString("ack_base_url", null), config.getString("device_token", null))
    }

    fun ackTokenFrom(body: String): String? = try {
        JSONObject(body).optString("ack_token").ifEmpty { null }
    } catch (_: org.json.JSONException) {
        null
    }

    fun outcome(code: Int): Outcome = when (code) {
        201 -> Outcome.DONE
        401 -> Outcome.REJECTED
        else -> Outcome.RETRY_LATER
    }

    /** `PUT /device/token` answers 200; 401 means the sender no longer knows this phone (stop, keep everything local). */
    fun tokenOutcome(code: Int): Outcome = when (code) {
        200 -> Outcome.DONE
        401 -> Outcome.REJECTED
        else -> Outcome.RETRY_LATER
    }

    fun shouldUpdate(baseUrl: String?, token: String?) = isRegistered(baseUrl, token)

    /**
     * After Firebase rotates the push token: `PUT /device/token` with the ack token as bearer (DESIGN-REGISTRATION
     * section 4). Blocking: run off the main thread. A rejection changes nothing on the phone (no self-invalidation).
     */
    fun updateToken(context: Context): Outcome {
        val config = context.getSharedPreferences("config", Context.MODE_PRIVATE)
        val base = config.getString("ack_base_url", null)
        val ack = config.getString("device_token", null)
        val fcm = config.getString("fcm_token", null)
        val pin = config.getString("ack_pin", null)
        if (!shouldUpdate(base, ack) || fcm.isNullOrEmpty()) return Outcome.RETRY_LATER
        return try {
            val conn = PinnedTls.open(URL("$base/device/token"), pin)
            try {
                conn.requestMethod = "PUT"
                conn.setRequestProperty("Authorization", "Bearer $ack")
                conn.setRequestProperty("Content-Type", "application/json")
                conn.doOutput = true
                conn.connectTimeout = 10_000
                conn.readTimeout = 10_000
                conn.outputStream.use { it.write(JSONObject().put("fcm_token", fcm).toString().toByteArray()) }
                tokenOutcome(conn.responseCode)
            } finally {
                conn.disconnect()
            }
        } catch (_: IllegalArgumentException) {
            // An unusable stored pin or address: nothing to retry, re-enrol (PinnedTls.open refuses it).
            Outcome.REJECTED
        } catch (_: IOException) {
            Outcome.RETRY_LATER
        }
    }

    /** Blocking network call: run off the main thread. Stores the base URL and ack token on success. */
    fun register(context: Context, enrolment: Enrolment): Outcome {
        val config = context.getSharedPreferences("config", Context.MODE_PRIVATE)
        val fcm = config.getString("fcm_token", null) ?: return Outcome.RETRY_LATER
        return try {
            val conn = PinnedTls.open(URL("${enrolment.baseUrl}/register"), enrolment.pin)
            try {
                conn.requestMethod = "POST"
                conn.setRequestProperty("Authorization", "Bearer ${enrolment.code}")
                conn.setRequestProperty("Content-Type", "application/json")
                conn.doOutput = true
                conn.connectTimeout = 10_000
                conn.readTimeout = 10_000
                conn.outputStream.use { it.write(JSONObject().put("fcm_token", fcm).toString().toByteArray()) }
                val result = outcome(conn.responseCode)
                if (result != Outcome.DONE) return result
                val token = ackTokenFrom(conn.inputStream.bufferedReader().use { it.readText() })
                    ?: return Outcome.RETRY_LATER
                config.edit {
                    putString("ack_base_url", enrolment.baseUrl)
                    putString("device_token", token)
                    if (enrolment.pin != null) putString("ack_pin", enrolment.pin) else remove("ack_pin")
                }
                Outcome.DONE
            } finally {
                conn.disconnect()
            }
        } catch (_: IllegalArgumentException) {
            // An unusable stored pin or address: nothing to retry, re-enrol (PinnedTls.open refuses it).
            Outcome.REJECTED
        } catch (_: IOException) {
            Outcome.RETRY_LATER
        }
    }
}
