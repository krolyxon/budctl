package io.github.krolyxon.budctl

import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.pager.HorizontalPager
import androidx.compose.foundation.pager.rememberPagerState
import androidx.compose.foundation.relocation.BringIntoViewRequester
import androidx.compose.foundation.relocation.bringIntoViewRequester
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicText
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import kotlin.math.abs
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

/**
 * The whole screen: the buds and how the app stands with them, the pages as dot-matrix words, and
 * the open page under them. Swiping changes the page; the pages fade into each other rather than
 * slide, as Nothing's motion asks.
 */
@Composable
fun Screen(app: App, askPermission: () -> Unit) {
    val context = LocalContext.current
    val fonts = remember { Fonts(context.assets) }
    val palette = remember(app.materialYou) { if (app.materialYou) materialYou(context) else BUILT_IN }
    val look = Look(palette, fonts, app.motion)
    CompositionLocalProvider(LocalLook provides look) {
        val pages = Page.entries
        val pager = rememberPagerState { pages.size }
        val scope = rememberCoroutineScope()
        Column(
            Modifier
                .fillMaxSize()
                .background(palette.bg)
                .safeDrawingPadding(),
        ) {
            Header(app)
            val open = { page: Int ->
                scope.launch {
                    if (look.motion) pager.animateScrollToPage(page, animationSpec = tween(Motion.PAGE, easing = Motion.EASE))
                    else pager.scrollToPage(page)
                }
                Unit
            }
            PageWords(pages, pager.currentPage, open)
            HorizontalPager(pager, Modifier.weight(1f)) { index ->
                Box(
                    Modifier
                        .fillMaxSize()
                        .graphicsLayer {
                            // The page stays where it is and fades, however far the swipe took it.
                            val offset = (pager.currentPage - index) + pager.currentPageOffsetFraction
                            translationX = offset * size.width
                            alpha = 1f - abs(offset).coerceIn(0f, 1f)
                        },
                ) {
                    Column(
                        Modifier
                            .fillMaxSize()
                            .verticalScroll(rememberScrollState())
                            .padding(horizontal = 14.dp, vertical = 10.dp),
                        verticalArrangement = Arrangement.spacedBy(18.dp),
                    ) {
                        Caption(pages[index].caption)
                        app.problem?.let { Problem(it) { app.read() } }
                        when (pages[index]) {
                            Page.Overview -> Overview(app, askPermission) { open(pages.indexOf(it)) }
                            Page.Equalizer -> Equalizer(app)
                            Page.Gestures -> Gestures(app)
                            Page.Earbuds -> Earbuds(app)
                            Page.App -> AppPage(app)
                        }
                        Spacer(Modifier.size(12.dp))
                    }
                }
            }
        }
        if (app.confirmDual) {
            Confirm(
                "Turn dual connection off?",
                "If two devices are connected to your buds, they may drop one of them.",
                "Turn off",
                app::answerDual,
            )
        }
        app.confirmDelete?.let { id ->
            Confirm(
                "Delete ${app.buds.preset(id)?.name ?: "this preset"}?",
                "Its bands are lost. The buds keep their other presets.",
                "Delete",
                app::answerDelete,
            )
        }
        app.prompt?.let { NameDialog(app, it) }
        if (app.confirmFind) {
            // The sound is loud enough to hurt in the ears.
            Confirm(
                "Ring your buds?",
                "Take them out of your ears first: the sound is loud. They ring until you press Stop.",
                "Ring",
                app::answerFind,
            )
        }
    }
}

/** The buds' name, how the app stands with them, and a way to read them again. */
@Composable
private fun Header(app: App) {
    val p = LocalLook.current.p
    val connection = app.connection
    val (light, state) = when {
        connection == Connection.Demo || connection == Connection.Live -> p.fill to "connected"
        connection == Connection.Connecting -> p.live to "connecting…"
        connection is Connection.Failed && connection.kind == "busy" -> p.err to "busy"
        !app.buds.connected -> p.mute to "not connected"
        else -> p.err to "error"
    }
    Column(Modifier.padding(start = 26.dp, end = 18.dp, top = 14.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Words(app.buds.name, 19.sp, weight = FontWeight.SemiBold, modifier = Modifier.weight(1f))
            if (app.demo) Tag("demo")
        }
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Light(light, 8.dp, blinking = connection == Connection.Connecting)
            Label(state, modifier = Modifier.weight(1f))
            if (app.permitted) Pill("Refresh") { app.read() }
        }
    }
}

