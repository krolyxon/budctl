package io.github.krolyxon.budctl

import android.content.SharedPreferences
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import org.json.JSONObject

/** The pages, in order: the dot-matrix word and the caption above the page. */
enum class Page(val word: String, val caption: String) {
    Overview("overview", "overview · battery, noise control, sound"),
    App("app", "app · colors, motion, debug log"),
}

/** What a write is for. While one is on its way, its row says "Saving…" and takes no other. */
sealed interface Item {
    data object Read : Item

    data object Noise : Item

    data class Switch(val key: String) : Item

    data object Bass : Item
}

/** Where the app stands with the buds. */
sealed interface Connection {
    data object Demo : Connection

    data object Connecting : Connection

    data object Live : Connection

    /** `kind` as the worker names it: `usage`, `not-found`, `busy` or `failed`. */
    data class Failed(val kind: String, val message: String) : Connection
}

/**
 * What the screen shows and what a touch does, as the desktop app's `update` (crates/budctl-gui):
 * every change goes to the worker as a command, and the buds' read-back, the worker's answer,
 * changes the screen. With the demo buds, a change shows at once.
 */
class App(
    val demo: Boolean,
    private val prefs: SharedPreferences,
    /** Queues a command for the worker: null, or why it was not queued. */
    private val send: (String) -> String?,
    /** Starts the worker again, as after the debug switch. */
    private val restart: () -> Unit,
) {
    var buds by mutableStateOf(if (demo) Buds.DEMO else Buds.UNKNOWN)
    var connection by mutableStateOf<Connection>(if (demo) Connection.Demo else Connection.Connecting)
    var permitted by mutableStateOf(demo)
    var debug by mutableStateOf(prefs.getBoolean(DEBUG, false))
        private set
    var materialYou by mutableStateOf(prefs.getBoolean(MATERIAL_YOU, true))
        private set
    var motion by mutableStateOf(prefs.getBoolean(MOTION, true))
        private set

    /** The noise-cancelling level to ask for: the buds' own, once they report one. */
    var ancLevel by mutableStateOf("deep")
        private set

    /** A bass level moved on the screen and not set on the buds yet. */
    var bassDraft by mutableStateOf<Int?>(null)
        private set

    /** "Turn dual connection off?" is open. */
    var confirmDual by mutableStateOf(false)
        private set

    /** The commands sent and not answered yet, oldest first: each gets one event, in order. */
    private val inFlight = mutableStateListOf<Item>()

    /** The worker's log lines, the debug dump with them. */
    val log = mutableStateListOf<String>()

    val problem: String? get() = (connection as? Connection.Failed)?.message

    fun saving(item: Item) = item in inFlight

    fun read() {
        if (connection is Connection.Failed) connection = Connection.Connecting
        command(Item.Read, JSONObject().put("command", "read"))
    }

    /** [mode] `off`, `transparency` or `anc`; [level] for `anc`, else the level the buds are at. */
    fun noise(mode: String, level: String? = null) {
        val json = JSONObject().put("command", "noise").put("mode", mode)
        if (mode == "anc") json.put("level", level ?: ancLevel)
        command(Item.Noise, json)
    }

    fun switch(key: String, on: Boolean) {
        // Turning dual connection off may drop one of two connected devices: ask first.
        if (key == DUAL && !on) {
            confirmDual = true
            return
        }
        command(Item.Switch(key), JSONObject().put("command", "switch").put("key", key).put("on", on))
    }

    fun answerDual(yes: Boolean) {
        confirmDual = false
        if (yes) command(Item.Switch(DUAL), JSONObject().put("command", "switch").put("key", DUAL).put("on", false))
    }

    /** The bass level slider moved: shown at once, written when the finger lifts ([saveBass]). */
    fun moveBass(level: Int) {
        bassDraft = level
    }

    fun saveBass() {
        val level = bassDraft ?: return
        if (level == buds.bass) {
            bassDraft = null
            return
        }
        command(Item.Bass, JSONObject().put("command", "bass").put("level", level))
    }

    private fun command(item: Item, json: JSONObject) {
        if (item != Item.Read && item in inFlight) return
        if (demo) return pretend(item, json)
        inFlight += item
        send(json.toString())?.let { failure ->
            inFlight.remove(item)
            connection = Connection.Failed("failed", failure)
        }
    }

    /** One event from the worker (docs/android.md#boundary), on the main thread. */
    fun answer(event: JSONObject) {
        when (event.getString("event")) {
            "found" -> buds = buds.copy(name = event.getString("name"))
            "lost" -> {
                connection = Connection.Failed("not-found", event.getString("message"))
                buds = buds.copy(connected = false)
            }
            else -> {
                val item = inFlight.removeFirstOrNull()
                when (event.getString("event")) {
                    "read" -> {
                        buds = buds.read(event)
                        buds.noise?.level?.let { ancLevel = it }
                        connection = Connection.Live
                    }
                    "noise" -> {
                        val now = parseNoise(event)
                        buds = buds.copy(noise = now)
                        now.level?.let { ancLevel = it }
                        connection = Connection.Live
                    }
                    "switch" -> {
                        val now = parseSwitch(event.getJSONObject("switch"))
                        buds = buds.copy(switches = buds.switches.map { if (it.key == now.key) now else it })
                        connection = Connection.Live
                    }
                    "bass" -> {
                        val level = event.getInt("level")
                        buds = buds.copy(bass = level)
                        if (bassDraft == level) bassDraft = null
                        connection = Connection.Live
                    }
                    "failed" -> {
                        val kind = event.getString("kind")
                        connection = Connection.Failed(kind, event.getString("message"))
                        if (kind == "not-found") buds = buds.copy(connected = false)
                        if (item == Item.Bass) bassDraft = null
                    }
                }
            }
        }
    }

    /** The demo buds take a change at once. */
    private fun pretend(item: Item, json: JSONObject) {
        when (item) {
            Item.Read -> {}
            Item.Noise -> {
                val mode = json.getString("mode")
                val level = if (mode == "anc") json.getString("level") else buds.noise?.level
                buds = buds.copy(noise = NoiseControl(mode, level))
                level?.let { ancLevel = it }
            }
            is Item.Switch -> {
                val on = json.getBoolean("on")
                buds = buds.copy(switches = buds.switches.map { if (it.key == item.key) it.copy(on = on) else it })
            }
            Item.Bass -> {
                buds = buds.copy(bass = json.getInt("level"))
                bassDraft = null
            }
        }
    }

    fun addLine(line: String) {
        log.add(line)
        if (log.size > MAX_LINES) log.removeRange(0, log.size - MAX_LINES)
    }

    fun toggleDebug() {
        debug = !debug
        prefs.edit().putBoolean(DEBUG, debug).apply()
        if (!demo) restart()
    }

    fun toggleMaterialYou() {
        materialYou = !materialYou
        prefs.edit().putBoolean(MATERIAL_YOU, materialYou).apply()
    }

    fun toggleMotion() {
        motion = !motion
        prefs.edit().putBoolean(MOTION, motion).apply()
    }

    private companion object {
        const val DUAL = "dual"
        const val DEBUG = "debug"
        const val MATERIAL_YOU = "material_you"
        const val MOTION = "motion"
        const val MAX_LINES = 1_000
    }
}
