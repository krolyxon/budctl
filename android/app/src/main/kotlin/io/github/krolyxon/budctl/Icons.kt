package io.github.krolyxon.budctl

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.unit.Dp
import kotlin.math.cos
import kotlin.math.sin

/** The desktop app's icons (crates/budctl-gui/src/icons.rs), as paths on a 24 × 24 grid. */
enum class Icon {
    Equalizer, Gestures, Earbud, Settings, NoiseOff, Anc, Transparency, Game, Spatial, Bass, Wind,
    Dual, Chevron, Bolt, Find, Info, Chip, Pencil, Trash, Laptop, Phone,
}

/** [icon], [size] across, in [color]. */
@Composable
fun Glyph(icon: Icon, size: Dp, color: Color, modifier: Modifier = Modifier) {
    Canvas(modifier.size(size)) { Pen(this, color).draw(icon) }
}

/** Draws on the 24 × 24 grid, scaled to the canvas, with round caps and joins. */
private class Pen(private val scope: DrawScope, private val color: Color) {
    private val scale = scope.size.width / 24f
    private val stroke = Stroke(width = 1.9f * scale, cap = StrokeCap.Round, join = StrokeJoin.Round)

    private fun at(x: Float, y: Float) = Offset(x * scale, y * scale)

    fun polyline(points: List<Pair<Float, Float>>, closed: Boolean = false) {
        val path = Path()
        points.forEachIndexed { n, (x, y) ->
            if (n == 0) path.moveTo(x * scale, y * scale) else path.lineTo(x * scale, y * scale)
        }
        if (closed) path.close()
        scope.drawPath(path, color, style = stroke)
    }

    fun line(from: Pair<Float, Float>, to: Pair<Float, Float>) = polyline(listOf(from, to))

    fun circle(x: Float, y: Float, r: Float) = scope.drawCircle(color, r * scale, at(x, y), style = stroke)

    fun dot(x: Float, y: Float, r: Float) = scope.drawCircle(color, r * scale, at(x, y))

    /** Degrees, clockwise from three o'clock; [to] below [from] goes the other way. */
    fun arc(x: Float, y: Float, r: Float, from: Float, to: Float) = scope.drawArc(
        color,
        startAngle = from,
        sweepAngle = to - from,
        useCenter = false,
        topLeft = at(x - r, y - r),
        size = Size(2 * r * scale, 2 * r * scale),
        style = stroke,
    )

    fun rect(x: Float, y: Float, w: Float, h: Float, r: Float) =
        scope.drawRoundRect(color, at(x, y), Size(w * scale, h * scale), CornerRadius(r * scale), style = stroke)

    fun fillRect(x: Float, y: Float, w: Float, h: Float, r: Float) =
        scope.drawRoundRect(color, at(x, y), Size(w * scale, h * scale), CornerRadius(r * scale))

