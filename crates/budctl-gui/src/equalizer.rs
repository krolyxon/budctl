//! The Equalizer page: the presets, and the bands of the custom ones.

use budctl_core::status::{EqBand, EqPreset};
use iced::widget::{Row, column, container, row, slider, space, text, vertical_slider};
use iced::{Alignment, Border, Element, Length};

use crate::buds::{self, Buds};
use crate::icons::{self, Icon};
use crate::theme::Look;
use crate::ui;
use crate::{Draft, Item, Marks, Message};

/// Shown instead of a subtitle while a write is on its way.
const SAVING: &str = "Saving…";

/// What the keyboard cursor can rest on, top to bottom, as [`view`] shows them: each preset,
/// "New preset", then for each custom preset "Rename" and its bands.
pub fn items(buds: &Buds) -> Vec<Item> {
    let mut items = Vec::new();
    if let Some(eq) = &buds.eq {
        items.extend(eq.presets.iter().map(|p| Item::Preset(p.id)));
        items.push(Item::NewPreset);
        for preset in eq.presets.iter().filter(|p| p.custom) {
            items.push(Item::Rename(preset.id));
            items.extend(preset.bands.iter().map(|band| Item::Band(preset.id, band.hz)));
        }
    }
    items
}

/// `draft`: bands moved on the screen and not saved on the buds yet.
pub fn view<'a>(look: Look, buds: &'a Buds, draft: Option<&'a Draft>, marks: Marks) -> Element<'a, Message> {
    let title = ui::title(look, "Equalizer");
    let Some(eq) = &buds.eq else {
        let none = text("These buds did not list their presets.")
            .size(14)
            .color(look.p.dim);
        return column![title, none].spacing(12).into();
    };
    let current = eq.current.as_deref();
    let rows = eq
        .presets
        .iter()
        .map(|preset| {
            let chosen = current == Some(buds::key(preset).as_str());
            let subtitle = if marks.pending.contains(&Item::Preset(preset.id)) {
                SAVING
            } else if preset.custom {
                "Your preset"
            } else {
                "Built in"
            };
            let mark: Element<'a, Message> = if chosen {
                icons::icon(Icon::Check, 20.0, look.p.icon)
            } else {
                space().width(20).into()
            };
            ui::setting(
                look,
                Icon::Equalizer,
                buds::shown_name(preset),
                Some(subtitle.to_owned()),
                mark,
                Some(Message::Preset(preset.id)),
                marks.focus == Some(Item::Preset(preset.id)),
            )
        })
        .collect();
    eq.presets
        .iter()
        .filter(|p| p.custom)
        .fold(
            column![title, ui::card(look, rows), new_preset(look, marks)].spacing(18),
            |page, preset| page.push(bands(look, preset, draft, marks)),
        )
        .into()
}

/// The button that creates a custom preset, and what it is doing.
fn new_preset<'a>(look: Look, marks: Marks) -> Element<'a, Message> {
    let creating = marks
        .pending
        .contains(&Item::NewPreset)
        .then(|| text("Creating…").size(13).color(look.p.icon));
    let about = text("A new preset starts flat, with the bands of your other presets, and is put to use.")
        .size(13)
        .color(look.p.dim)
        .width(Length::Fill);
    row![about]
        .push(creating)
        .push(ui::pill_with(
            look,
            "New preset",
            false,
            marks.focus == Some(Item::NewPreset),
            Message::NewPreset,
        ))
        .spacing(14)
        .align_y(Alignment::Center)
        .padding([0, 8])
        .into()
}

/// A custom preset's bands, one slider each.
fn bands<'a>(look: Look, preset: &'a EqPreset, draft: Option<&'a Draft>, marks: Marks) -> Element<'a, Message> {
    let p = look.p;
    let id = preset.id;
    let gains: &[EqBand] = match draft {
        Some(draft) if draft.preset == id => &draft.bands,
        _ => &preset.bands,
    };
    let (low, high) = preset.range.unwrap_or((-6, 6));
    let saving = marks
        .pending
        .contains(&Item::Preset(id))
        .then(|| text(SAVING).size(13).color(p.icon));
    let heading = row![
        text(preset.name.as_str()).size(16).font(look.medium()).color(p.fg),
        text(format!("{low:+} to {high:+} dB"))
            .size(13)
            .color(p.dim)
            .width(Length::Fill)
    ]
    .push(saving)
    .push(ui::pill_with(
        look,
        "Rename",
        false,
        marks.focus == Some(Item::Rename(id)),
        Message::RenamePreset(id),
    ))
    .spacing(10)
    .align_y(Alignment::Center);
    let sliders = gains.iter().fold(Row::new().spacing(6), |sliders, &band| {
        let focused = marks.focus == Some(Item::Band(id, band.hz));
        let hz = band.hz;
        let slider = vertical_slider(i16::from(low)..=i16::from(high), i16::from(band.gain), move |gain| {
            Message::Band {
                preset: id,
                hz,
                gain: i8::try_from(gain).unwrap_or(0),
                save: false,
            }
        })
        .on_release(Message::SaveBands(id))
        .height(170)
        .style(move |_, _| slider::Style {
            rail: slider::Rail {
                backgrounds: (p.accent.into(), p.surface_highest.into()),
                width: 6.0,
                border: Border::default().rounded(3),
            },
            handle: slider::Handle {
                shape: slider::HandleShape::Circle { radius: 9.0 },
                background: p.fg.into(),
                border_width: 2.0,
                border_color: p.accent,
            },
        });
        let band = column![
            text(gain(band.gain)).size(14).font(look.medium()).color(p.fg),
            slider,
            text(frequency(hz)).size(12).color(p.dim)
        ]
        .spacing(10)
        .align_x(Alignment::Center);
        sliders.push(
            container(band)
                .center_x(Length::Fill)
                .padding([12, 0])
                .style(move |_| container::Style {
                    background: focused.then_some(p.surface_high.into()),
                    border: Border {
                        color: p.accent,
                        width: if focused { 2.0 } else { 0.0 },
                        radius: 12.0.into(),
                    },
                    ..container::Style::default()
                }),
        )
    });
    ui::panel(look, column![heading, sliders].spacing(16).into())
}

/// `3` → "+3", `0` → "0", `-2` → "-2".
pub fn gain(db: i8) -> String {
    if db == 0 { "0".to_owned() } else { format!("{db:+}") }
}

/// `50` → "50 Hz", `16000` → "16 kHz".
fn frequency(hz: u16) -> String {
    if hz >= 1000 && hz.is_multiple_of(1000) {
        format!("{} kHz", hz / 1000)
    } else {
        format!("{hz} Hz")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn items_follow_the_page() {
        let items = items(&buds::demo());
        assert_eq!(items.len(), 5 + 1 + 1 + 6);
        assert_eq!(items.first(), Some(&Item::Preset(0)));
        assert_eq!(items.get(5), Some(&Item::NewPreset));
        assert_eq!(items.get(6), Some(&Item::Rename(4)));
        assert_eq!(items.get(7), Some(&Item::Band(4, 50)));
        assert_eq!(items.last(), Some(&Item::Band(4, 16000)));
        assert!(super::items(&buds::unknown()).is_empty());
    }

    #[test]
    fn gains() {
        assert_eq!(gain(3), "+3");
        assert_eq!(gain(0), "0");
        assert_eq!(gain(-6), "-6");
    }

    #[test]
    fn frequencies() {
        assert_eq!(frequency(50), "50 Hz");
        assert_eq!(frequency(1000), "1 kHz");
        assert_eq!(frequency(16000), "16 kHz");
        assert_eq!(frequency(62), "62 Hz");
    }
}
