package io.github.krolyxon.budctl

import java.io.ByteArrayOutputStream
import java.io.PipedInputStream
import java.io.PipedOutputStream
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** The link's reader thread and timeouts, with pipes for the socket. */
class LinkTest {
    private val buds = PipedOutputStream()
    private val sent = ByteArrayOutputStream()
    private var closed = false

    // Closing the socket ends the stream, as with a real one.
    private val link = Link(PipedInputStream(buds), sent) {
        closed = true
        buds.close()
    }

    @Test
    fun bytesFromTheBudsArrive() {
        buds.write(byteArrayOf(0xAA.toByte(), 0x07))
        buds.flush()
        assertArrayEquals(byteArrayOf(0xAA.toByte(), 0x07), link.recv(1_000))
    }

    @Test
    fun silenceIsNullOnceTheTimeRunsOut() {
        val start = System.nanoTime()
        assertNull(link.recv(100))
        assertTrue("waited the timeout", System.nanoTime() - start >= 90_000_000)
    }

    @Test
    fun aClosedLinkStaysClosed() {
        buds.close()
        assertArrayEquals(ByteArray(0), link.recv(1_000))
        assertArrayEquals(ByteArray(0), link.recv(1_000))
    }

    @Test
    fun sendWritesAndCloseClosesTheSocket() {
        link.send(byteArrayOf(0x01, 0x02))
        assertArrayEquals(byteArrayOf(0x01, 0x02), sent.toByteArray())
        link.close()
        assertTrue(closed)
        assertArrayEquals(ByteArray(0), link.recv(1_000))
    }
}
