package io.github.krolyxon.budctl

import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.keyframes
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.gestures.detectHorizontalDragGestures
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.text.BasicText
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.util.lerp
import kotlin.math.PI
import kotlin.math.cos
import kotlin.math.max
import kotlin.math.roundToInt
import kotlin.math.sin

/** The dots in a battery ring. */
private const val RING_DOTS = 44

/** How many of [n] dots the fraction [f] lights. */
private fun litCount(f: Float, n: Int) = (f.coerceIn(0f, 1f) * n).roundToInt().coerceAtMost(n)

/**
 * A battery ring as wide as it is given: a ring of dots, lit clockwise from the top up to the
 * [level] (a percentage), with the level in LED dots in the middle; "--" and no lit dot when it
 * is unknown. The dots light one after the other, like the Glyph lights, when the ring first
 * shows and when the level changes.
 */
@Composable
fun Ring(level: Int?, modifier: Modifier = Modifier) {
    val look = LocalLook.current
    val p = look.p
    val target = level?.let { litCount(it / 100f, RING_DOTS) } ?: 0
    val lit = remember { Animatable(0f) }
    LaunchedEffect(target, look.motion) {
        if (look.motion) lit.animateTo(target.toFloat(), tween(Motion.SWEEP, easing = Motion.EASE))
        else lit.snapTo(target.toFloat())
    }
    BoxWithConstraints(modifier.aspectRatio(1f), contentAlignment = Alignment.Center) {
        Canvas(Modifier.size(maxWidth)) {
            val dot = max(size.width * 0.045f, 3.dp.toPx())
            val radius = size.width / 2 - dot / 2 - 1
            val on = lit.value.roundToInt()
            for (n in 0 until RING_DOTS) {
                val angle = n.toFloat() / RING_DOTS * 2 * PI.toFloat() - PI.toFloat() / 2
                val at = Offset(center.x + radius * cos(angle), center.y + radius * sin(angle))
                drawCircle(if (n < on) p.fill else p.s3, dot / 2, at)
            }
        }
        // Doto is never set under 26 sp.
        val figure = max(26f, maxWidth.value * 0.28f).sp
        BasicText(
            level?.toString() ?: "--",
            style = TextStyle(fontFamily = look.fonts.dot, fontSize = figure, color = if (level != null) p.fg else p.mute),
        )
    }
}

/**
 * An on/off switch: off, a dark pill with a dim knob on the left; on, the fill runs to the right
 * end, with a dot in it. Turned, the knob stretches into the fill (or back). It only shows the
 * state: the row around it takes the touch.
 */
@Composable
fun Toggle(on: Boolean) {
    val look = LocalLook.current
    val p = look.p
    val t by animateFloatAsState(if (on) 1f else 0f, Motion.eased(Motion.STATE, look.motion), label = "switch")
    Canvas(Modifier.size(60.dp, 32.dp)) {
        drawRoundRect(p.s2, cornerRadius = CornerRadius(size.height / 2))
        val pad = 4.dp.toPx()
        val height = size.height - 2 * pad
        val width = lerp(height, size.width - 2 * pad, t)
        drawRoundRect(lerp(p.mute, p.fill, t), Offset(pad, pad), Size(width, height), CornerRadius(height / 2))
        if (t > 0f) {
            val r = 4.5.dp.toPx()
            drawCircle(p.fillFg.copy(alpha = p.fillFg.alpha * t), r, Offset(pad + width - 7.dp.toPx() - r, size.height / 2))
        }
    }
}

/** A dotted slider's dots: the value's, and the others. */
private val DOT = 5.dp
private val DOT_BIG = 11.dp

/**
 * A dotted slider over [low]..[high], one dot per step, left to right: the dots from 0 to the
 * value are lit, and the value's own dot is larger. A touch or a drag calls [change]; lifting the
 * finger calls [release], which is when a front end writes.
 */
@Composable
fun DotSlider(low: Int, high: Int, value: Int, change: (Int) -> Unit, release: () -> Unit, modifier: Modifier = Modifier) {
    val look = LocalLook.current
    val p = look.p
    val onChange by rememberUpdatedState(change)
    val onRelease by rememberUpdatedState(release)
    // The value's dot grows; the one it left shrinks back.
    var left by remember { mutableStateOf<Int?>(null) }
    var shown by remember { mutableStateOf(value) }
    val grow = remember { Animatable(1f) }
    LaunchedEffect(value) {
        if (value != shown) {
            left = shown
            shown = value
            grow.snapTo(0f)
            grow.animateTo(1f, Motion.eased(Motion.HOVER, look.motion))
        }
    }
    val steps = max(1, high - low)
    Canvas(
        modifier
            .fillMaxWidth()
            .height(DOT_BIG + 22.dp)
            .pointerInput(low, high) {
                fun at(x: Float): Int {
                    val pad = DOT_BIG.toPx()
                    val along = ((x - pad) / max(1f, size.width - 2 * pad)).coerceIn(0f, 1f)
                    return (low + (along * steps).roundToInt()).coerceIn(low, high)
                }
                detectTapGestures { offset ->
                    onChange(at(offset.x))
                    onRelease()
                }
            }
            .pointerInput(low, high) {
                fun at(x: Float): Int {
                    val pad = DOT_BIG.toPx()
                    val along = ((x - pad) / max(1f, size.width - 2 * pad)).coerceIn(0f, 1f)
                    return (low + (along * steps).roundToInt()).coerceIn(low, high)
                }
                detectHorizontalDragGestures(onDragEnd = { onRelease() }, onDragCancel = { onRelease() }) { change, _ ->
                    change.consume()
                    onChange(at(change.position.x))
                }
            },
    ) {
        val pad = DOT_BIG.toPx()
        val base = low.coerceAtLeast(0).coerceAtMost(high)
        val from = minOf(base, value)
        val to = maxOf(base, value)
        val g = grow.value
        for (v in low..high) {
            val x = pad + (v - low).toFloat() / steps * (size.width - 2 * pad)
            val lit = v in from..to
            val diameter = when {
                v == value -> lerp(DOT.toPx(), DOT_BIG.toPx(), g)
                v == left -> lerp(DOT_BIG.toPx(), DOT.toPx(), g)
                else -> DOT.toPx()
            }
            drawCircle(if (lit || v == value) p.fill else p.s3, diameter / 2, Offset(x, size.height / 2))
        }
    }
}

/** A round light [size] across; [blinking], it is lit and dark in turn, like a recording light. */
@Composable
fun Light(color: Color, size: Dp, blinking: Boolean = false) {
    val look = LocalLook.current
    val alpha = if (blinking && look.motion) {
        val blink = rememberInfiniteTransition(label = "blink")
        // Hard steps, no fade: lit for one beat, dark for the next.
        val a by blink.animateFloat(
            1f,
            0f,
            infiniteRepeatable(
                keyframes {
                    durationMillis = 2 * Motion.BLINK
                    1f at 0
                    1f at Motion.BLINK - 1
                    0f at Motion.BLINK
                    0f at 2 * Motion.BLINK - 1
                },
                RepeatMode.Restart,
            ),
            label = "blink",
        )
        a
    } else {
        1f
    }
    Box(Modifier.size(size)) {
        Canvas(Modifier.size(size)) { drawCircle(color.copy(alpha = color.alpha * alpha)) }
    }
}
