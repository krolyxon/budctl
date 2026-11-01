package io.github.krolyxon.budctl

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicText
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/** The Equalizer page (crates/budctl-gui/src/equalizer.rs): the presets, and the bands of the custom ones. */
@Composable
fun Equalizer(app: App) {
    val p = LocalLook.current.p
    val buds = app.buds
    val presets = buds.presets
    if (presets == null) {
        Note("These buds did not list their presets.")
        return
    }
    Card {
        for (preset in presets.presets) {
            val id = preset.id
            val chosen = buds.eq == Buds.key(preset)
            val open = preset.custom && app.editing == id
            val subtitle = when {
                app.saving(Item.Preset(id)) -> SAVING
                chosen && open -> "In use, open below"
                chosen -> "In use"
                open -> "Your preset, open below"
                preset.custom -> "Your preset"
                else -> "Built in"
            }
            SettingRow(Icon.Equalizer, Buds.shownName(preset), subtitle, current = chosen, onClick = { app.selectPreset(id) }) {
                if (preset.custom) {
                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        IconButton(Icon.Pencil) { app.editPreset(id) }
                        IconButton(Icon.Trash) { app.deletePreset(id) }
                    }
                }
            }
        }
    }
    Row(verticalAlignment = Alignment.CenterVertically) {
        Note("A new preset starts flat and is put to use.", Modifier.weight(1f))
        if (app.saving(Item.NewPreset)) Label("Creating…", color = p.live, modifier = Modifier.padding(end = 10.dp))
        Pill("New preset", onClick = app::newPreset)
    }
    val edited = app.editing?.let(buds::preset)?.takeIf { it.custom }
    when {
        edited != null -> Bands(app, edited)
        presets.presets.any { it.custom } -> Note("Use the pencil on one of your presets to change its bands.")
    }
}

/** A custom preset's bands, one dotted slider each. */
@Composable
private fun Bands(app: App, preset: EqPreset) {
    val look = LocalLook.current
    val p = look.p
    val id = preset.id
    val gains = app.draft?.takeIf { it.preset == id }?.bands ?: preset.bands
    val range = preset.range ?: -6..6
    Column(
        Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(28.dp))
            .background(p.s1)
            .padding(20.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            Words(preset.name, 20.sp, weight = FontWeight.SemiBold)
            Label("%+d to %+d dB".format(range.first, range.last), modifier = Modifier.weight(1f))
            if (app.saving(Item.Preset(id))) Label(SAVING, color = p.live)
        }
        Words("Moving a band saves this preset and puts it in use, so you hear it.", 13.5.sp, p.fgDim)
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(10.dp, Alignment.End)) {
            Pill("Rename") { app.renamePreset(id) }
            Pill("Done", onClick = app::doneEditing)
        }
        Row(Modifier.fillMaxWidth()) {
            for (band in gains) {
                Column(
                    Modifier.weight(1f),
                    horizontalAlignment = Alignment.CenterHorizontally,
                    verticalArrangement = Arrangement.spacedBy(10.dp),
                ) {
                    // Doto is never set under 26 sp.
                    BasicText(gain(band.gain), style = TextStyle(fontFamily = look.fonts.dot, fontSize = 26.sp, color = p.fg))
                    DotSlider(
                        range.first,
                        range.last,
                        band.gain,
                        change = { app.moveBand(id, band.hz, it) },
                        release = { app.saveBands(id) },
                        vertical = true,
                    )
                    Label(frequency(band.hz), 10.sp)
                }
            }
        }
    }
}

/** `50` → "50 Hz", `16000` → "16 kHz". */
private fun frequency(hz: Int) = if (hz >= 1000 && hz % 1000 == 0) "${hz / 1000} kHz" else "$hz Hz"
