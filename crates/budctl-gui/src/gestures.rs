//! The Gestures page: what each tap and hold does on each bud, and what a hold steps through.

use std::fmt;

use budctl_core::status::TouchControls;
use iced::widget::overlay::menu;
use iced::widget::{column, pick_list};
use iced::{Border, Element, Shadow};

use crate::buds::{self, Buds};
use crate::icons::Icon;
use crate::theme::Look;
use crate::ui;
use crate::{Item, Marks, Message, TouchDraft};

/// The buds, as the page shows them.
const SIDES: [(&str, &str); 2] = [("left", "Left bud"), ("right", "Right bud")];

/// Shown instead of a subtitle while a write is on its way.
const SAVING: &str = "Saving…";

/// One gesture of one bud, and the actions it can be given.
pub struct Row<'a> {
    pub side: &'static str,
    pub gesture: &'a str,
    pub actions: &'a [String],
}

/// The page's gesture rows, in order: each bud's gestures, in the order the buds offer them.
/// [`Item::Gesture`] counts in this list.
pub fn rows(touch: &TouchControls) -> Vec<Row<'_>> {
    SIDES
        .iter()
        .flat_map(|&(side, _)| {
            touch.choices.iter().map(move |choice| Row {
                side,
                gesture: &choice.gesture,
                actions: &choice.actions,
            })
        })
        .collect()
}

/// What the keyboard cursor can rest on, top to bottom: the gestures, then the hold-cycle modes.
pub fn items(buds: &Buds) -> Vec<Item> {
    let Some(touch) = &buds.touch else {
        return Vec::new();
    };
    let count = |n: usize| (0..n).filter_map(|i| u8::try_from(i).ok());
    count(rows(touch).len())
        .map(Item::Gesture)
        .chain(count(touch.cycle_modes.len()).map(Item::Cycle))
        .collect()
}

/// `draft`: an action picked on the screen and not on the buds yet.
pub fn view<'a>(look: Look, buds: &'a Buds, draft: Option<&'a TouchDraft>, marks: Marks) -> Element<'a, Message> {
    let p = look.p;
    let title = ui::title(look, "gestures · taps and holds on each bud");
    let Some(touch) = &buds.touch else {
        let none = ui::note(look, "These buds did not report their touch controls.");
        return column![title, none].spacing(12).into();
    };
    let rows = rows(touch);
    let mut page = column![title].spacing(18);
    for (side, heading) in SIDES {
        let gestures = rows
            .iter()
            .enumerate()
            .filter(|(_, row)| row.side == side)
            .filter_map(|(n, row)| Some((u8::try_from(n).ok()?, row)))
            .map(|(n, row)| {
                let shown = draft
                    .filter(|draft| draft.row == n)
                    .map(|draft| draft.action.as_str())
                    .or_else(|| buds.action(side, row.gesture))
                    .unwrap_or("none");
                let options: Vec<Choice> = row.actions.iter().map(|a| Choice::new(a)).collect();
                let picker = pick_list(options, Some(Choice::new(shown)), move |choice: Choice| {
                    Message::Action {
                        row: n,
                        action: choice.key,
                        save: true,
                    }
                })
                .width(230)
                .padding([10, 18])
                .text_size(14)
                .font(look.medium())
                .style(move |_, status| pick_list::Style {
                    text_color: p.fg,
                    placeholder_color: p.grey,
                    handle_color: p.grey,
                    background: if matches!(status, pick_list::Status::Hovered | pick_list::Status::Opened { .. }) {
                        p.mute
                    } else {
                        p.s2
                    }
                    .into(),
                    border: Border::default().rounded(999),
                })
                .menu_style(move |_| menu::Style {
                    background: p.s2.into(),
                    border: Border::default().rounded(20),
                    text_color: p.fg,
                    selected_text_color: p.fill_fg,
                    selected_background: p.fill.into(),
                    shadow: Shadow::default(),
                });
                let saving = marks.pending.contains(&Item::Gesture(n)).then(|| SAVING.to_owned());
                ui::setting(
                    look,
                    gesture_icon(row.gesture),
                    gesture_name(row.gesture),
                    saving,
                    picker.into(),
                    None,
                    marks.focus == Some(Item::Gesture(n)),
                )
            })
            .collect();
        page = page.push(column![ui::heading(look, heading), ui::card(look, gestures)].spacing(10));
    }
    if let (Some(cycle), false) = (&touch.hold_cycle, touch.cycle_modes.is_empty()) {
        let modes = touch
            .cycle_modes
            .iter()
            .enumerate()
            .filter_map(|(n, mode)| Some((u8::try_from(n).ok()?, mode)))
            .map(|(n, mode)| {
                let on = cycle.contains(mode);
                let about = if marks.pending.contains(&Item::Cycle(n)) {
                    Some(SAVING.to_owned())
                } else if on && cycle.len() <= 2 {
                    Some("A hold needs two modes to step through".to_owned())
                } else {
                    None
                };
                ui::setting(
                    look,
                    mode_icon(mode),
                    mode_name(mode),
                    about,
                    ui::switch(look, on),
                    Some(Message::Cycle(n, !on)),
                    marks.focus == Some(Item::Cycle(n)),
                )
            })
            .collect();
        page = page.push(column![ui::heading(look, "A hold steps through"), ui::card(look, modes)].spacing(10));
    }
    page.into()
}

