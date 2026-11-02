package io.github.krolyxon.budctl

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp

/** The Overview page (crates/budctl-gui/src/overview.rs): battery, noise control, the switches. */
@Composable
fun Overview(app: App, askPermission: () -> Unit, open: (Page) -> Unit) {
    if (!app.permitted) {
        Note("budctl needs the Nearby devices permission to reach the buds.")
        Row(Modifier.padding(horizontal = 18.dp)) { Pill("Allow", filled = true, onClick = askPermission) }
        return
    }
    val buds = app.buds
    Battery(buds)
    buds.noise?.let { NoiseChoice(app, it) }
    if (buds.switches.isNotEmpty()) Switches(app)
    More(app, open)
}

/** The rows that open the other pages. */
@Composable
private fun More(app: App, open: (Page) -> Unit) {
    Card {
        SettingRow(Icon.Equalizer, "Equalizer", app.buds.eqName(), onClick = { open(Page.Equalizer) }) { Chevron() }
        SettingRow(Icon.Gestures, "Gestures", "Taps and holds on each bud", onClick = { open(Page.Gestures) }) { Chevron() }
        SettingRow(Icon.Earbud, "Earbud settings", "Dual connection, find my earbuds, firmware", onClick = { open(Page.Earbuds) }) {
            Chevron()
        }
    }
}

@Composable
private fun Battery(buds: Buds) {
    val p = LocalLook.current.p
    Card {
        Row(Modifier.padding(horizontal = 4.dp, vertical = 20.dp)) {
            for ((side, charge) in listOf("left" to buds.left, "case" to buds.case, "right" to buds.right)) {
                Column(
                    Modifier.weight(1f),
                    horizontalAlignment = Alignment.CenterHorizontally,
                    verticalArrangement = Arrangement.spacedBy(12.dp),
                ) {
                    Ring(charge?.level, Modifier.padding(horizontal = 8.dp).fillMaxWidth())
                    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                        // Charging: the bolt, and the side in the live color.
                        if (charge?.charging == true) Glyph(Icon.Bolt, 13.dp, p.live)
                        Label(side, color = if (charge?.charging == true) p.live else p.grey)
                    }
                }
            }
        }
    }
}

@Composable
private fun NoiseChoice(app: App, now: NoiseControl) {
    val p = LocalLook.current.p
    val modes = listOf(
        Triple(Icon.NoiseOff, "Off", "off"),
        Triple(Icon.Anc, "Noise cancelling", "anc"),
        Triple(Icon.Transparency, "Transparency", "transparency"),
    )
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Segmented(modes.map { (icon, label, mode) -> Segment(label, now.mode == mode, icon) { app.noise(mode) } }, large = true)
        if (app.saving(Item.Noise)) {
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.Center) { Label(SAVING, color = p.live) }
        }
        // The strength, when noise cancelling is on and the buds report its level.
        if (now.mode == "anc" && now.level != null) {
            val levels = listOf("light" to "Light", "medium" to "Medium", "deep" to "Deep")
            Row(Modifier.padding(start = 18.dp), verticalAlignment = Alignment.CenterVertically) {
                Label("strength", modifier = Modifier.weight(1f))
                Segmented(
                    levels.map { (level, label) -> Segment(label, now.level == level) { app.noise("anc", level) } },
                    large = false,
                    Modifier.width(240.dp),
                )
            }
        }
    }
}

/** How the page shows the switches it knows, in this order; others come last, plain. */
private val SWITCHES = listOf(
    Triple("game", Icon.Game, "Lower latency, for games and video"),
    Triple("spatial", Icon.Spatial, "Wider, surround-like sound"),
    Triple("bass-boost", Icon.Bass, "Stronger low end"),
    Triple("wind", Icon.Wind, "Less wind noise outdoors"),
    Triple("dual", Icon.Dual, "Stay connected to two devices at once"),
)

@Composable
private fun Switches(app: App) {
    val buds = app.buds
    val rank = { s: Switch -> SWITCHES.indexOfFirst { it.first == s.key }.let { if (it < 0) SWITCHES.size else it } }
    Card {
        for (s in buds.switches.sortedBy(rank)) {
            val known = SWITCHES.find { it.first == s.key }
            SettingRow(
                known?.second ?: Icon.Settings,
                s.label,
                if (app.saving(Item.Switch(s.key))) SAVING else known?.third,
                onClick = { app.switch(s.key, !s.on) },
            ) { Toggle(s.on) }
            if (s.key == "bass-boost") buds.bassRange?.let { range -> for ((band, title) in BASS_BANDS) BassLevel(app, range, band, title) }
        }
    }
}

/** The bands of bass boost, as the buds and the rows name them. */
private val BASS_BANDS = listOf("low" to "Low frequencies", "medium" to "Medium frequencies", "high" to "High frequencies")

/** One band of bass boost: the buds take its level but do not report it, so the row shows the last level set here. */
@Composable
private fun BassLevel(app: App, range: IntRange, band: String, title: String) {
    val level = app.bassDraft[band] ?: app.buds.bass[band]
    SettingRow(
        Icon.Bass,
        title,
        if (app.saving(Item.Bass(band))) SAVING else "Bass boost, last set here",
        below = {
            DotSlider(
                range.first,
                range.last,
                level ?: 0,
                change = { app.moveBass(band, it) },
                release = { app.saveBass(band) },
                modifier = Modifier.padding(start = 52.dp, top = 6.dp),
            )
        },
    ) { ValueChip(level?.let(::gain) ?: "not set") }
}

/** A gain or level with its sign: "+3", "0", "-1". */
fun gain(value: Int) = if (value == 0) "0" else "%+d".format(value)
