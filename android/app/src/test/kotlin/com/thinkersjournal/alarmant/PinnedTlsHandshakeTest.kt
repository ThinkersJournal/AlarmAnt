package com.thinkersjournal.alarmant

import java.io.File
import java.io.IOException
import java.net.InetAddress
import java.net.URL
import java.security.KeyStore
import java.security.MessageDigest
import java.security.cert.X509Certificate
import javax.net.ssl.KeyManagerFactory
import javax.net.ssl.SSLContext
import javax.net.ssl.SSLException
import javax.net.ssl.SSLServerSocket
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Test

/**
 * A real TLS handshake against a loopback server whose certificate is made with the JDK's own `keytool`, so the pin
 * check in `PinnedTls.PinTrustManager` and the default hostname verifier run for real (ROADMAP A11): the pinned
 * certificate is accepted, another valid certificate is refused as a pin mismatch, and a pinned certificate that does
 * not carry the address in its subject alternative names is refused by the hostname check.
 */
class PinnedTlsHandshakeTest {
    private val dir = File.createTempFile("alarmant-tls", "").also { it.delete(); it.mkdirs() }
    private val servers = mutableListOf<SSLServerSocket>()

    @After
    fun cleanUp() {
        servers.forEach { runCatching { it.close() } }
        dir.deleteRecursively()
    }

    private class Identity(val keyStore: KeyStore, val pin: String)

    /** A fresh self-signed certificate whose subject alternative name is `ip:[san]`, made with the JDK's keytool. */
    private fun identity(name: String, san: String): Identity {
        val file = File(dir, "$name.p12")
        val keytool = File(File(System.getProperty("java.home"), "bin"), if (File.separatorChar == '\\') "keytool.exe" else "keytool")
        val proc = ProcessBuilder(
            keytool.path, "-genkeypair", "-alias", name, "-keyalg", "EC", "-groupname", "secp256r1",
            "-dname", "CN=alarmant-test", "-ext", "san=ip:$san", "-validity", "2",
            "-storetype", "PKCS12", "-keystore", file.path, "-storepass", "changeit", "-keypass", "changeit",
        ).redirectErrorStream(true).start()
        val out = proc.inputStream.bufferedReader().readText()
        assertEquals("keytool failed: $out", 0, proc.waitFor())
        val ks = KeyStore.getInstance("PKCS12")
        file.inputStream().use { ks.load(it, "changeit".toCharArray()) }
        val der = (ks.getCertificate(name) as X509Certificate).encoded
        return Identity(ks, MessageDigest.getInstance("SHA-256").digest(der).joinToString("") { "%02x".format(it) })
    }

    /** Serves one fixed HTTP reply per connection over TLS on a loopback port; returns the https URL. */
    private fun serve(id: Identity): URL {
        val kmf = KeyManagerFactory.getInstance(KeyManagerFactory.getDefaultAlgorithm())
        kmf.init(id.keyStore, "changeit".toCharArray())
        val ctx = SSLContext.getInstance("TLS")
        ctx.init(kmf.keyManagers, null, null)
        val server = ctx.serverSocketFactory.createServerSocket(0, 5, InetAddress.getByName("127.0.0.1")) as SSLServerSocket
        servers += server
        Thread {
            while (!server.isClosed) {
                try {
                    server.accept().use { s ->
                        s.soTimeout = 5000
                        val reader = s.getInputStream().bufferedReader()
                        while (true) {
                            val line = reader.readLine() ?: break
                            if (line.isEmpty()) break
                        }
                        s.getOutputStream().write("HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok".toByteArray())
                        s.getOutputStream().flush()
                    }
                } catch (_: IOException) {
                    // A refused handshake ends that connection; the loop serves the next.
                }
            }
        }.also { it.isDaemon = true }.start()
        return URL("https://127.0.0.1:${server.localPort}/")
    }

    private fun fetch(url: URL, pin: String): Int {
        val conn = PinnedTls.open(url, pin)
        conn.connectTimeout = 5000
        conn.readTimeout = 5000
        return try {
            conn.responseCode
        } finally {
            conn.disconnect()
        }
    }

    private fun failure(url: URL, pin: String): Throwable = try {
        fetch(url, pin)
        fail("the handshake must be refused")
        throw AssertionError()
    } catch (e: SSLException) {
        e
    }

    @Test
    fun theRightCertificateIsAccepted() {
        val a = identity("a", "127.0.0.1")
        assertEquals(200, fetch(serve(a), a.pin))
    }

    @Test
    fun anotherValidCertificateIsRefusedAsAPinMismatch() {
        val a = identity("a", "127.0.0.1")
        val b = identity("b", "127.0.0.1")
        val e = failure(serve(b), a.pin)
        assertTrue("expected a pin mismatch, got $e", PinnedTls.isPinFailure(e))
    }

    @Test
    fun aPinnedCertificateForAnotherAddressIsRefusedByTheHostnameCheck() {
        // The pin matches, so the pin check passes; only the default hostname verifier can refuse this one.
        val c = identity("c", "10.9.9.9")
        val e = failure(serve(c), c.pin)
        assertTrue("a hostname mismatch is not a pin failure, got $e", !PinnedTls.isPinFailure(e))
    }
}
