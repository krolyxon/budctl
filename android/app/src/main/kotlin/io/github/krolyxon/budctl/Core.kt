package io.github.krolyxon.budctl

/**
 * budctl-core, in libbudctl_android.so (crates/budctl-android): the worker that owns the session
 * with the buds. See docs/android.md#boundary.
 */
object Core {
    init {
        System.loadLibrary("budctl_android")
    }

    /** Starts the worker, in place of any running one. Null, or why it could not start. */
    @JvmStatic
    external fun start(platform: Platform, idleMs: Long, debug: Boolean): String?

    /** Queues one command (JSON); its event comes back through [Platform.event]. Null, or why not. */
    @JvmStatic
    external fun command(json: String): String?

    /** Ends the worker, which closes any session once the command at hand is done. */
    @JvmStatic
    external fun stop()
}

/**
 * What the worker calls, on its own thread. The Rust side calls these by name, so they keep their
 * names and signatures. None of them throws: a failure is an answer.
 */
interface Platform {
    /** The paired devices, as JSON: address, name, alias, connected, service UUIDs. */
    fun pairedDevices(): String

    /**
     * Opens the RFCOMM link to [service] on the device at [address], in place of any open one.
     * Null, or `{"kind", "message"}` with the kind `busy`, `not-connected`, `timeout` or `failed`.
     */
    fun connect(address: String, service: String): String?

    /** Null, or why the link failed. */
    fun send(bytes: ByteArray): String?

    /** The next bytes; null once [timeoutMs] passes; an empty array once the link is closed. */
    fun recv(timeoutMs: Long): ByteArray?

    fun close()

    /** One worker event, as JSON. */
    fun event(json: String)

    /** A session or debug line. */
    fun log(line: String)
}
