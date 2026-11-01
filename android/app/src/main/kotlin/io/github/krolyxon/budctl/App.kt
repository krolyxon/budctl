package io.github.krolyxon.budctl

import android.content.SharedPreferences
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import org.json.JSONArray
import org.json.JSONObject

/** The pages, in order: the dot-matrix word and the caption above the page. */
enum class Page(val word: String, val caption: String) {
    Overview("overview", "overview · battery, noise control, sound"),
    Equalizer("eq", "equalizer · presets, and the bands of yours"),
    App("app", "app · colors, motion, debug log"),
}

/** What a write is for. While one is on its way, its row says "Saving…" and takes no other. */
sealed interface Item {
    data object Read : Item

    data object Noise : Item

    data class Switch(val key: String) : Item

    data object Bass : Item

    /** A preset chosen, saved, renamed or deleted. */
    data class Preset(val id: Int) : Item

    data object NewPreset : Item
}

/** A custom preset's bands as moved on the screen, not saved on the buds yet. */
data class Draft(val preset: Int, val bands: List<EqBand>)

/** The name asked for a new preset, or for renaming the preset [renaming]. */
data class NamePrompt(val renaming: Int?, val text: String, val problem: String? = null)

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

    /** The custom preset whose bands are open. */
    var editing by mutableStateOf<Int?>(null)
        private set

    var draft by mutableStateOf<Draft?>(null)
        private set

    /** "Delete <preset>?" is open, for this preset. */
    var confirmDelete by mutableStateOf<Int?>(null)
        private set

    var prompt by mutableStateOf<NamePrompt?>(null)
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

    fun selectPreset(id: Int) {
        val preset = buds.preset(id) ?: return
        if (buds.eq == Buds.key(preset)) return
        val json = if (preset.custom) customEq(preset, emptyList()) else JSONObject().put("command", "eq").put("preset", preset.name)
        command(Item.Preset(id), json)
    }

    /** Opens a custom preset's bands, or closes them. */
    fun editPreset(id: Int) {
        editing = if (editing == id) null else id
    }

    fun doneEditing() {
        editing = null
    }

    /** Moves a band on the screen, within the preset's range. Nothing goes to the buds yet. */
    fun moveBand(id: Int, hz: Int, gain: Int) {
        val preset = buds.preset(id) ?: return
        val range = preset.range ?: Int.MIN_VALUE..Int.MAX_VALUE
        val bands = draft?.takeIf { it.preset == id }?.bands ?: preset.bands
        draft = Draft(id, bands.map { if (it.hz == hz) it.copy(gain = gain.coerceIn(range)) else it })
    }

    /** Saves the moved bands. If a write to the preset is on its way, they go after it. */
    fun saveBands(id: Int) {
        val bands = draft?.takeIf { it.preset == id }?.bands ?: return
        val preset = buds.preset(id) ?: return
        if (Item.Preset(id) !in inFlight) command(Item.Preset(id), customEq(preset, bands))
    }

    fun deletePreset(id: Int) {
        confirmDelete = id
    }

    fun answerDelete(yes: Boolean) {
        val id = confirmDelete
        confirmDelete = null
        val preset = id?.let(buds::preset) ?: return
        if (!yes) return
        if (editing == id) editing = null
        command(Item.Preset(preset.id), JSONObject().put("command", "delete-eq").put("preset", Buds.key(preset)))
    }

    fun newPreset() {
        prompt = NamePrompt(null, "")
    }

    fun renamePreset(id: Int) {
        prompt = NamePrompt(id, buds.preset(id)?.name ?: "")
    }

    fun typeName(text: String) {
        prompt = prompt?.copy(text = text, problem = null)
    }

    fun cancelName() {
        prompt = null
    }

    /** Creates or renames the preset, if the name will do. What the buds refuse comes back as an error. */
    fun submitName() {
        val asked = prompt ?: return
        val text = asked.text
        val taken = buds.presets?.presets.orEmpty().any { it.custom && it.name == text && it.id != asked.renaming }
        val problem = when {
            text.isEmpty() || text.trim() != text -> "A name cannot be empty, or start or end with a space."
            text.any(Char::isISOControl) -> "A name cannot hold control characters."
            taken -> "A preset is called $text already."
            else -> null
        }
        if (problem != null) {
            prompt = asked.copy(problem = problem)
            return
        }
        prompt = null
        val id = asked.renaming
        if (id == null) {
            command(Item.NewPreset, JSONObject().put("command", "create-eq").put("name", text))
        } else {
            val preset = buds.preset(id) ?: return
            if (preset.name != text) {
                command(Item.Preset(id), JSONObject().put("command", "rename-eq").put("preset", Buds.key(preset)).put("name", text))
            }
        }
    }

    private fun customEq(preset: EqPreset, bands: List<EqBand>) = JSONObject()
        .put("command", "custom-eq")
        .put("preset", Buds.key(preset))
        .put("bands", JSONArray(bands.map { JSONObject().put("hz", it.hz).put("gain", it.gain) }))

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
                    "eq" -> {
                        buds = buds.withEq(event.getString("preset"))
                        connection = Connection.Live
                    }
                    "custom-eq" -> {
                        val saved = parsePreset(event.getJSONObject("preset"))
                        buds = buds.withCustom(saved)
                        connection = Connection.Live
                        // Bands moved while this write was on its way go out now.
                        draft?.takeIf { it.preset == saved.id }?.let {
                            if (it.bands == saved.bands) draft = null else saveBands(saved.id)
                        }
                    }
                    "presets" -> {
                        buds = buds.withPresets(parsePresets(event.getJSONObject("eq")))
                        connection = Connection.Live
                        // A new preset opens its bands: it is the one in use.
                        if (item == Item.NewPreset) editing = buds.inUse()?.id
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
                        if (item is Item.Preset && draft?.preset == item.id) draft = null
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
            is Item.Preset, Item.NewPreset -> pretendEq(json)
        }
    }

    /** The demo buds' presets, changed as the real ones would be. */
    private fun pretendEq(json: JSONObject) {
        val presets = buds.presets?.presets.orEmpty()
        when (json.getString("command")) {
            "eq" -> buds = buds.withEq(json.getString("preset"))
            "custom-eq" -> {
                val key = json.getString("preset")
                val preset = presets.find { Buds.key(it) == key } ?: return
                val bands = json.getJSONArray("bands").let { a -> List(a.length()) { a.getJSONObject(it) } }
                    .associate { it.getInt("hz") to it.getInt("gain") }
                buds = buds.withCustom(preset.copy(bands = preset.bands.map { it.copy(gain = bands[it.hz] ?: it.gain) }))
                draft = null
            }
            "create-eq" -> {
                val id = (presets.maxOfOrNull { it.id } ?: 3) + 1
                val flat = (presets.firstOrNull { it.custom }?.bands ?: DEMO_BANDS).map { it.copy(gain = 0) }
                buds = buds.withCustom(EqPreset(json.getString("name"), id, true, flat, -6..6))
                editing = id
            }
            "rename-eq" -> {
                val key = json.getString("preset")
                val preset = presets.find { Buds.key(it) == key } ?: return
                buds = buds.withCustom(preset.copy(name = json.getString("name")))
            }
            "delete-eq" -> {
                val key = json.getString("preset")
                val left = presets.filterNot { Buds.key(it) == key }
                val current = buds.eq.takeUnless { it == key } ?: left.first().name
                buds = buds.withPresets(EqPresets(current, left))
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

        /** The bands a demo preset starts with when there is no custom one to copy. */
        val DEMO_BANDS = listOf(50, 250, 1000, 4000, 8000, 16000).map { EqBand(it, 0) }
    }
}
