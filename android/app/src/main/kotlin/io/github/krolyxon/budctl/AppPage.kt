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

/** The app's own settings: colors, motion, the debug log, and what budctl is. */
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
    val version = remember { context.packageManager.getPackageInfo(context.packageName, 0).versionName ?: "?" }
    Card {
        SettingRow(Icon.Info, "Version") { ValueChip(version) }
        SettingRow(Icon.Phone, "No network access", "budctl has no internet permission")
        SettingRow(Icon.Chip, "License", "GPL-3.0-or-later")
    }
}

/** The log lines the page shows; Copy log copies them all. */
private const val SHOWN_LINES = 60
