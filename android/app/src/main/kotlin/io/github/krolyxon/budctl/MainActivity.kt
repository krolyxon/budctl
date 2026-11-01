package io.github.krolyxon.budctl

import android.Manifest
import android.content.pm.PackageManager
import android.graphics.Color
import android.os.Bundle
import android.util.Log
import androidx.activity.ComponentActivity
import androidx.activity.SystemBarStyle
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import org.json.JSONObject

/**
 * The app's one activity. It wires the screen ([App], [Screen]) to the Rust worker ([Core]) and
 * the phone's Bluetooth ([Bluetooth]).
 *
 * The worker runs while the app is on screen: it starts in onStart and stops in onStop, so the
 * control channel is free for the laptop once the app is left (docs/android.md#bluetooth).
 *
 * `adb shell am start -n io.github.krolyxon.budctl/.MainActivity --ez demo true` shows made-up
 * buds instead, with no Bluetooth: for the emulator and for screenshots.
 */
class MainActivity : ComponentActivity() {
    private lateinit var app: App
    private var bluetooth: Bluetooth? = null

    private val askPermission = registerForActivityResult(ActivityResultContracts.RequestPermission()) { granted ->
        app.permitted = granted
        if (granted) startWorker()
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge(SystemBarStyle.dark(Color.TRANSPARENT), SystemBarStyle.dark(Color.TRANSPARENT))
        val demo = intent.getBooleanExtra("demo", false)
        app = App(
            demo,
            getSharedPreferences(PREFERENCES, MODE_PRIVATE),
            send = { Core.command(it) },
            restart = {
                Core.stop()
                startWorker()
            },
        )
        if (!demo) {
            bluetooth = Bluetooth(
                this,
                onEvent = { json -> runOnUiThread { app.answer(JSONObject(json)) } },
                onLog = { line ->
                    Log.i(TAG, line)
                    runOnUiThread { app.addLine(line) }
                },
            )
            app.permitted = checkSelfPermission(Manifest.permission.BLUETOOTH_CONNECT) == PackageManager.PERMISSION_GRANTED
            if (!app.permitted) askPermission.launch(Manifest.permission.BLUETOOTH_CONNECT)
        }
        setContent { Screen(app) { askPermission.launch(Manifest.permission.BLUETOOTH_CONNECT) } }
    }

    override fun onStart() {
        super.onStart()
        if (!app.demo && app.permitted) startWorker()
    }

    override fun onStop() {
        super.onStop()
        if (!app.demo) Core.stop()
    }

    override fun onDestroy() {
        super.onDestroy()
        bluetooth?.release()
    }

    /** Starts the worker and reads the buds. */
    private fun startWorker() {
        val bluetooth = bluetooth ?: return
        Core.start(bluetooth, IDLE_MS, app.debug)?.let { failure ->
            app.connection = Connection.Failed("failed", failure)
            return
        }
        app.read()
    }

    private companion object {
        const val TAG = "budctl"
        const val PREFERENCES = "budctl"

        /** How long a session stays open after the last command, as on the desktop. */
        const val IDLE_MS = 10_000L
    }
}
