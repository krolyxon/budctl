package io.github.krolyxon.budctl

import android.Manifest
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.content.pm.PackageInstaller
import android.content.pm.PackageManager
import android.graphics.Color
import android.os.Build
import android.os.Bundle
import android.util.Log
import androidx.activity.ComponentActivity
import androidx.activity.SystemBarStyle
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.core.content.ContextCompat
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
    private lateinit var updater: Updater
    private var bluetooth: Bluetooth? = null

    /** Android's installer answers here: it asks the owner first, then replaces the app. */
    private val installResult = object : BroadcastReceiver() {
        override fun onReceive(context: Context, intent: Intent) {
            when (intent.getIntExtra(PackageInstaller.EXTRA_STATUS, PackageInstaller.STATUS_FAILURE)) {
                PackageInstaller.STATUS_PENDING_USER_ACTION -> {
                    val confirm = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                        intent.getParcelableExtra(Intent.EXTRA_INTENT, Intent::class.java)
                    } else {
                        @Suppress("DEPRECATION")
                        intent.getParcelableExtra(Intent.EXTRA_INTENT)
                    }
                    confirm?.let { startActivity(it) }
                }
                // Android replaces the app, and stops this one.
                PackageInstaller.STATUS_SUCCESS -> {}
                else -> updater.failed(intent.getStringExtra(PackageInstaller.EXTRA_STATUS_MESSAGE))
            }
        }
    }

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
            updates = object : UpdateActions {
                override fun check(automatic: Boolean) = updater.check(automatic)

                override fun download(release: Release) = updater.download(release)
            },
        )
        val installed = packageManager.getPackageInfo(packageName, 0).longVersionCode.toInt()
        updater = Updater(this, installed) { state, automatic -> app.updateState(state, automatic, System.currentTimeMillis()) }
        // Not exported: on Android 12, AndroidX guards it with a permission only budctl holds, so no
        // other app can hand it an intent to start.
        ContextCompat.registerReceiver(this, installResult, IntentFilter(Updater.INSTALL_RESULT), ContextCompat.RECEIVER_NOT_EXPORTED)
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
        app.checkForUpdateIfDue(System.currentTimeMillis())
    }

    override fun onStop() {
        super.onStop()
        if (!app.demo) Core.stop()
    }

    override fun onDestroy() {
        super.onDestroy()
        unregisterReceiver(installResult)
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
