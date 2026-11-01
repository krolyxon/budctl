package io.github.krolyxon.budctl

import androidx.compose.runtime.Immutable
import org.json.JSONArray
import org.json.JSONObject

data class Charge(val level: Int, val charging: Boolean)

/** `off`, `transparency` or `anc`, and the noise-cancelling level when the buds report one. */
data class NoiseControl(val mode: String, val level: String?)

data class Switch(val key: String, val label: String, val on: Boolean)

data class EqBand(val hz: Int, val gain: Int)

data class EqPreset(val name: String, val id: Int, val custom: Boolean, val bands: List<EqBand>, val range: IntRange?)

data class EqPresets(val current: String?, val presets: List<EqPreset>)

data class Gesture(val side: String, val gesture: String, val action: String, val group: Int?)

data class GestureChoice(val gesture: String, val actions: List<String>)

data class TouchControls(
    val gestures: List<Gesture>,
    val holdCycle: List<String>?,
    val choices: List<GestureChoice>,
    val cycleModes: List<String>,
)

/** A phone or computer the buds keep for dual connection. [thisDevice]: the one that asked. */
data class DualDevice(
    val address: String,
    val name: String,
    val connected: Boolean,
    val thisDevice: Boolean,
    val mainAudio: Boolean,
    val playing: Boolean,
)

/**
 * The buds the screen shows: made-up ones for the demo, an empty pair until the real buds answer,
 * then what they report (the worker's `read` event, docs/android.md#boundary).
 */
@Immutable
data class Buds(
    val name: String,
    val connected: Boolean,
    val left: Charge? = null,
    val right: Charge? = null,
    val case: Charge? = null,
    val noise: NoiseControl? = null,
    /** The EQ preset in use: a built-in one's name, or `custom-<id>`. */
    val eq: String? = null,
    val switches: List<Switch> = emptyList(),
    val codec: String? = null,
    val firmware: String? = null,
    val presets: EqPresets? = null,
    val touch: TouchControls? = null,
    /** The levels the bass level takes, when the buds have one to set. */
    val bassRange: IntRange? = null,
    /** The bass level last set from this app: the buds do not report it. */
    val bass: Int? = null,
    val dual: List<DualDevice>? = null,
) {
    /** The name to show for the EQ preset in use (crates/budctl-gui/src/buds.rs, `eq_name`). */
    fun eqName(): String {
        val current = eq ?: return "Unknown"
        val preset = presets?.presets?.find { key(it) == current } ?: return words(current)
        return shownName(preset)
    }

    /** The buds as a reading says, keeping what the app knows and the buds do not report. */
    fun read(reading: JSONObject): Buds {
        val status = reading.getJSONObject("status")
        val battery = status.optJSONObject("battery")
        return copy(
            connected = true,
            left = battery?.optJSONObject("left")?.let(::charge),
            right = battery?.optJSONObject("right")?.let(::charge),
            case = battery?.optJSONObject("case")?.let(::charge),
            noise = status.optJSONObject("noise_control")?.let(::parseNoise),
            eq = status.text("eq"),
            switches = status.optJSONArray("switches").objects().map(::parseSwitch),
            codec = status.text("codec"),
            firmware = status.text("firmware"),
            presets = reading.optJSONObject("eq")?.let(::presets),
            touch = reading.optJSONObject("touch")?.let(::touch),
            bassRange = reading.optJSONArray("bass_range")?.let { it.getInt(0)..it.getInt(1) },
            dual = reading.optJSONArray("dual")?.objects()?.map(::dual),
        )
    }

    companion object {
        /** What the screen shows before the real buds answer. */
        val UNKNOWN = Buds(name = "Your buds", connected = false)

        /** The made-up buds of the demo: roughly what the owner's T310 reports. */
        val DEMO = Buds(
            name = "realme Buds T310",
            connected = true,
            left = Charge(80, false),
            right = Charge(80, false),
            noise = NoiseControl("anc", "deep"),
            eq = "custom-4",
            switches = listOf(
                Switch("game", "Game mode", false),
                Switch("dual", "Dual connection", true),
                Switch("wind", "Wind-noise reduction", true),
                Switch("spatial", "3D sound", true),
                Switch("bass-boost", "Bass boost", true),
            ),
            codec = "SBC",
            firmware = "1.1.0.43",
            presets = EqPresets(
                "custom-4",
                listOf("pure-raw", "deep-bass", "pure-vocals", "pure-bass").mapIndexed { id, name ->
                    EqPreset(name, id, false, emptyList(), null)
                } + EqPreset(
                    "Custom1",
                    4,
                    true,
                    listOf(EqBand(50, 3), EqBand(250, 1), EqBand(1000, 0), EqBand(4000, -1), EqBand(8000, 1), EqBand(16000, 2)),
                    -6..6,
                ),
            ),
            touch = TouchControls(
                gestures = listOf(
                    Gesture("left", "double-tap", "play-pause", null),
                    Gesture("left", "triple-tap", "previous", null),
                    Gesture("left", "hold", "noise-cycle", null),
                    Gesture("right", "double-tap", "play-pause", null),
                    Gesture("right", "triple-tap", "next", null),
                    Gesture("right", "hold", "noise-cycle", null),
                ),
                holdCycle = listOf("off", "anc"),
                choices = listOf(
                    GestureChoice("double-tap", listOf("none", "play-pause", "next", "voice-assistant")),
                    GestureChoice(
                        "triple-tap",
                        listOf("none", "previous", "next", "voice-assistant", "volume-up", "volume-down", "game-mode"),
                    ),
                    GestureChoice("hold", listOf("none", "noise-cycle", "voice-assistant", "game-mode")),
                ),
                cycleModes = listOf("off", "transparency", "anc"),
            ),
            bassRange = -5..5,
            dual = listOf(
                DualDevice("2B:1A:00:AA:BB:CC", "arch", connected = true, thisDevice = false, mainAudio = false, playing = false),
                DualDevice("11:22:33:44:55:66", "Phone", connected = true, thisDevice = true, mainAudio = true, playing = true),
            ),
        )

        /** The name a preset is chosen by: a built-in one's own, `custom-<id>` for a custom one. */
        fun key(preset: EqPreset) = if (preset.custom) "custom-${preset.id}" else preset.name

        /** A preset's name to show: a custom one's stored name, a built-in one's in words. */
        fun shownName(preset: EqPreset) = if (preset.custom) preset.name else words(preset.name)

        /** `pure-bass` → "Pure bass". */
        fun words(name: String) = name.replace('-', ' ').replaceFirstChar { it.uppercase() }
    }
}

