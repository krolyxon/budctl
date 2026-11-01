package io.github.krolyxon.budctl

import androidx.compose.animation.animateColorAsState
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog

/** The buds, as the page shows them. */
private val SIDES = listOf("left" to "Left bud", "right" to "Right bud")

/** A gesture whose action is being picked. */
private data class Picking(val side: String, val gesture: String, val actions: List<String>)

/** The Gestures page (crates/budctl-gui/src/gestures.rs): what each tap and hold does, and what a hold steps through. */
@Composable
fun Gestures(app: App) {
    val touch = app.buds.touch
    if (touch == null) {
        Note("These buds did not report their touch controls.")
        return
    }
    var picking by remember { mutableStateOf<Picking?>(null) }
    for ((side, heading) in SIDES) {
        Heading(heading)
        Card {
            for (choice in touch.choices) {
                val draft = app.touchDraft?.takeIf { it.side == side && it.gesture == choice.gesture }
                val shown = draft?.action ?: app.buds.action(side, choice.gesture) ?: "none"
                SettingRow(
                    if (choice.gesture == "hold") Icon.Earbud else Icon.Gestures,
                    gestureName(choice.gesture),
                    if (app.saving(Item.Gesture(side, choice.gesture))) SAVING else null,
                    onClick = { picking = Picking(side, choice.gesture, choice.actions) },
                ) { ActionPill(actionName(shown)) }
            }
        }
    }
    val cycle = touch.holdCycle
    if (cycle != null && touch.cycleModes.isNotEmpty()) {
        Heading("A hold steps through")
        Card {
            for (mode in touch.cycleModes) {
                val on = mode in cycle
                val about = when {
                    app.saving(Item.Cycle(mode)) -> SAVING
                    on && cycle.size <= 2 -> "A hold needs two modes to step through"
                    else -> null
                }
                SettingRow(modeIcon(mode), modeName(mode), about, onClick = { app.cycle(mode, !on) }) { Toggle(on) }
            }
        }
    }
    picking?.let { asked ->
        val now = app.touchDraft?.takeIf { it.side == asked.side && it.gesture == asked.gesture }?.action
            ?: app.buds.action(asked.side, asked.gesture)
        ActionList(asked, now, close = { picking = null }) { action -> app.pickAction(asked.side, asked.gesture, action) }
    }
}

/** The action a gesture has, in a pill with a chevron: the row opens the list. */
@Composable
private fun ActionPill(words: String) {
    val p = LocalLook.current.p
    Row(
        Modifier
            .widthIn(max = 170.dp)
            .clip(RoundedCornerShape(percent = 50))
            .background(p.s2)
            .padding(start = 16.dp, end = 10.dp, top = 9.dp, bottom = 9.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        Words(words, 14.sp, weight = FontWeight.Medium, modifier = Modifier.weight(1f, fill = false))
        Glyph(Icon.Chevron, 14.dp, p.grey, Modifier.rotate(90f))
    }
}

/** The actions a gesture can be given, over the dimmed page; the one it has is filled. */
@Composable
private fun ActionList(asked: Picking, now: String?, close: () -> Unit, pick: (String) -> Unit) {
    val look = LocalLook.current
    val p = look.p
    Dialog(onDismissRequest = close) {
        CompositionLocalProvider(LocalLook provides look) {
            Column(
                Modifier
                    .clip(RoundedCornerShape(28.dp))
                    .background(p.s1)
                    .padding(vertical = 20.dp, horizontal = 12.dp),
                verticalArrangement = Arrangement.spacedBy(4.dp),
            ) {
                Label(
                    "${SIDES.first { it.first == asked.side }.second} · ${gestureName(asked.gesture)}",
                    modifier = Modifier.padding(start = 12.dp, bottom = 10.dp),
                )
                for (action in asked.actions) {
                    val chosen = action == now
                    val fill by animateColorAsState(if (chosen) p.fill else Color.Transparent, Motion.eased(Motion.STATE, look.motion), label = "action")
                    Box(
                        Modifier
                            .fillMaxWidth()
                            .clip(RoundedCornerShape(20.dp))
                            .background(fill)
                            .clickable(remember { MutableInteractionSource() }, indication = null) {
                                pick(action)
                                close()
                            }
                            .padding(horizontal = 16.dp, vertical = 13.dp),
                    ) {
                        Words(actionName(action), 15.sp, if (chosen) p.fillFg else p.fg, FontWeight.Medium)
                    }
                }
            }
        }
    }
}

private fun gestureName(gesture: String) = if (gesture == "hold") "Touch and hold" else Buds.words(gesture)

/** An action, as people read it. */
fun actionName(action: String) = when (action) {
    "none" -> "Nothing"
    "play-pause" -> "Play / pause"
    "next" -> "Next track"
    "previous" -> "Previous track"
    "voice-assistant" -> "Voice assistant"
    "noise-cycle" -> "Noise control"
    else -> Buds.words(action)
}

private fun modeName(mode: String) = if (mode == "anc") "Noise cancelling" else Buds.words(mode)

private fun modeIcon(mode: String) = when (mode) {
    "off" -> Icon.NoiseOff
    "anc" -> Icon.Anc
    else -> Icon.Transparency
}
