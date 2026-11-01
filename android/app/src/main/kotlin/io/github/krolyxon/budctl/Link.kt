package io.github.krolyxon.budctl

import java.io.Closeable
import java.io.IOException
import java.io.InputStream
import java.io.OutputStream
import java.util.concurrent.LinkedBlockingQueue
import java.util.concurrent.TimeUnit
import kotlin.concurrent.thread

/**
 * One RFCOMM link to the buds. A Bluetooth socket's input stream has no read timeout, so a thread
 * of its own reads it into a queue and [recv] waits on the queue. Closing the socket ends that
 * thread.
 */
class Link(private val input: InputStream, private val output: OutputStream, private val socket: Closeable) {
    private val received = LinkedBlockingQueue<ByteArray>()

    init {
        thread(name = "budctl-link", isDaemon = true) {
            val buffer = ByteArray(1024)
            try {
                while (true) {
                    val n = input.read(buffer)
                    if (n < 0) break
                    if (n > 0) received.put(buffer.copyOf(n))
                }
            } catch (_: IOException) {
                // Closed, by us or by the buds.
            }
            received.put(CLOSED)
        }
    }

    fun send(bytes: ByteArray) {
        output.write(bytes)
        output.flush()
    }

    /** The next bytes; null if none came within [timeoutMs]; an empty array once the link is closed. */
    fun recv(timeoutMs: Long): ByteArray? {
        val bytes = received.poll(timeoutMs, TimeUnit.MILLISECONDS) ?: return null
        // Closed stays closed, for the next call too.
        if (bytes.isEmpty()) received.put(CLOSED)
        return bytes
    }

    fun close() {
        try {
            socket.close()
        } catch (_: IOException) {
            // Already closed.
        }
    }

    private companion object {
        val CLOSED = ByteArray(0)
    }
}