/// An action, for the dropdown: its name on the buds, and how it reads.
#[derive(Debug, Clone, PartialEq)]
struct Choice {
    key: String,
    label: String,
}

impl Choice {
    fn new(key: &str) -> Self {
        Choice {
            key: key.to_owned(),
            label: action_name(key),
        }
    }
}

impl fmt::Display for Choice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.label)
    }
}

fn gesture_name(gesture: &str) -> String {
    match gesture {
        "hold" => "Touch and hold".to_owned(),
        other => buds::words(other),
    }
}

fn gesture_icon(gesture: &str) -> Icon {
    match gesture {
        "hold" => Icon::Earbud,
        _ => Icon::Gestures,
    }
}

pub fn action_name(action: &str) -> String {
    match action {
        "none" => "Nothing",
        "play-pause" => "Play / pause",
        "next" => "Next track",
        "previous" => "Previous track",
        "voice-assistant" => "Voice assistant",
        "noise-cycle" => "Noise control",
        other => return buds::words(other),
    }
    .to_owned()
}

fn mode_name(mode: &str) -> String {
    match mode {
        "anc" => "Noise cancelling".to_owned(),
        other => buds::words(other),
    }
}

fn mode_icon(mode: &str) -> Icon {
    match mode {
        "off" => Icon::NoiseOff,
        "anc" => Icon::Anc,
        _ => Icon::Transparency,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_go_bud_by_bud() {
        let buds = buds::demo();
        let touch = buds.touch.as_ref().unwrap();
        let rows = rows(touch);
        let names: Vec<(&str, &str)> = rows.iter().map(|r| (r.side, r.gesture)).collect();
        assert_eq!(
            names,
            [
                ("left", "double-tap"),
                ("left", "triple-tap"),
                ("left", "hold"),
                ("right", "double-tap"),
                ("right", "triple-tap"),
                ("right", "hold"),
            ]
        );
        let items = items(&buds);
        assert_eq!(items.len(), 6 + 3);
        assert_eq!(items.last(), Some(&Item::Cycle(2)));
    }

    #[test]
    fn names_read_well() {
        assert_eq!(action_name("play-pause"), "Play / pause");
        assert_eq!(action_name("volume-up"), "Volume up");
        assert_eq!(gesture_name("triple-tap"), "Triple tap");
        assert_eq!(mode_name("anc"), "Noise cancelling");
    }
}
