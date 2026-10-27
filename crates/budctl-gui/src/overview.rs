//! The Overview page: battery, noise control, sound, and the buds' other settings.

use budctl_core::status::{AncLevel, Charge, NoiseMode, Switch};
use iced::Element;

use crate::demo::Buds;
use crate::glyphs;
use crate::theme::Look;
use crate::ui::{self, Opt};
use crate::{Message, Page};

/// The switches the "sound" section shows, in this order. The others go under "buds".
const SOUND: [&str; 3] = ["game", "spatial", "bass-boost"];

/// The line under a switch's name, for the switches the GUI knows.
fn about(key: &str) -> Option<&'static str> {
    match key {
        "game" => Some("lower latency, for games and video"),
        "spatial" => Some("wider, surround-like sound"),
        "bass-boost" => Some("stronger low end"),
        "wind" => Some("less wind noise outdoors"),
        "dual" => Some("stay connected to two devices at once"),
        _ => None,
    }
}

pub fn view<'a>(look: Look, buds: &'a Buds, anc_level: AncLevel) -> Element<'a, Message> {
    let status = &buds.status;
    let switch = |s: &Switch| {
        ui::switch_row(
            look,
            s.label.to_lowercase(),
            about(s.key),
            s.on,
            Message::Switch(s.key, !s.on),
        )
    };

    let mut sound = vec![ui::section(look, "sound")];
    sound.extend(
        SOUND
            .iter()
            .filter_map(|key| status.switches.iter().find(|s| s.key == *key))
            .map(switch),
    );
    sound.push(ui::link_row(
        look,
        "equalizer",
        Some("presets, and your own"),
        Some(buds.eq_name.clone()),
        Message::Open(Page::Equalizer),
    ));

    let mut rest = vec![ui::section(look, "buds")];
    rest.extend(status.switches.iter().filter(|s| !SOUND.contains(&s.key)).map(switch));
    rest.push(ui::link_row(
        look,
        "gestures",
        Some("taps and holds on each bud"),
        None,
        Message::Open(Page::Gestures),
    ));
    rest.push(ui::link_row(
        look,
        "earbuds",
        Some("find my earbuds, firmware"),
        None,
        Message::Open(Page::Earbuds),
    ));

    let mut sections = vec![battery(look, buds)];
    sections.extend(
        status
            .noise_control
            .as_ref()
            .map(|now| noise_control(look, &now.mode, anc_level)),
    );
    sections.extend([sound, rest]);
    ui::sections(sections)
}

fn battery<'a>(look: Look, buds: &Buds) -> Vec<Element<'a, Message>> {
    let p = look.p;
    let bar = |name: &'a str, charge: Option<Charge>| match charge {
        Some(charge) => {
            let low = charge.level <= 15 && !charge.charging;
            let value = if charge.charging {
                format!("{}%  charging", charge.level)
            } else {
                format!("{}%", charge.level)
            };
            let fraction = f32::from(charge.level.min(100)) / 100.0;
            ui::stat_bar(look, name, fraction, value, if low { p.err } else { p.accent })
        }
        None => ui::stat_bar(look, name, 0.0, "unknown".to_owned(), p.accent),
    };
    let battery = buds.status.battery;
    vec![
        ui::section(look, "battery"),
        bar("left", battery.left),
        bar("right", battery.right),
        bar("case", battery.case),
    ]
}

fn noise_control<'a>(look: Look, mode: &str, anc_level: AncLevel) -> Vec<Element<'a, Message>> {
    let modes = [
        (glyphs::NOISE_OFF, "off", NoiseMode::Off),
        (glyphs::ANC, "noise cancelling", NoiseMode::Anc(anc_level)),
        (glyphs::TRANSPARENCY, "transparency", NoiseMode::Transparency),
    ];
    let options = modes
        .into_iter()
        .map(|(glyph, words, set)| Opt {
            glyph: Some(glyph),
            words,
            chosen: set.name() == mode,
            on_press: Message::Noise(set),
        })
        .collect();
    let mut rows = vec![ui::section(look, "noise control"), ui::choice(look, "mode", options)];
    if mode == NoiseMode::Anc(anc_level).name() {
        let levels = [
            (AncLevel::Light, "light"),
            (AncLevel::Medium, "medium"),
            (AncLevel::Deep, "deep"),
        ]
        .into_iter()
        .map(|(level, words)| Opt {
            glyph: None,
            words,
            chosen: level == anc_level,
            on_press: Message::Noise(NoiseMode::Anc(level)),
        })
        .collect();
        rows.push(ui::choice(look, "strength", levels));
    }
    rows
}
