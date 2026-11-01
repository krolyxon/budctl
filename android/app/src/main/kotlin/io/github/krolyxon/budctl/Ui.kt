package io.github.krolyxon.budctl

import androidx.compose.animation.animateColorAsState
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicText
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.TextUnit
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/** Round corners: cards, and the rows in them. */
private val CARD = RoundedCornerShape(28.dp)

/** Fully round: pills, chips, tracks. */
private val ROUND = RoundedCornerShape(percent = 50)

/** Shown instead of a row's subtitle while a write to it is on its way. */
const val SAVING = "Saving…"

/** A caps label in the monospace font, like "LEFT" or "BUILT IN". */
@Composable
fun Label(words: String, size: TextUnit = 11.sp, color: Color = LocalLook.current.p.grey, modifier: Modifier = Modifier) {
    val look = LocalLook.current
    BasicText(
        words.uppercase(),
        modifier,
        style = TextStyle(fontFamily = look.fonts.mono, fontSize = size, color = color, lineHeight = size * 1.35f),
    )
}

/** Plain text in Space Grotesk. */
@Composable
fun Words(
    words: String,
    size: TextUnit = 14.sp,
    color: Color = LocalLook.current.p.fg,
    weight: FontWeight = FontWeight.Normal,
    modifier: Modifier = Modifier,
) {
    val look = LocalLook.current
    BasicText(words, modifier, style = TextStyle(fontFamily = look.fonts.ui, fontSize = size, color = color, fontWeight = weight))
}

/** A page's caption, at its top: "overview · battery, noise control, sound". */
@Composable
fun Caption(words: String) = Label(words, 12.sp, modifier = Modifier.padding(horizontal = 18.dp))

/** A heading above a card. */
@Composable
fun Heading(words: String) = Label(words, 11.sp, modifier = Modifier.padding(horizontal = 18.dp))

/** Plain words under or beside a card. */
@Composable
fun Note(words: String, modifier: Modifier = Modifier) =
    Words(words, 13.5.sp, LocalLook.current.p.fgDim, modifier = modifier.padding(horizontal = 18.dp))

/** A round card holding rows. */
@Composable
fun Card(content: @Composable ColumnScope.() -> Unit) {
    Column(
        Modifier
            .fillMaxWidth()
            .clip(CARD)
            .background(LocalLook.current.p.s1)
            .padding(6.dp),
        verticalArrangement = Arrangement.spacedBy(2.dp),
        content = content,
    )
}

/** A surface that a touch darkens, fading, the way a hover does on the desktop. */
@Composable
private fun pressable(rest: Color, onClick: (() -> Unit)?, shape: androidx.compose.ui.graphics.Shape): Modifier {
    val look = LocalLook.current
    val touches = remember { MutableInteractionSource() }
    val pressed by touches.collectIsPressedAsState()
    val color by animateColorAsState(
        if (pressed && onClick != null) look.p.s3 else rest,
        Motion.eased(Motion.HOVER, look.motion),
        label = "press",
    )
    val clicks = if (onClick != null) Modifier.clickable(touches, indication = null, onClick = onClick) else Modifier
    return Modifier.clip(shape).background(color).then(clicks)
}

/** [content] centered in a circle [size] across. */
@Composable
fun Disc(color: Color, size: androidx.compose.ui.unit.Dp = 44.dp, content: @Composable () -> Unit) {
    val look = LocalLook.current
    val fill by animateColorAsState(color, Motion.eased(Motion.STATE, look.motion), label = "disc")
    Box(Modifier.size(size).clip(CircleShape).background(fill), contentAlignment = Alignment.Center) { content() }
}

/**
 * A setting row: the icon in a circle, a title, an optional subtitle, and something on the right.
 * With [onClick], the whole row takes the touch. [current], like the preset in use, fills the
 * circle. A subtitle ending in "…" ("Saving…") says something is under way, in the live color.
 * [below] goes under the row, full width, like a slider.
 */
