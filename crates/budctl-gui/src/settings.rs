//! The App settings page: where the colors come from, the keys, and what budctl is.

use std::path::Path;

use iced::Element;
use iced::widget::{column, space, text};

use crate::Message;
use crate::icons::Icon;
use crate::theme::Look;
use crate::ui;

/// Where the colors come from: the palette file followed, if any, and why it could not be used.
pub struct Colors<'a> {
    pub file: Option<&'a Path>,
    pub problem: Option<&'a str>,
}

pub fn view<'a>(look: Look, colors: Colors<'a>) -> Element<'a, Message> {
    let p = look.p;
    let source = match (colors.file, colors.problem) {
        (_, Some(problem)) => format!("Built in: {problem}"),
        (Some(file), None) if file.exists() => format!("From {}", file.display()),
        _ => "Built in".to_owned(),
    };
    let none = || -> Element<'a, Message> { space().into() };
    let palette = ui::setting(look, Icon::Settings, "Colors", Some(source), none(), None, false);
    let how = text(
        "budctl follows a matugen palette with Aurora's keys (bg, fg, primary, surface…): the file given \
         with --theme, else ~/.config/budctl/theme.json, which starts out with the built-in colors. \
         It is read again whenever the window gets focus.",
    )
    .size(13)
    .color(p.dim);
    let keys = ui::setting(
        look,
        Icon::Keys,
        "Keys",
        Some("Vim-like keys work on every page".to_owned()),
        ui::pill(look, "Show keys", false, Message::ShowHelp),
        None,
        false,
    );
    let files = ui::setting(
        look,
        Icon::Settings,
        "Preferences",
        Some("~/.config/budctl/config.json: default device, idle time, start page. Read at launch.".to_owned()),
        none(),
        None,
        false,
    );
    let log = ui::setting(
        look,
        Icon::Info,
        "Log",
        Some("~/.local/state/budctl/gui.log: errors and session events, started afresh each launch.".to_owned()),
        none(),
        None,
        false,
    );
    let about = ui::setting(
        look,
        Icon::Info,
        concat!("budctl ", env!("CARGO_PKG_VERSION")),
        Some("GPL-3.0-or-later. No telemetry, no account: it talks to your buds over Bluetooth only.".to_owned()),
        none(),
        None,
        false,
    );
    column![
        ui::title(look, "App settings"),
        column![ui::card(look, vec![palette]), how].spacing(10),
        ui::card(look, vec![keys]),
        ui::card(look, vec![files, log]),
        ui::card(look, vec![about]),
    ]
    .spacing(18)
    .into()
}