/** The pages, as on the desktop's sidebar: the open one large with the live dot, the rest dim. */
@Composable
private fun PageWords(pages: List<Page>, open: Int, onOpen: (Int) -> Unit) {
    val look = LocalLook.current
    Row(
        Modifier
            .fillMaxWidth()
            .horizontalScroll(rememberScrollState())
            .padding(horizontal = 12.dp, vertical = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        pages.forEachIndexed { index, page ->
            val isOpen = index == open
            val size by animateFloatAsState(if (isOpen) 38f else 26f, Motion.eased(Motion.PAGE, look.motion), label = "word")
            val color by animateColorAsState(
                if (isOpen) look.p.fg else look.p.mute,
                Motion.eased(Motion.PAGE, look.motion),
                label = "word",
            )
            // The open word scrolls into view once it has grown.
            val requester = remember { BringIntoViewRequester() }
            LaunchedEffect(isOpen) {
                if (isOpen) {
                    if (look.motion) delay(Motion.PAGE.toLong())
                    requester.bringIntoView()
                }
            }
            Row(
                Modifier
                    .bringIntoViewRequester(requester)
                    .clip(RoundedCornerShape(20.dp))
                    .clickable(remember { MutableInteractionSource() }, indication = null) { onOpen(index) }
                    .padding(horizontal = 8.dp, vertical = 4.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(10.dp),
            ) {
                if (isOpen) Light(look.p.live, 11.dp)
                // Doto is never set under 26 sp.
                BasicText(page.word.uppercase(), style = TextStyle(fontFamily = look.fonts.dot, fontSize = size.sp, color = color))
            }
        }
    }
}

/** Asks for a preset's name: a new one's, or a new name for one. */
@Composable
private fun NameDialog(app: App, asked: NamePrompt) {
    val look = LocalLook.current
    val p = look.p
    val heading = asked.renaming?.let { "Rename ${app.buds.preset(it)?.name ?: "the preset"}" } ?: "New preset"
    val focus = remember { FocusRequester() }
    Dialog(onDismissRequest = app::cancelName) {
        CompositionLocalProvider(LocalLook provides look) {
            Column(
                Modifier
                    .clip(RoundedCornerShape(28.dp))
                    .background(p.s1)
                    .padding(24.dp),
                verticalArrangement = Arrangement.spacedBy(18.dp),
            ) {
                Words(heading, 22.sp, weight = FontWeight.SemiBold)
                BasicTextField(
                    asked.text,
                    app::typeName,
                    Modifier
                        .fillMaxWidth()
                        .focusRequester(focus)
                        .clip(RoundedCornerShape(percent = 50))
                        .background(p.s2)
                        .border(1.dp, p.fill, RoundedCornerShape(percent = 50))
                        .padding(horizontal = 20.dp, vertical = 12.dp),
                    textStyle = TextStyle(fontFamily = look.fonts.ui, fontSize = 16.sp, color = p.fg),
                    keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Sentences, imeAction = ImeAction.Done),
                    keyboardActions = KeyboardActions(onDone = { app.submitName() }),
                    singleLine = true,
                    cursorBrush = SolidColor(p.fill),
                )
                asked.problem?.let { Words(it, 13.sp, p.err) }
                Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(10.dp, Alignment.End)) {
                    Pill("Cancel", onClick = app::cancelName)
                    Pill(if (asked.renaming == null) "Create" else "Rename", filled = true, onClick = app::submitName)
                }
            }
        }
    }
    LaunchedEffect(Unit) { focus.requestFocus() }
}

/** A question over the dimmed page: Cancel, or [action]. */
@Composable
private fun Confirm(heading: String, words: String, action: String, answer: (Boolean) -> Unit) {
    val look = LocalLook.current
    Dialog(onDismissRequest = { answer(false) }) {
        CompositionLocalProvider(LocalLook provides look) {
            Column(
                Modifier
                    .clip(RoundedCornerShape(28.dp))
                    .background(look.p.s1)
                    .padding(24.dp),
                verticalArrangement = Arrangement.spacedBy(10.dp),
            ) {
                Words(heading, 22.sp, weight = FontWeight.SemiBold)
                Words(words, 13.5.sp, look.p.fgDim)
                Row(
                    Modifier
                        .fillMaxWidth()
                        .padding(top = 16.dp),
                    horizontalArrangement = Arrangement.spacedBy(10.dp, Alignment.End),
                ) {
                    Pill("Cancel") { answer(false) }
                    Pill(action, filled = true) { answer(true) }
                }
            }
        }
    }
}