@Composable
fun SettingRow(
    icon: Icon,
    title: String,
    subtitle: String? = null,
    current: Boolean = false,
    onClick: (() -> Unit)? = null,
    below: (@Composable () -> Unit)? = null,
    trailing: @Composable () -> Unit = {},
) {
    val p = LocalLook.current.p
    Column(pressable(Color.Transparent, onClick, CARD).fillMaxWidth().padding(horizontal = 14.dp, vertical = 8.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(14.dp)) {
            Disc(if (current) p.fill else p.s2) { Glyph(icon, 20.dp, if (current) p.fillFg else p.fg) }
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(3.dp)) {
                Words(title, 15.5.sp, weight = FontWeight.Medium)
                subtitle?.let { Label(it, 10.5.sp, if (it.endsWith('…')) p.live else p.grey) }
            }
            trailing()
        }
        below?.invoke()
    }
}

/** A chevron: the row opens a page. */
@Composable
fun Chevron() = Glyph(Icon.Chevron, 18.dp, LocalLook.current.p.grey)

/** One choice of a [Segmented] control. */
class Segment(val label: String, val selected: Boolean, val icon: Icon? = null, val onClick: () -> Unit)

/** A one-of-n pill track: the chosen cell is filled. [large] puts an icon above each label. */
@Composable
fun Segmented(segments: List<Segment>, large: Boolean, modifier: Modifier = Modifier) {
    val look = LocalLook.current
    val p = look.p
    Row(
        modifier.fillMaxWidth().clip(ROUND).background(p.s2).padding(6.dp),
        horizontalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        for (segment in segments) {
            val chosen by animateColorAsState(
                if (segment.selected) p.fill else Color.Transparent,
                Motion.eased(Motion.STATE, look.motion),
                label = "segment",
            )
            val ink = if (segment.selected) p.fillFg else p.grey
            Column(
                pressable(chosen, segment.onClick, ROUND)
                    .weight(1f)
                    .padding(horizontal = 8.dp, vertical = if (large) 14.dp else 10.dp),
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                if (large && segment.icon != null) Glyph(segment.icon, 22.dp, if (segment.selected) p.fillFg else p.fg)
                val size = if (large) 11.5.sp else 11.sp
                BasicText(
                    segment.label.uppercase(),
                    style = TextStyle(fontFamily = look.fonts.mono, fontSize = size, color = ink, textAlign = TextAlign.Center),
                )
            }
        }
    }
}

/** A chip: a caps label in a pill, [filled] for the main action, else on a dark pill. */
@Composable
fun Pill(words: String, filled: Boolean = false, onClick: () -> Unit) {
    val p = LocalLook.current.p
    Box(pressable(if (filled) p.fill else p.s3, onClick, ROUND).padding(horizontal = 18.dp, vertical = 10.dp)) {
        Label(words, 11.sp, if (filled) p.fillFg else p.fg)
    }
}

/** A value shown in a dark pill, like a firmware version or "NOT SET". */
@Composable
fun ValueChip(words: String) {
    val p = LocalLook.current.p
    Box(Modifier.clip(ROUND).background(p.s2).padding(horizontal = 14.dp, vertical = 8.dp)) {
        Label(words, 11.sp, p.fg)
    }
}

/** A small caps tag with a live dot, like "DEMO". */
@Composable
fun Tag(words: String) {
    val p = LocalLook.current.p
    Row(
        Modifier.clip(ROUND).background(p.s2).padding(horizontal = 12.dp, vertical = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Label(words, 10.5.sp, p.fg)
        Light(p.live, 7.dp)
    }
}

/** What went wrong, above a page, with a way to try again. */
@Composable
fun Problem(message: String, retry: () -> Unit) {
    val p = LocalLook.current.p
    Card {
        Row(
            Modifier.padding(horizontal = 14.dp, vertical = 10.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(14.dp),
        ) {
            Words(message, 13.5.sp, p.err, modifier = Modifier.weight(1f))
            Pill("Try again", filled = true, onClick = retry)
        }
    }
}
