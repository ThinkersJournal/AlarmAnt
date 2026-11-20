package com.thinkersjournal.alarmant

import android.annotation.SuppressLint
import java.net.HttpURLConnection
import java.net.URL
import java.security.MessageDigest
import java.security.cert.CertificateException
import java.security.cert.X509Certificate
import javax.net.ssl.HttpsURLConnection
import javax.net.ssl.SSLContext
import javax.net.ssl.X509TrustManager

/**
 * TLS to a self-signed home-LAN sender, pinned to the SHA-256 of its leaf certificate (the `f=` value in the enrol
 * link; PATENT-TLS-PIN-READING.md: one value in the link, no certificate request, no signature, no rotation).
 *
 * Android's guidance is Network Security Config, which is a static resource and cannot take a fingerprint the operator
 * supplies at run time, so this is the one place that replaces CA trust. It is deliberately narrow: used only for
 * connections opened through [open] with a pin, never installed as the app or JVM default; a certificate is accepted
 * only when its SHA-256 equals the pin, and every other path throws; the default hostname verifier is kept (the
 * sender's certificate carries the address as a subject alternative name). Without a pin, [open] changes nothing and
 * the system trust store applies, so a proxied public certificate still works.
 */
object PinnedTls {
    private val PIN = Regex("[0-9a-f]{64}")

    /** A pin is exactly 64 lowercase hex digits (SHA-256); anything else is not a pin. */
    fun isPin(value: String?): Boolean = value != null && PIN.matches(value)

    fun sha256Hex(der: ByteArray): String =
        MessageDigest.getInstance("SHA-256").digest(der).joinToString("") { "%02x".format(it) }

    /** Throws unless [der] is a certificate whose SHA-256 equals [pin]. The comparison is constant-time. */
    fun checkLeaf(der: ByteArray?, pin: String) {
        if (!isPin(pin)) throw CertificateException("no valid pin")
        if (der == null || der.isEmpty()) throw CertificateException("no certificate presented")
        val got = sha256Hex(der).toByteArray(Charsets.US_ASCII)
        if (!MessageDigest.isEqual(got, pin.toByteArray(Charsets.US_ASCII))) {
            throw PinMismatch("certificate does not match the pin")
        }
    }

    /** The pin did not match: permanent until the phone is enrolled again, so callers must not retry it for ever. */
    class PinMismatch(message: String) : CertificateException(message)

    /** True when [e], or anything it was caused by, is a pin mismatch. */
    fun isPinFailure(e: Throwable?): Boolean {
        var t = e
        var depth = 0
        while (t != null && depth < 10) {
            if (t is PinMismatch) return true
            t = t.cause
            depth++
        }
        return false
    }

    @SuppressLint("CustomX509TrustManager") // Self-signed LAN sender: the pin replaces CA trust for this connection only; see the class comment.
    private class PinTrustManager(private val pin: String) : X509TrustManager {
        override fun checkClientTrusted(chain: Array<out X509Certificate>?, authType: String?) {
            throw CertificateException("client certificates are not accepted")
        }

        override fun checkServerTrusted(chain: Array<out X509Certificate>?, authType: String?) {
            val leaf = chain?.firstOrNull() ?: throw CertificateException("no certificate presented")
            checkLeaf(leaf.encoded, pin)
        }

        override fun getAcceptedIssuers(): Array<X509Certificate> = emptyArray()
    }

    /** A connection to [url]; with a valid [pin] on an https URL only the pinned certificate is accepted. */
    fun open(url: URL, pin: String?): HttpURLConnection {
        // Fail closed: a pin is only meaningful on https, and a bad pin or a non-https URL with a pin is an
        // error, never a silent fallback to cleartext or to system trust.
        if (pin != null) {
            require(isPin(pin)) { "invalid pin" }
            require(url.protocol == "https") { "a pin needs an https URL" }
        }
        val conn = url.openConnection() as HttpURLConnection
        if (conn is HttpsURLConnection && pin != null) {
            val ctx = SSLContext.getInstance("TLS")
            ctx.init(null, arrayOf(PinTrustManager(pin)), null)
            conn.sslSocketFactory = ctx.socketFactory
        }
        return conn
    }
}
