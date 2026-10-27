//! The Earbud settings page: find the buds, and what they run.

use iced::Element;
use iced::widget::{column, text};

use crate::buds::Buds;
use crate::icons::Icon;
use crate::theme::Look;
use crate::ui;
use crate::{Item, Marks, Message};

/// What the keyboard cursor can rest on.
pub fn items() -> Vec<Item> {
    vec![Item::Find]
}

/// `ringing`: the buds were told to ring and not yet to stop.
pub fn view<'a>(look: Look, buds: &'a Buds, ringing: bool, marks: Marks) -> Element<'a, Message> {
    let p = look.p;
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
    column![
        ui::title(look, "Earbud settings"),
        ui::card(look, vec![find]),
        column![ui::heading(look, "About these buds"), ui::card(look, facts)].spacing(10),
    ]
    .spacing(18)
    .into()
}
