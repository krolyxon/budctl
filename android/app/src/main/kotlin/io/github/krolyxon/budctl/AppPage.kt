package io.github.krolyxon.budctl

import android.content.ClipData
import android.content.ClipboardManager
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.text.BasicText
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/** The app's own settings: colors, motion, updates, the debug log, and what budctl is. */
@Composable
fun AppPage(app: App) {
    val look = LocalLook.current
    val context = LocalContext.current
    Heading("look")
    Card {
        SettingRow(
            Icon.Settings,
            "Material You colors",
            if (app.materialYou) "From your wallpaper" else "Nothing's near-black",
            onClick = app::toggleMaterialYou,
        ) { Toggle(app.materialYou) }
        SettingRow(
            Icon.Spatial,
            "Motion",
            "Fades, switches, the battery rings' sweep",
            onClick = app::toggleMotion,
        ) { Toggle(app.motion) }
    }
    Updates(app)
    if (!app.demo) {
        Heading("debug")
        Card {
            SettingRow(
                Icon.Chip,
                "Debug log",
                "Every frame to and from the buds, in hex",
                onClick = app::toggleDebug,
            ) { Toggle(app.debug) }
        }
        Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
            Note("The log of this session, to paste in a bug report.", Modifier.weight(1f))
            Pill("Copy log") {
                val clipboard = context.getSystemService(ClipboardManager::class.java)
                clipboard.setPrimaryClip(ClipData.newPlainText("budctl log", app.log.joinToString("\n")))
            }
        }
        if (app.log.isNotEmpty()) {
            Card {
                BasicText(
                    app.log.takeLast(SHOWN_LINES).joinToString("\n"),
                    Modifier.padding(14.dp),
                    style = TextStyle(fontFamily = look.fonts.mono, fontSize = 10.sp, color = look.p.fgDim),
                )
            }
        }
    }
    Heading("about")
    Card {
        SettingRow(
            Icon.Phone,
            "Network",
            if (app.updateCheck) "GitHub, once a day, for updates" else "Only when you tap Check now",
        )
        SettingRow(Icon.Chip, "License", "GPL-3.0-or-later")
    }
}

/**
 * The update check (docs/android.md#updates): the daily check's switch, off until switched on, and
 * the installed version with what is known about a newer one.
 */
@Composable
private fun Updates(app: App) {
    val context = LocalContext.current
    val version = remember { context.packageManager.getPackageInfo(context.packageName, 0).versionName ?: "?" }
    Heading("updates")
    Card {
        SettingRow(
            Icon.Update,
            "Check for updates",
            if (app.updateCheck) "Once a day, on budctl's GitHub releases" else "Off: budctl stays offline",
            onClick = app::toggleUpdateCheck,
        ) { Toggle(app.updateCheck) }
        val now = app.update
        SettingRow(Icon.Info, "Version $version", updateStatus(now, app.updateCheck)) {
            when {
                now is UpdateState.Available || (now is UpdateState.Failed && now.release != null) ->
                    Pill("Update", filled = true, onClick = app::startUpdate)
                now is UpdateState.Checking || now is UpdateState.Downloading || now is UpdateState.Installing -> {}
                else -> Pill("Check now", onClick = app::checkForUpdate)
            }
        }
    }
}

/** The update's state in words; one ending in "…" is under way, and shows in the live color. */
fun updateStatus(state: UpdateState, on: Boolean): String? = when (state) {
    UpdateState.Idle -> if (on) "Not checked yet" else null
    UpdateState.Checking -> "Checking…"
    UpdateState.UpToDate -> "Up to date"
    is UpdateState.Available -> "${state.release.version} is out"
    is UpdateState.Downloading -> "Downloading ${state.release.version}: ${state.percent}%…"
    is UpdateState.Installing -> "Android is installing ${state.release.version}…"
    is UpdateState.Failed -> state.message
}

/** The log lines the page shows; Copy log copies them all. */
private const val SHOWN_LINES = 60
