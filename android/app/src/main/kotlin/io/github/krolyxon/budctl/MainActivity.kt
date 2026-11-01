package io.github.krolyxon.budctl

import android.Manifest
import android.content.ClipData
import android.content.ClipboardManager
import android.content.pm.PackageManager
import android.os.Bundle
import android.util.Log
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicText
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import org.json.JSONObject

/**
 * The plain screen of step 4 (docs/android.md#steps): what the buds report, the noise-control
 * choices, and the log, for checkpoints A1 and A2. The real pages come in step 5.
 *
 * The worker runs while the app is on screen: it starts in onStart and stops in onStop, so the
 * control channel is free for the laptop once the app is left.
 */
class MainActivity : ComponentActivity() {
    private lateinit var bluetooth: Bluetooth

    private var permitted by mutableStateOf(false)
    private var debug by mutableStateOf(false)
    private var name by mutableStateOf<String?>(null)
    private var status by mutableStateOf<JSONObject?>(null)
    private var waiting by mutableStateOf(false)
    private var problem by mutableStateOf<String?>(null)
    private val log = mutableStateListOf<String>()

    private val askPermission = registerForActivityResult(ActivityResultContracts.RequestPermission()) { granted ->
        permitted = granted
        if (granted) startWorker()
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        bluetooth = Bluetooth(
            this,
            onEvent = { json -> runOnUiThread { answer(JSONObject(json)) } },
            onLog = { line ->
                Log.i(TAG, line)
                runOnUiThread { addLine(line) }
            },
        )
        debug = getSharedPreferences(PREFERENCES, MODE_PRIVATE).getBoolean("debug", false)
        permitted = checkSelfPermission(Manifest.permission.BLUETOOTH_CONNECT) == PackageManager.PERMISSION_GRANTED
        if (!permitted) askPermission.launch(Manifest.permission.BLUETOOTH_CONNECT)
        setContent { Screen() }
    }

    override fun onStart() {
        super.onStart()
        if (permitted) startWorker()
    }

    override fun onStop() {
        super.onStop()
        Core.stop()
    }

    override fun onDestroy() {
        super.onDestroy()
        bluetooth.release()
    }

    private fun startWorker() {
        val failure = Core.start(bluetooth, IDLE_MS, debug)
        if (failure != null) problem = failure else send(JSONObject().put("command", "read"))
    }

    private fun send(command: JSONObject) {
        waiting = true
        Core.command(command.toString())?.let {
            problem = it
            waiting = false
        }
    }

    private fun noise(mode: String) = send(JSONObject().put("command", "noise").put("mode", mode))

    /** One event from the worker, on the main thread. */
    private fun answer(event: JSONObject) {
        when (event.getString("event")) {
            "found" -> name = event.getString("name")
            "lost" -> problem = event.getString("message")
            else -> {
                waiting = false
                when (event.getString("event")) {
                    "read" -> {
                        status = event.getJSONObject("status")
                        problem = null
                    }
                    "noise" -> {
                        val now = JSONObject().put("mode", event.getString("mode")).put("level", event.opt("level"))
                        status = JSONObject(status?.toString() ?: "{}").put("noise_control", now)
                        problem = null
                    }
                    "failed" -> problem = event.getString("message")
                }
            }
        }
    }

    private fun addLine(line: String) {
        log.add(line)
        if (log.size > MAX_LINES) log.removeRange(0, log.size - MAX_LINES)
    }

    private fun toggleDebug() {
        debug = !debug
        getSharedPreferences(PREFERENCES, MODE_PRIVATE).edit().putBoolean("debug", debug).apply()
        Core.stop()
        if (permitted) startWorker()
    }

    private fun copyLog() {
        val clipboard = getSystemService(ClipboardManager::class.java)
        clipboard.setPrimaryClip(ClipData.newPlainText("budctl log", log.joinToString("\n")))
    }

    @Composable
    private fun Screen() {
        Column(
            Modifier
                .fillMaxSize()
                .background(Color.Black)
                .safeDrawingPadding()
                .verticalScroll(rememberScrollState())
                .padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(14.dp),
        ) {
            BasicText(name ?: "budctl", style = TEXT.copy(fontSize = 26.sp))
            if (!permitted) {
                BasicText("budctl needs the Nearby devices permission to reach the buds.", style = TEXT)
                Chip("Allow") { askPermission.launch(Manifest.permission.BLUETOOTH_CONNECT) }
                return@Column
            }
            problem?.let { BasicText(it, style = TEXT.copy(color = RED)) }
            if (waiting) BasicText("Working…", style = DIM)
            status?.let { Status(it) }
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Chip("Off") { noise("off") }
                Chip("Transparency") { noise("transparency") }
                Chip("ANC") { noise("anc") }
            }
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Chip("Read") { send(JSONObject().put("command", "read")) }
                Chip(if (debug) "Debug on" else "Debug off") { toggleDebug() }
                Chip("Copy log") { copyLog() }
            }
            BasicText(log.joinToString("\n"), style = DIM.copy(fontFamily = FontFamily.Monospace, fontSize = 11.sp))
        }
    }

    @Composable
    private fun Status(status: JSONObject) {
        val battery = status.optJSONObject("battery")
        val charges = listOf("left" to "L", "right" to "R", "case" to "Case").mapNotNull { (key, label) ->
            battery?.optJSONObject(key)?.let { charge ->
                "$label ${charge.getInt("level")}%" + if (charge.getBoolean("charging")) " (charging)" else ""
            }
        }
        val lines = mutableListOf("Battery: " + charges.joinToString(" · ").ifEmpty { "unknown" })
        status.optJSONObject("noise_control")?.let { noise ->
            lines += "Noise control: " + noise.getString("mode") +
                (noise.optString("level").takeUnless { it.isEmpty() || it == "null" }?.let { " ($it)" } ?: "")
        }
        status.optString("eq").takeUnless { it.isEmpty() || it == "null" }?.let { lines += "EQ: $it" }
        status.optJSONArray("switches")?.let { switches ->
            for (i in 0 until switches.length()) {
                val switch = switches.getJSONObject(i)
                lines += switch.getString("label") + ": " + if (switch.getBoolean("on")) "on" else "off"
            }
        }
        status.optString("codec").takeUnless { it.isEmpty() || it == "null" }?.let { lines += "Codec: $it" }
        status.optString("firmware").takeUnless { it.isEmpty() || it == "null" }?.let { lines += "Firmware: $it" }
        BasicText(lines.joinToString("\n"), style = TEXT)
    }

    @Composable
    private fun Chip(label: String, onClick: () -> Unit) {
        BasicText(
            label,
            style = TEXT.copy(color = Color.Black),
            modifier = Modifier
                .clip(RoundedCornerShape(50))
                .background(Color.White)
                .clickable(onClick = onClick)
                .padding(horizontal = 16.dp, vertical = 10.dp),
        )
    }

    private companion object {
        const val TAG = "budctl"
        const val PREFERENCES = "budctl"
        /** How long a session stays open after the last command, as on the desktop. */
        const val IDLE_MS = 10_000L
        const val MAX_LINES = 1_000
        val TEXT = TextStyle(color = Color.White, fontSize = 15.sp)
        val DIM = TEXT.copy(color = Color(0xFF9A9A9A))
        val RED = Color(0xFFFF5A4F)
    }
}