    fun draw(icon: Icon) {
        when (icon) {
            Icon.Equalizer -> for ((x, knob) in listOf(6f to 14f, 12f to 8f, 18f to 16f)) {
                line(x to 3.5f, x to 20.5f)
                line(x - 2.5f to knob, x + 2.5f to knob)
            }
            Icon.Gestures -> {
                rect(9.5f, 11f, 5f, 11f, 2.5f)
                arc(12f, 11.5f, 4f, 200f, 340f)
                arc(12f, 11.5f, 7.5f, 210f, 330f)
            }
            Icon.Earbud -> {
                fillRect(14.5f, 5f, 6f, 7f, 2.8f)
                dot(11f, 8.5f, 6.3f)
                fillRect(6.6f, 10f, 4.4f, 12.5f, 2.2f)
            }
            Icon.Settings -> for ((y, knob) in listOf(6f to 15f, 12f to 8f, 18f to 13f)) {
                line(3.5f to y, 20.5f to y)
                dot(knob, y, 2.6f)
            }
            Icon.NoiseOff -> {
                circle(12f, 12f, 8.5f)
                line(6f to 6f, 18f to 18f)
            }
            Icon.Anc -> {
                circle(12f, 12f, 8.5f)
                circle(12f, 12f, 4.5f)
                dot(12f, 12f, 1.4f)
            }
            Icon.Transparency -> {
                for (n in 0 until 12) {
                    val angle = Math.toRadians(n * 30.0)
                    dot(12f + 8.5f * cos(angle).toFloat(), 12f + 8.5f * sin(angle).toFloat(), 1.15f)
                }
                dot(12f, 12f, 2.2f)
            }
            Icon.Game, Icon.Bolt -> polyline(
                listOf(13.5f to 2.5f, 5f to 13.5f, 11.5f to 13.5f, 10.5f to 21.5f, 19f to 10.5f, 12.5f to 10.5f),
                closed = true,
            )
            Icon.Spatial -> {
                dot(12f, 12f, 2.4f)
                for ((r, spread) in listOf(5.5f to 40f, 9.5f to 35f)) {
                    arc(12f, 12f, r, -spread, spread)
                    arc(12f, 12f, r, 180f - spread, 180f + spread)
                }
            }
            Icon.Bass -> {
                polyline(
                    listOf(3.5f to 9.5f, 7.5f to 9.5f, 12.5f to 5f, 12.5f to 19f, 7.5f to 14.5f, 3.5f to 14.5f),
                    closed = true,
                )
                arc(13f, 12f, 4f, -50f, 50f)
                arc(13f, 12f, 8f, -50f, 50f)
            }
            Icon.Wind -> {
                line(3f to 8.5f, 13f to 8.5f)
                arc(13f, 6f, 2.5f, 90f, -180f)
                line(3f to 12.5f, 17.5f to 12.5f)
                arc(17.5f, 15.5f, 3f, -90f, 180f)
                line(3f to 16.5f, 9f to 16.5f)
            }
            Icon.Dual -> {
                rect(2.5f, 5f, 12.5f, 9f, 1.5f)
                line(1.5f to 17.5f, 16f to 17.5f)
                rect(17.5f, 8f, 4.5f, 11f, 1.2f)
            }
            Icon.Chevron -> polyline(listOf(9f to 5.5f, 15.5f to 12f, 9f to 18.5f))
            Icon.Find -> {
                circle(12f, 12f, 6.5f)
                dot(12f, 12f, 2f)
                line(12f to 2f, 12f to 5f)
                line(12f to 19f, 12f to 22f)
                line(2f to 12f, 5f to 12f)
                line(19f to 12f, 22f to 12f)
            }
            Icon.Pencil -> {
                polyline(listOf(4.5f to 19.5f, 5.5f to 15f, 15.5f to 5f, 19f to 8.5f, 9f to 18.5f), closed = true)
                line(13f to 7.5f, 16.5f to 11f)
            }
            Icon.Trash -> {
                line(4f to 6.5f, 20f to 6.5f)
                polyline(listOf(9f to 6.5f, 9.5f to 3.5f, 14.5f to 3.5f, 15f to 6.5f))
                polyline(listOf(6f to 6.5f, 7f to 20.5f, 17f to 20.5f, 18f to 6.5f))
                line(10f to 10.5f, 10f to 16.5f)
                line(14f to 10.5f, 14f to 16.5f)
            }
            Icon.Info -> {
                circle(12f, 12f, 9f)
                line(12f to 11f, 12f to 16.5f)
                dot(12f, 7.8f, 1.2f)
            }
            Icon.Chip -> {
                rect(6.5f, 6.5f, 11f, 11f, 2f)
                for (at in listOf(9.5f, 14.5f)) {
                    line(at to 2.5f, at to 6.5f)
                    line(at to 17.5f, at to 21.5f)
                    line(2.5f to at, 6.5f to at)
                    line(17.5f to at, 21.5f to at)
                }
            }
            Icon.Laptop -> {
                rect(4f, 5f, 16f, 11f, 1.5f)
                line(2f to 19f, 22f to 19f)
            }
            Icon.Phone -> {
                rect(7f, 2.5f, 10f, 19f, 2f)
                line(10.5f to 18.5f, 13.5f to 18.5f)
            }
        }
    }
}
