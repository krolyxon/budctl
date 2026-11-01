package io.github.krolyxon.budctl

import android.annotation.SuppressLint
import android.bluetooth.BluetoothAdapter
import android.bluetooth.BluetoothDevice
import android.bluetooth.BluetoothManager
import android.bluetooth.BluetoothProfile
import android.content.Context
import java.io.IOException
import java.util.Timer
import java.util.UUID
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicBoolean
import kotlin.concurrent.schedule
import org.json.JSONArray
import org.json.JSONObject

/**
 * The phone's side of the worker (docs/android.md#bluetooth): the paired devices and one RFCOMM
 * link, used by the Rust worker on its own thread, and where its events and log lines go.
 * Written from Android's API reference.
 */
@SuppressLint("MissingPermission") // The activity starts the worker only once BLUETOOTH_CONNECT is granted.
class Bluetooth(
    context: Context,
    private val onEvent: (String) -> Unit,
    private val onLog: (String) -> Unit,
) : Platform {
    private val adapter: BluetoothAdapter? = context.getSystemService(BluetoothManager::class.java)?.adapter
    private val profiles = ConcurrentHashMap<Int, BluetoothProfile>()
    private val profilesReady = CountDownLatch(AUDIO_PROFILES.size)

    @Volatile
    private var link: Link? = null

    init {
        val listener = object : BluetoothProfile.ServiceListener {
            override fun onServiceConnected(profile: Int, proxy: BluetoothProfile) {
                profiles[profile] = proxy
                profilesReady.countDown()
            }

            override fun onServiceDisconnected(profile: Int) {
                profiles.remove(profile)
            }
        }
        for (profile in AUDIO_PROFILES) {
            if (adapter?.getProfileProxy(context, listener, profile) != true) profilesReady.countDown()
        }
    }

    /** Whether the buds' audio link is up: their control channel only opens then. */
    private fun connected(device: BluetoothDevice): Boolean {
        // The proxies come soon after start; this runs on the worker's thread, so it may wait.
        profilesReady.await(PROFILE_WAIT_MS, TimeUnit.MILLISECONDS)
        return profiles.values.any { it.getConnectionState(device) == BluetoothProfile.STATE_CONNECTED }
    }

    override fun pairedDevices(): String {
        val list = JSONArray()
        for (device in adapter?.bondedDevices.orEmpty()) {
            list.put(
                JSONObject()
                    .put("address", device.address)
                    .put("name", device.name ?: "")
                    .put("alias", device.alias ?: device.name ?: device.address)
                    .put("connected", connected(device))
                    .put("uuids", JSONArray(device.uuids.orEmpty().map { it.uuid.toString() })),
            )
        }
        return list.toString()
    }

    override fun connect(address: String, service: String): String? {
        close()
        val adapter = adapter ?: return failure("not-connected", "this phone has no Bluetooth")
        if (!adapter.isEnabled) return failure("not-connected", "Bluetooth is off")
        val device = adapter.getRemoteDevice(address)
        val socket = try {
            device.createRfcommSocketToServiceRecord(UUID.fromString(service))
        } catch (e: IOException) {
            return failure("failed", "could not make an RFCOMM socket: ${e.message}")
        }
        // connect() takes no timeout: a watchdog closes the socket after 8 s, when budctl gives
        // up on Linux too.
        val timedOut = AtomicBoolean(false)
        val watchdog = Timer("budctl-connect", true)
        watchdog.schedule(CONNECT_TIMEOUT_MS) {
            timedOut.set(true)
            quietly { socket.close() }
        }
        try {
            socket.connect()
        } catch (e: IOException) {
            quietly { socket.close() }
            val detail = e.message ?: "refused"
            return when {
                timedOut.get() -> failure("timeout", "the control channel did not connect in 8 s")
                // The audio link is up but the control channel refused: someone else holds it.
                connected(device) -> failure("busy", detail)
                else -> failure("not-connected", "the buds are not connected ($detail)")
            }
        } finally {
            watchdog.cancel()
        }
        link = Link(socket.inputStream, socket.outputStream, socket)
        return null
    }

    override fun send(bytes: ByteArray): String? {
        val link = link ?: return "no link is open"
        return try {
            link.send(bytes)
            null
        } catch (e: IOException) {
            e.message ?: "the link failed"
        }
    }

    override fun recv(timeoutMs: Long): ByteArray? {
        val link = link ?: return ByteArray(0)
        return link.recv(timeoutMs)
    }

    override fun close() {
        link?.close()
        link = null
    }

    override fun event(json: String) = onEvent(json)

    override fun log(line: String) = onLog(line)

    /** Lets go of everything, once the app is done with Bluetooth. */
    fun release() {
        close()
        for ((profile, proxy) in profiles) adapter?.closeProfileProxy(profile, proxy)
        profiles.clear()
    }

    private fun failure(kind: String, message: String): String =
        JSONObject().put("kind", kind).put("message", message).toString()

    private inline fun quietly(block: () -> Unit) {
        try {
            block()
        } catch (_: IOException) {
            // Closing what is already closed.
        }
    }

    private companion object {
        val AUDIO_PROFILES = listOf(BluetoothProfile.A2DP, BluetoothProfile.HEADSET)
        const val PROFILE_WAIT_MS = 2_000L
        const val CONNECT_TIMEOUT_MS = 8_000L
    }
}
