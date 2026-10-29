//! The Overview page: battery, noise control, the switches, and the way to the other pages.

use budctl_core::status::{AncLevel, Charge, NoiseMode, Switch};
use iced::widget::{column, container, row};
use iced::{Alignment, Element, Length};

use crate::buds::Buds;
use crate::dots;
use crate::equalizer;
use crate::icons::{self, Icon};
use crate::theme::Look;
use crate::ui::{self, Segment};
use crate::{Item, Marks, Message, Page};

/// What the keyboard cursor can rest on, top to bottom, as [`view`] shows them.
pub fn items(buds: &Buds) -> Vec<Item> {
    let mut items = Vec::new();
    if let Some(now) = &buds.status.noise_control {
        items.push(Item::NoiseMode);
        if now.mode == NoiseMode::Anc(AncLevel::Deep).name() {
            items.push(Item::AncLevel);
        }
    }
    for switch in shown(&buds.status.switches) {
        items.push(Item::Switch(switch.key));
        if switch.key == BASS_BOOST && buds.bass_range.is_some() {
            items.push(Item::Bass);
        }
    }
    items.extend(MORE.map(|(page, _, _)| Item::Open(page)));
    items
}

/// The switch the bass level shows under.
const BASS_BOOST: &str = "bass-boost";

/// Shown instead of an item's subtitle while a write to it is on its way.
const SAVING: &str = "Saving…";

/// `bass`: a bass level moved on the screen and not set on the buds yet.
pub fn view<'a>(
    look: Look,
    buds: &'a Buds,
    anc_level: AncLevel,
    bass: Option<i8>,
    marks: Marks,
) -> Element<'a, Message> {
    column![
        ui::title(look, "overview · battery, noise control, sound"),
        battery(look, buds)
    ]
    .push(
        buds.status
            .noise_control
            .as_ref()
            .map(|now| noise_control(look, &now.mode, anc_level, marks)),
    )
    .push(switches(look, buds, bass, marks))
    .push(more(look, buds, marks.focus))
    .spacing(18)
    .into()
}

fn battery<'a>(look: Look, buds: &Buds) -> Element<'a, Message> {
    let p = look.p;
    let gauge = |label: &str, charge: Option<Charge>| -> Element<'a, Message> {
        let ring = dots::ring(look, charge.map(|c| c.level), 150.0);
        let charging = charge.is_some_and(|c| c.charging);
        let caption = row![ui::label(look, label, 11.0, p.grey)]
            .push(charging.then(|| icons::icon(Icon::Bolt, 14.0, p.live)))
            .push(charging.then(|| ui::label(look, "charging", 11.0, p.live)))
            .spacing(6)
            .align_y(Alignment::Center);
        column![ring, caption]
            .spacing(14)
            .align_x(Alignment::Center)
            .width(Length::Fill)
            .into()
    };
    let battery = buds.status.battery;
    let gauges = row![
        gauge("left", battery.left),
        gauge("case", battery.case),
        gauge("right", battery.right),
    ];
    ui::card(look, vec![container(gauges).padding([26, 8]).into()])
}

fn noise_control<'a>(look: Look, mode: &str, anc_level: AncLevel, marks: Marks) -> Element<'a, Message> {
    let focus = marks.focus;
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
        container(ui::label(look, "strength", 11.0, look.p.grey)).width(Length::Fill),
        container(ui::segmented(look, levels, false, focus == Some(Item::AncLevel))).width(340)
    ]
    .align_y(Alignment::Center)
    .padding([0, 18]);
    let saving = marks
        .pending
        .contains(&Item::NoiseMode)
        .then(|| container(ui::label(look, SAVING, 11.0, look.p.live)).center_x(Length::Fill));
    column![ui::segmented(look, segments, true, focus == Some(Item::NoiseMode))]
        .push(saving)
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

/// The switches in the order the page shows them.
fn shown(switches: &[Switch]) -> Vec<&Switch> {
    let rank = |s: &&Switch| {
        SWITCHES
            .iter()
            .position(|(k, _, _)| *k == s.key)
            .unwrap_or(SWITCHES.len())
    };
    let mut shown: Vec<&Switch> = switches.iter().collect();
    shown.sort_by_key(rank);
    shown
}

fn switches<'a>(look: Look, buds: &Buds, bass: Option<i8>, marks: Marks) -> Element<'a, Message> {
    let known = |key: &str| SWITCHES.iter().find(|(k, _, _)| *k == key);
    let mut rows = Vec::new();
    for s in shown(&buds.status.switches) {
        rows.push({
            let (icon, about) = known(s.key).map_or((Icon::Settings, None), |&(_, icon, about)| (icon, Some(about)));
            let saving = marks.pending.contains(&Item::Switch(s.key));
            let subtitle = if saving { Some(SAVING) } else { about }.map(str::to_owned);
            ui::setting(
                look,
                icon,
                s.label,
                subtitle,
                ui::switch(look, s.on),
                Some(Message::Switch(s.key, !s.on)),
                marks.focus == Some(Item::Switch(s.key)),
            )
        });
        if let (BASS_BOOST, Some(range)) = (s.key, buds.bass_range) {
            rows.push(bass_level(look, range, bass.or(buds.bass), marks));
        }
    }
    ui::card(look, rows)
}

/// The bass level: the buds take it but do not report it, so the row shows the last level set
/// from this window, if any.
fn bass_level<'a>(look: Look, (low, high): (i8, i8), level: Option<i8>, marks: Marks) -> Element<'a, Message> {
    let p = look.p;
    let about = if marks.pending.contains(&Item::Bass) {
        SAVING
    } else {
        "Last set here: the buds do not report it"
    };
    let shown = level.map_or_else(|| "not set".to_owned(), equalizer::gain);
    let slider = dots::Slider {
        low: i16::from(low),
        high: i16::from(high),
        value: i16::from(level.unwrap_or(0)),
        vertical: false,
        change: |level, save| Message::Bass {
            level: i8::try_from(level).unwrap_or(0),
            save,
        },
        release: Message::SaveBass,
        on: p.fill,
        off: p.s3,
    };
    let control = row![container(ui::value(look, &shown)).align_right(76), slider.view(200.0)]
        .spacing(14)
        .align_y(Alignment::Center);
    ui::setting(
        look,
        Icon::Bass,
        "Bass level",
        Some(about.to_owned()),
        control.into(),
        None,
        marks.focus == Some(Item::Bass),
    )
}

/// The rows that open other pages: the page, its icon and title.
const MORE: [(Page, Icon, &str); 3] = [
    (Page::Equalizer, Icon::Equalizer, "Equalizer"),
    (Page::Gestures, Icon::Gestures, "Gestures"),
    (Page::Earbuds, Icon::Earbud, "Earbud settings"),
];

fn more<'a>(look: Look, buds: &Buds, focus: Option<Item>) -> Element<'a, Message> {
    let rows = MORE
        .into_iter()
        .map(|(page, icon, title)| {
            let subtitle = match page {
                Page::Equalizer => buds.eq_name(),
                Page::Gestures => "Taps and holds on each bud".to_owned(),
                _ => "Dual connection, find my earbuds, firmware".to_owned(),
            };
            ui::setting(
                look,
                icon,
                title,
                Some(subtitle),
                ui::chevron(look),
                Some(Message::Open(page)),
                focus == Some(Item::Open(page)),
            )
        })
        .collect();
    ui::card(look, rows)
}
