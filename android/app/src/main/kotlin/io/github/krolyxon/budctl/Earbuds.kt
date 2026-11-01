package io.github.krolyxon.budctl

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.unit.dp

/** The switch for dual connection, by its key. */
private const val DUAL = "dual"

/**
 * The Earbud settings page (crates/budctl-gui/src/earbuds.rs): dual connection and its devices,
 * find the buds, and what they run. The device that asked the buds for their list is this phone.
 */
@Composable
fun Earbuds(app: App) {
    val buds = app.buds
    buds.switches.find { it.key == DUAL }?.let { dual ->
        Heading("connection")
        Card {
            SettingRow(
                Icon.Dual,
                "Dual connection",
                when {
                    app.saving(Item.Switch(DUAL)) -> SAVING
                    dual.on -> "Your buds stay connected to two devices at once, like this phone and your computer"
                    else -> "Your buds connect to one device at a time"
                },
                onClick = { app.switch(DUAL, !dual.on) },
            ) { Toggle(dual.on) }
        }
    }
    buds.dual?.let { Devices(it) }
    Heading("find my earbuds")
    Card {
        val about = when {
            app.saving(Item.Find) -> if (app.ringing) "Stopping…" else "Starting…"
            app.ringing -> "Ringing: stop it once you have found them"
            else -> "Plays a loud sound from both buds"
        }
        SettingRow(Icon.Find, "Find my earbuds", about) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                // A blinking live dot while the buds ring.
                if (app.ringing) Light(LocalLook.current.p.live, 8.dp, blinking = true)
                Pill(if (app.ringing) "Stop" else "Ring", filled = app.ringing, onClick = app::find)
            }
        }
    }
    Heading("about these buds")
    Card {
        SettingRow(Icon.Earbud, "Name") { ValueChip(buds.name) }
        SettingRow(Icon.Chip, "Firmware") { ValueChip(buds.firmware ?: "unknown") }
        SettingRow(Icon.Info, "Audio codec") { ValueChip(buds.codec ?: "unknown") }
    }
}

/**
 * The devices dual connection keeps, as the buds list them. Nothing here can be changed: the buds
 * take no command to connect, disconnect or forget one.
 */
@Composable
private fun Devices(list: List<DualDevice>) {
    Heading("devices")
    if (list.isEmpty()) {
        Note("The buds list no devices.")
    } else {
        Card {
            for (device in list) {
                SettingRow(
                    if (device.thisDevice) Icon.Phone else Icon.Laptop,
                    device.name.ifEmpty { "Unnamed device" },
                    state(device) + "\n" + device.address,
                    current = device.connected,
                )
            }
        }
    }
    Note("These buds cannot connect, disconnect or forget a device from here.")
}

/** "Connected · this phone", "Not connected"… */
private fun state(device: DualDevice) = buildList {
    add(if (device.connected) "Connected" else "Not connected")
    if (device.thisDevice) add("this phone")
    if (device.mainAudio) add("main audio")
    if (device.playing) add("playing")
}.joinToString(" · ")
