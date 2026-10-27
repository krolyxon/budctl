//! The Overview page: battery, noise control, the switches, and the way to the other pages.

use budctl_core::status::{AncLevel, Charge, NoiseMode, Switch};
use iced::widget::{column, container, row, text};
use iced::{Alignment, Element, Length};

use crate::demo::Buds;
use crate::icons::{self, Icon, Part, RingColors};
use crate::theme::Look;
use crate::ui::{self, Segment};
use crate::{Message, Page};

pub fn view<'a>(look: Look, buds: &'a Buds, anc_level: AncLevel) -> Element<'a, Message> {
    column![ui::title(look, "Overview"), battery(look, buds)]
        .push(
            buds.status
                .noise_control
                .as_ref()
                .map(|now| noise_control(look, &now.mode, anc_level)),
        )
        .push(switches(look, &buds.status.switches))
        .push(more(look, buds))
        .spacing(18)
        .into()
}

fn battery<'a>(look: Look, buds: &Buds) -> Element<'a, Message> {
    let p = look.p;
    let colors = RingColors {
        track: p.surface_highest,
        charge: p.accent,
        part: p.fg,
        tip: p.dim,
        cutout: p.surface,
    };
    let gauge = |part, label: &'a str, charge: Option<Charge>| -> Element<'a, Message> {
        let reading = match charge {
            Some(charge) => format!("{}%", charge.level),
            None => "—".to_owned(),
        };
        let figure = row![text(reading).size(21).font(look.bold()).color(p.fg)]
            .push(
                charge
                    .filter(|c| c.charging)
                    .map(|_| icons::icon(Icon::Bolt, 16.0, p.icon)),
            )
            .spacing(4)
            .align_y(Alignment::Center);
        column![
            icons::battery_ring(part, charge.map(|c| c.level), 112.0, colors),
            figure,
            text(label).size(13).color(p.dim)
        ]
        .spacing(6)
        .align_x(Alignment::Center)
        .width(Length::Fill)
        .into()
    };
    let battery = buds.status.battery;
    let gauges = row![
        gauge(Part::Left, "Left", battery.left),
        gauge(Part::Case, "Case", battery.case),
        gauge(Part::Right, "Right", battery.right),
    ];
    ui::card(look, vec![container(gauges).padding([22, 8]).into()])
}

fn noise_control<'a>(look: Look, mode: &str, anc_level: AncLevel) -> Element<'a, Message> {
    let modes = [
        (Icon::NoiseOff, "Off", NoiseMode::Off),
        (Icon::Anc, "Noise cancelling", NoiseMode::Anc(anc_level)),
        (Icon::Transparency, "Transparency", NoiseMode::Transparency),
    ];
    let segments = modes
        .into_iter()
        .map(|(icon, label, set)| Segment {
            icon: Some(icon),
            label,
            selected: set.name() == mode,
            on_press: Message::Noise(set),
        })
        .collect();
    let levels: Vec<Segment> = [
        (AncLevel::Light, "Light"),
        (AncLevel::Medium, "Medium"),
        (AncLevel::Deep, "Deep"),
    ]
    .into_iter()
    .map(|(level, label)| Segment {
        icon: None,
        label,
        selected: level == anc_level,
        on_press: Message::Noise(NoiseMode::Anc(level)),
    })
    .collect();
    let strength = row![
        text("Strength").size(13.5).color(look.p.dim).width(Length::Fill),
        container(ui::segmented(look, levels, false)).width(300)
    ]
    .align_y(Alignment::Center)
    .padding([0, 8]);
    column![ui::segmented(look, segments, true)]
        .push((mode == NoiseMode::Anc(anc_level).name()).then_some(strength))
        .spacing(12)
        .into()
}

/// How the page shows the switches it knows, in this order. Others still show, last, with a
/// plain icon and no subtitle.
const SWITCHES: [(&str, Icon, &str); 5] = [
    ("game", Icon::Game, "Lower latency, for games and video"),
    ("spatial", Icon::Spatial, "Wider, surround-like sound"),
    ("bass-boost", Icon::Bass, "Stronger low end"),
    ("wind", Icon::Wind, "Less wind noise outdoors"),
    ("dual", Icon::Dual, "Stay connected to two devices at once"),
];

fn switches<'a>(look: Look, switches: &[Switch]) -> Element<'a, Message> {
    let known = |key: &str| SWITCHES.iter().find(|(k, _, _)| *k == key);
    let rank = |s: &&Switch| {
        SWITCHES
            .iter()
            .position(|(k, _, _)| *k == s.key)
            .unwrap_or(SWITCHES.len())
    };
    let mut shown: Vec<&Switch> = switches.iter().collect();
    shown.sort_by_key(rank);
    let rows = shown
        .into_iter()
        .map(|s| {
            let (icon, subtitle) = known(s.key).map_or((Icon::Settings, None), |&(_, icon, subtitle)| {
                (icon, Some(subtitle.to_owned()))
            });
            ui::setting(
                look,
                icon,
                s.label,
                subtitle,
                ui::switch(look, s.on),
                Message::Switch(s.key, !s.on),
            )
        })
        .collect();
    ui::card(look, rows)
}

fn more<'a>(look: Look, buds: &Buds) -> Element<'a, Message> {
    let open = |icon, title, subtitle: &str, page| {
        ui::setting(
            look,
            icon,
            title,
            Some(subtitle.to_owned()),
            ui::chevron(look),
            Message::Open(page),
        )
    };
    ui::card(
        look,
        vec![
            open(Icon::Equalizer, "Equalizer", &buds.eq_name, Page::Equalizer),
            open(Icon::Gestures, "Gestures", "Taps and holds on each bud", Page::Gestures),
            open(
                Icon::Earbud,
                "Earbud settings",
                "Find my earbuds, firmware",
                Page::Earbuds,
            ),
        ],
    )
}