/** A string field, or null when it is missing or JSON null (org.json's optString says "null"). */
private fun JSONObject.text(name: String): String? = if (isNull(name)) null else optString(name)

private fun JSONArray?.objects(): List<JSONObject> = if (this == null) emptyList() else List(length()) { getJSONObject(it) }

private fun JSONArray?.strings(): List<String> = if (this == null) emptyList() else List(length()) { getString(it) }

private fun charge(json: JSONObject) = Charge(json.getInt("level"), json.getBoolean("charging"))

/** A noise control, as a reading and the `noise` event give it. */
internal fun parseNoise(json: JSONObject) = NoiseControl(json.getString("mode"), json.text("level"))

/** A switch, as a reading and the `switch` event give it. */
internal fun parseSwitch(json: JSONObject) = Switch(json.getString("key"), json.getString("label"), json.getBoolean("on"))

private fun preset(json: JSONObject) = EqPreset(
    json.getString("name"),
    json.getInt("id"),
    json.getBoolean("custom"),
    json.optJSONArray("bands").objects().map { EqBand(it.getInt("hz"), it.getInt("gain")) },
    json.optJSONArray("range")?.let { it.getInt(0)..it.getInt(1) },
)

private fun presets(json: JSONObject) = EqPresets(json.text("current"), json.optJSONArray("presets").objects().map(::preset))

private fun touch(json: JSONObject) = TouchControls(
    gestures = json.optJSONArray("gestures").objects().map {
        Gesture(it.getString("side"), it.getString("gesture"), it.getString("action"), if (it.isNull("group")) null else it.getInt("group"))
    },
    holdCycle = json.optJSONArray("hold_cycle")?.strings(),
    choices = json.optJSONArray("choices").objects().map { GestureChoice(it.getString("gesture"), it.optJSONArray("actions").strings()) },
    cycleModes = json.optJSONArray("cycle_modes").strings(),
)

private fun dual(json: JSONObject) = DualDevice(
    json.getString("address"),
    json.getString("name"),
    json.getBoolean("connected"),
    json.getBoolean("this_computer"),
    json.getBoolean("main_audio"),
    json.getBoolean("playing"),
)
