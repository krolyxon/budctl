//! The Earbud settings page: dual connection and its devices, find the buds, and what they run.

use budctl_core::status::DualDevice;
use iced::Element;
use iced::widget::{column, text};

use crate::buds::Buds;
use crate::icons::Icon;
use crate::theme::Look;
use crate::ui;
use crate::{Item, Marks, Message};

/// The switch for dual connection, by its key.
pub const DUAL: &str = "dual";

/// What the keyboard cursor can rest on, top to bottom.
pub fn items(buds: &Buds) -> Vec<Item> {
    let dual = buds.status.switches.iter().any(|s| s.key == DUAL);
    dual.then_some(Item::Switch(DUAL))
        .into_iter()
        .chain([Item::Find])
        .collect()
}

/// `ringing`: the buds were told to ring and not yet to stop.
pub fn view<'a>(look: Look, buds: &'a Buds, ringing: bool, marks: Marks) -> Element<'a, Message> {
    let p = look.p;
    let dual = buds.status.switches.iter().find(|s| s.key == DUAL).map(|dual| {
        let about = if marks.pending.contains(&Item::Switch(DUAL)) {
            "Saving…"
        } else if dual.on {
            "Your buds stay connected to two devices at once, like your phone and this computer"
        } else {
            "Your buds connect to one device at a time"
        };
        let row = ui::setting(
            look,
            Icon::Dual,
            "Dual connection",
            Some(about.to_owned()),
            ui::switch(look, dual.on),
            Some(Message::Switch(DUAL, !dual.on)),
            marks.focus == Some(Item::Switch(DUAL)),
        );
        column![ui::heading(look, "Connection"), ui::card(look, vec![row])].spacing(10)
    });
    let about = match (marks.pending.contains(&Item::Find), ringing) {
        (true, false) => "Starting…",
        (true, true) => "Stopping…",
        (false, true) => "Ringing: stop it once you have found them",
        (false, false) => "Plays a loud sound from both buds",
    };
    let find = ui::setting(
        look,
        Icon::Find,
        "Find my earbuds",
        Some(about.to_owned()),
        ui::pill(look, if ringing { "Stop" } else { "Ring" }, ringing, Message::Find),
        None,
        marks.focus == Some(Item::Find),
    );
    let value = |value: Option<&'a str>| -> Element<'a, Message> {
        text(value.unwrap_or("Unknown")).size(14).color(p.dim).into()
    };
    let status = &buds.status;
    let facts = vec![
        ui::setting(look, Icon::Earbud, "Name", None, value(Some(&buds.name)), None, false),
        ui::setting(
            look,
            Icon::Chip,
            "Firmware",
            None,
            value(status.firmware.as_deref()),
            None,
            false,
        ),
        ui::setting(
            look,
            Icon::Info,
            "Audio codec",
            None,
            value(status.codec.as_deref()),
            None,
            false,
        ),
    ];
    column![ui::title(look, "Earbud settings")]
        .push(dual)
        .push(buds.dual.as_deref().map(|list| devices(look, list)))
        .push(column![ui::heading(look, "Find my earbuds"), ui::card(look, vec![find])].spacing(10))
        .push(column![ui::heading(look, "About these buds"), ui::card(look, facts)].spacing(10))
        .spacing(18)
        .into()
}

/// The devices dual connection keeps, as the buds list them. Nothing here can be changed: the
/// buds take no command to connect, disconnect or forget one.
fn devices<'a>(look: Look, list: &'a [DualDevice]) -> Element<'a, Message> {
    let p = look.p;
    let rows: Vec<Element<'a, Message>> = list
        .iter()
        .map(|device| {
            let icon = if device.this_computer {
                Icon::Laptop
            } else {
                Icon::Phone
            };
            let name = if device.name.is_empty() {
                "Unnamed device"
            } else {
                device.name.as_str()
            };
            let address = text(device.address.to_string()).size(13).color(p.dim);
            ui::setting(look, icon, name, Some(state(device)), address.into(), None, false)
        })
        .collect();
    let list: Element<'a, Message> = if rows.is_empty() {
        text("The buds list no devices.").size(13).color(p.dim).into()
    } else {
        ui::card(look, rows)
    };
    let note = text("These buds cannot connect, disconnect or forget a device from here.")
        .size(13)
        .color(p.dim);
    column![ui::heading(look, "Devices"), list, note].spacing(10).into()
}

/// "Connected · this computer", "Not connected"…
fn state(device: &DualDevice) -> String {
    let mut words = vec![if device.connected { "Connected" } else { "Not connected" }];
    for (on, what) in [
        (device.this_computer, "this computer"),
        (device.main_audio, "main audio"),
        (device.playing, "playing"),
    ] {
        if on {
            words.push(what);
        }
    }
    words.join(" · ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buds;

    #[test]
    fn device_states_in_words() {
        let dual = buds::demo().dual.unwrap();
        let words: Vec<String> = dual.iter().map(state).collect();
        assert_eq!(words, ["Connected · this computer", "Connected · main audio · playing"]);
        let mut gone = dual[1].clone();
        gone.connected = false;
        gone.main_audio = false;
        gone.playing = false;
        assert_eq!(state(&gone), "Not connected");
    }
}
