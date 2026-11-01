package io.github.krolyxon.budctl

import android.content.Context
import android.content.res.AssetManager
import android.os.Build
import androidx.compose.animation.core.AnimationSpec
import androidx.compose.animation.core.CubicBezierEasing
import androidx.compose.animation.core.snap
import androidx.compose.animation.core.tween
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight

/**
 * The desktop app's look (CLAUDE.md, GUI): Nothing OS's shapes and type in Material You colors.
 * The colors come by role, as in crates/budctl-gui/src/theme.rs: a ladder of surfaces, and the
 * "on" states on top.
 */
@Immutable
data class Palette(
    /** The page (`surfaceLowest`). */
    val bg: Color,
    /** Cards (`surfaceLow`). */
    val s1: Color,
    /** Icon circles, tracks, the switches' "off" fill (`surfaceHigh`). */
    val s2: Color,
    /** Unlit dots, pressed rows (`surfaceHighest`). */
    val s3: Color,
    /** Inactive things: the pages not open, an off switch's knob (`outlineVariant`). */
    val mute: Color,
    /** Caps labels (`outline`). */
    val grey: Color,
    /** Text (`onSurface`). */
    val fg: Color,
    /** Secondary text (`onSurfaceVariant`). */
    val fgDim: Color,
    /** What is on or chosen: switches, the chosen segment, lit dots (`primary`). */
    val fill: Color,
    /** Text and icons on [fill] (`onPrimary`). */
    val fillFg: Color,
    /** Live things, one at a time: the open page's dot, "Saving…", charging (`tertiary`). */
    val live: Color,
    val liveFg: Color,
    val err: Color,
    val errFg: Color,
)

/** Near-black surfaces, white "on" states and a red for live things: the desktop's own. */
val BUILT_IN = Palette(
    bg = Color(0xFF000000),
    s1 = Color(0xFF111112),
    s2 = Color(0xFF1C1C1E),
    s3 = Color(0xFF2A2A2D),
    mute = Color(0xFF454549),
    grey = Color(0xFF8E8E93),
    fg = Color(0xFFF2F2F2),
    fgDim = Color(0xFFB8B8BC),
    fill = Color(0xFFF2F2F2),
    fillFg = Color(0xFF000000),
    live = Color(0xFFD71921),
    liveFg = Color(0xFFFFFFFF),
    err = Color(0xFFFFB4AB),
    errFg = Color(0xFF690005),
)

/**
 * The phone's Material You colors (from the wallpaper), dark, in the same roles as the desktop
 * maps Aurora's palette. Android 14 names every role; before it, the tonal palettes stand in.
 */
fun materialYou(context: Context): Palette {
    fun c(id: Int) = Color(context.getColor(id))
    return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
        Palette(
            bg = c(android.R.color.system_surface_container_lowest_dark),
            s1 = c(android.R.color.system_surface_container_low_dark),
            s2 = c(android.R.color.system_surface_container_high_dark),
            s3 = c(android.R.color.system_surface_container_highest_dark),
            mute = c(android.R.color.system_outline_variant_dark),
            grey = c(android.R.color.system_outline_dark),
            fg = c(android.R.color.system_on_surface_dark),
            fgDim = c(android.R.color.system_on_surface_variant_dark),
            fill = c(android.R.color.system_primary_dark),
            fillFg = c(android.R.color.system_on_primary_dark),
            live = c(android.R.color.system_tertiary_dark),
            liveFg = c(android.R.color.system_on_tertiary_dark),
            err = c(android.R.color.system_error_dark),
            errFg = c(android.R.color.system_on_error_dark),
        )
    } else {
        Palette(
            bg = c(android.R.color.system_neutral1_1000),
            s1 = c(android.R.color.system_neutral1_900),
            s2 = c(android.R.color.system_neutral1_800),
            s3 = c(android.R.color.system_neutral1_700),
            mute = c(android.R.color.system_neutral2_700),
            grey = c(android.R.color.system_neutral2_400),
            fg = c(android.R.color.system_neutral1_100),
            fgDim = c(android.R.color.system_neutral2_200),
            fill = c(android.R.color.system_accent1_200),
            fillFg = c(android.R.color.system_accent1_800),
            live = c(android.R.color.system_accent3_200),
            liveFg = c(android.R.color.system_accent3_800),
            err = BUILT_IN.err,
            errFg = BUILT_IN.errFg,
        )
    }
}

/**
 * The desktop app's fonts, read from crates/budctl-gui/fonts (OFL 1.1), which Gradle adds to the
 * app's assets: Space Grotesk for text, Space Mono for caps labels, and Doto Rounded for numbers
 * and page words. Doto is never set under 26 sp: smaller, the dots blur.
 */
class Fonts(assets: AssetManager) {
    val ui = FontFamily(
        Font("SpaceGrotesk-Regular.otf", assets, FontWeight.Normal),
        Font("SpaceGrotesk-Medium.otf", assets, FontWeight.Medium),
        Font("SpaceGrotesk-SemiBold.otf", assets, FontWeight.SemiBold),
    )
    val mono = FontFamily(Font("SpaceMono-Regular.ttf", assets))
    val dot = FontFamily(Font("DotoRounded-Black.ttf", assets, FontWeight.Black))
}

/** What the screen draws with: the colors, the fonts, and whether things move. */
@Immutable
data class Look(val p: Palette, val fonts: Fonts, val motion: Boolean)

val LocalLook = staticCompositionLocalOf<Look> { error("no Look provided") }

/**
 * Nothing's motion (crates/budctl-gui/src/motion.rs): 150-250 ms, eased out on
 * `cubic-bezier(0.25, 0.1, 0.25, 1)`, no bounce; things fade rather than slide.
 */
object Motion {
    val EASE = CubicBezierEasing(0.25f, 0.1f, 0.25f, 1f)

    /** A press and a small change of state. */
    const val HOVER = 150

    /** A switch turning, a choice changing. */
    const val STATE = 200

    /** A page fading in. */
    const val PAGE = 240

    /** The battery rings lighting up, dot after dot. */
    const val SWEEP = 600

    /** A blinking light: lit, then dark, this long each. */
    const val BLINK = 500

    fun <T> eased(millis: Int, motion: Boolean): AnimationSpec<T> =
        if (motion) tween(millis, easing = EASE) else snap()
}
