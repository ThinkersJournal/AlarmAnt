// SPDX-License-Identifier: MIT OR Apache-2.0
package com.thinkersjournal.alarmant

import java.net.URL
import java.security.cert.CertificateException
import javax.net.ssl.HttpsURLConnection
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Test

class PinnedTlsTest {
    private val certA = "certificate A".toByteArray()
    private val certB = "certificate B, valid but another".toByteArray()

    private fun refused(der: ByteArray?, pin: String) = try {
        PinnedTls.checkLeaf(der, pin)
        false
    } catch (_: CertificateException) {
        true
    }

    @Test
    fun sha256OfKnownInput() {
        // SHA-256("abc")
        assertEquals("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad", PinnedTls.sha256Hex("abc".toByteArray()))
    }

    @Test
    fun theRightFingerprintIsAccepted() {
        PinnedTls.checkLeaf(certA, PinnedTls.sha256Hex(certA))
    }

    @Test
    fun aWrongFingerprintIsRefused() {
        assertTrue(refused(certA, "0".repeat(64)))
    }

    @Test
    fun aDifferentValidCertificatesFingerprintIsRefused() {
        assertTrue(refused(certA, PinnedTls.sha256Hex(certB)))
        assertTrue(refused(certB, PinnedTls.sha256Hex(certA)))
    }

    @Test
    fun nothingPresentedOrABadPinIsRefused() {
        val pin = PinnedTls.sha256Hex(certA)
        assertTrue(refused(null, pin))
        assertTrue(refused(ByteArray(0), pin))
        assertTrue(refused(certA, ""))
        assertTrue(refused(certA, pin.uppercase()))
        assertTrue(refused(certA, pin.dropLast(1)))
    }

    @Test
    fun onlyLowercaseSha256HexIsAPin() {
        assertTrue(PinnedTls.isPin("a".repeat(64)))
        assertFalse(PinnedTls.isPin(null))
        assertFalse(PinnedTls.isPin("A".repeat(64)))
        assertFalse(PinnedTls.isPin("g".repeat(64)))
        assertFalse(PinnedTls.isPin("a".repeat(63)))
    }

    @Test
    fun withoutAPinTheSystemTrustStoreStaysInForce() {
        val conn = PinnedTls.open(URL("https://sender.example"), null) as HttpsURLConnection
        assertEquals(HttpsURLConnection.getDefaultSSLSocketFactory(), conn.sslSocketFactory)
    }

    @Test
    fun withAPinThisConnectionGetsItsOwnSocketFactoryAndTheDefaultIsUntouched() {
        val before = HttpsURLConnection.getDefaultSSLSocketFactory()
        val conn = PinnedTls.open(URL("https://10.0.0.5:8443"), PinnedTls.sha256Hex(certA)) as HttpsURLConnection
        assertNotEquals(before, conn.sslSocketFactory)
        assertEquals(before, HttpsURLConnection.getDefaultSSLSocketFactory())
    }

    @Test
    fun aPinOnAnHttpUrlIsRefusedNotIgnored() {
        try {
            PinnedTls.open(URL("http://10.0.0.5:8080"), PinnedTls.sha256Hex(certA))
            fail("expected an exception: a pin on http must not fall back to cleartext")
        } catch (_: IllegalArgumentException) {
        }
    }

    @Test
    fun aMismatchIsRecognisedThroughItsCauseChainAndNothingElseIs() {
        val mismatch = try {
            PinnedTls.checkLeaf(certA, "0".repeat(64))
            null
        } catch (e: CertificateException) {
            e
        }
        assertTrue(PinnedTls.isPinFailure(mismatch))
        assertTrue(PinnedTls.isPinFailure(javax.net.ssl.SSLHandshakeException("handshake").initCause(mismatch)))
        assertFalse(PinnedTls.isPinFailure(java.io.IOException("timeout")))
        assertFalse(PinnedTls.isPinFailure(CertificateException("other")))
        assertFalse(PinnedTls.isPinFailure(null))
    }

    @Test
    fun anInvalidPinIsNotSilentlyIgnored() {
        try {
            PinnedTls.open(URL("https://h"), "nope")
            fail("expected an exception")
        } catch (_: IllegalArgumentException) {
        }
    }
}
