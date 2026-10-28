//! The building blocks every page uses, in Nothing OS's shapes: caps labels, dot-matrix words,
//! round cards, rows with their icon in a circle, pill switches, pill tracks and chips.
//! Everything takes its colors from the [`Look`].

use iced::alignment::Horizontal;
use iced::widget::{Column, Row, button, column, container, row, space, text};
use iced::{Alignment, Border, Color, Element, Length};

use crate::Message;
use crate::icons::{self, Icon};
use crate::theme::{Look, Palette};

/// Round corners: cards, and the rows in them.
const CARD: f32 = 28.0;
/// Fully round: pills, chips and circles.
const ROUND: f32 = 999.0;

/// A caps label in the monospace font, like "LEFT" or "BUILT IN".
pub fn label<'a>(look: Look, words: &str, size: f32, color: Color) -> Element<'a, Message> {
    text(words.to_uppercase())
        .size(size)
        .font(look.mono())
        .color(color)
        .into()
}

/// Numbers and short words as LED dots. Never under 26 px: smaller, the dots blur.
pub fn num<'a>(look: Look, words: &str, size: f32, color: Color) -> Element<'a, Message> {
    text(words.to_uppercase())
        .size(size)
        .font(look.dot())
        .color(color)
        .into()
}

/// A page's caption, at its top: "overview · battery, noise control, sound".
pub fn title<'a>(look: Look, caption: &str) -> Element<'a, Message> {
    container(label(look, caption, 12.0, look.p.grey))
        .padding([0, 18])
        .into()
}

/// A dialog's heading.
pub fn headline<'a>(look: Look, words: impl text::IntoFragment<'a>) -> Element<'a, Message> {
    text(words).size(24).font(look.semibold()).color(look.p.fg).into()
}

/// A heading above a card.
pub fn heading<'a>(look: Look, words: &str) -> Element<'a, Message> {
    container(label(look, words, 11.0, look.p.grey)).padding([0, 18]).into()
}

/// Plain words under or beside a card.
pub fn note<'a>(look: Look, words: impl text::IntoFragment<'a>) -> Element<'a, Message> {
    text(words).size(13.5).color(look.p.fg_dim).into()
}

/// A round card holding rows.
pub fn card<'a>(look: Look, rows: Vec<Element<'a, Message>>) -> Element<'a, Message> {
    let p = look.p;
    container(Column::with_children(rows).spacing(2))
        .padding(6)
        .width(Length::Fill)
        .style(move |_| surface(p.s1, CARD))
        .into()
}

/// A round card around content of its own.
pub fn panel<'a>(look: Look, content: Element<'a, Message>) -> Element<'a, Message> {
    let p = look.p;
    container(content)
        .padding(24)
        .width(Length::Fill)
        .style(move |_| surface(p.s1, CARD))
        .into()
}

fn surface(color: Color, radius: f32) -> container::Style {
    container::Style {
        background: Some(color.into()),
        border: Border::default().rounded(radius),
        ..container::Style::default()
    }
}

/// `content` in the middle of the window, `width` wide, over the dimmed page.
pub fn scrim<'a>(look: Look, content: Element<'a, Message>, width: f32) -> Element<'a, Message> {
    let bg = look.p.bg;
    container(container(content).width(width))
        .center(Length::Fill)
        .style(move |_| container::Style {
            background: Some(Color { a: 0.74, ..bg }.into()),
            ..container::Style::default()
        })
        .into()
}

/// A setting row: icon in a circle, title, an optional subtitle, and something on the right.
/// With `on_press`, the whole row is the click target. `focused`: the keyboard cursor is on it.
///
/// A subtitle ending in "…" ("Saving…", "Starting…") says something is under way, and shows
/// in the live color.
pub fn setting<'a>(
    look: Look,
    icon: Icon,
    title: impl text::IntoFragment<'a>,
    subtitle: Option<String>,
    trailing: Element<'a, Message>,
    on_press: Option<Message>,
    focused: bool,
) -> Element<'a, Message> {
    list_row(look, icon, false, title, subtitle, trailing, on_press, focused)
}

/// A [`setting`] row that can be `current`, like the preset in use: its circle is filled.
#[allow(clippy::too_many_arguments)]
pub fn list_row<'a>(
    look: Look,
    icon: Icon,
    current: bool,
    title: impl text::IntoFragment<'a>,
    subtitle: Option<String>,
    trailing: Element<'a, Message>,
    on_press: Option<Message>,
    focused: bool,
) -> Element<'a, Message> {
    let p = look.p;
    let subtitle = subtitle.map(|words| {
        let color = if words.ends_with('…') { p.live } else { p.grey };
        label(look, &words, 10.5, color)
    });
    let words = column![text(title).size(15.5).font(look.medium()).color(p.fg)]
        .push(subtitle)
        .spacing(3)
        .width(Length::Fill);
    let circle = if current {
        p.fill
    } else if focused {
        p.s3
    } else {
        p.s2
    };
    let glyph = if current { p.fill_fg } else { p.fg };
    let content = row![disc(icons::icon(icon, 20.0, glyph), 44.0, circle), words, trailing]
        .spacing(14)
        .align_y(Alignment::Center)
        .padding([6, 14]);
    let content = container(content).padding([0, 6]);
    button(content)
        .on_press_maybe(on_press)
        .padding(0)
        .width(Length::Fill)
        .style(move |_, status| button::Style {
            background: (focused || hovered(status)).then_some(p.s3.into()),
            text_color: p.fg,
            border: cursor(p, focused, CARD),
            ..button::Style::default()
        })
        .into()
}

/// `content` centered in a circle `size` across.
fn disc<'a>(content: Element<'a, Message>, size: f32, color: Color) -> Element<'a, Message> {
    container(content)
        .center(size)
        .style(move |_| surface(color, ROUND))
        .into()
}

/// The keyboard cursor: a 2 px frame in the fill color. No frame without it.
fn cursor(p: Palette, focused: bool, radius: f32) -> Border {
    Border {
        color: p.fill,
        width: if focused { 2.0 } else { 0.0 },
        radius: radius.into(),
    }
}

fn hovered(status: button::Status) -> bool {
    matches!(status, button::Status::Hovered | button::Status::Pressed)
}

/// An on/off switch: off, a dark pill with a dim knob on the left; on, the fill runs to the
/// right end, with a dot in it. It only shows the state: the row around it takes the click.
pub fn switch<'a>(look: Look, on: bool) -> Element<'a, Message> {
    let p = look.p;
    let knob = container(on.then(|| dot(p.fill_fg, 10.0)))
        .width(if on { Length::Fill } else { Length::Fixed(26.0) })
        .height(26)
        .align_x(Horizontal::Right)
        .align_y(Alignment::Center)
        .padding([0, 8])
        .style(move |_| surface(if on { p.fill } else { p.mute }, ROUND));
    container(knob)
        .width(64)
        .height(34)
        .padding(4)
        .style(move |_| surface(p.s2, ROUND))
        .into()
}

/// A chevron: the row opens a page.
pub fn chevron<'a>(look: Look) -> Element<'a, Message> {
    icons::icon(Icon::Chevron, 18.0, look.p.grey)
}

/// One choice of a [`segmented`] control.
pub struct Segment<'a> {
    pub icon: Option<Icon>,
    pub label: &'a str,
    pub selected: bool,
    pub on_press: Message,
}

/// A one-of-n pill track: the chosen cell is filled. `large` puts an icon above each label.
/// `focused`: the keyboard cursor is on it.
pub fn segmented<'a>(look: Look, segments: Vec<Segment<'a>>, large: bool, focused: bool) -> Element<'a, Message> {
    let p = look.p;
    let mut choices = Row::new().spacing(4);
    for segment in segments {
        let selected = segment.selected;
        let color = if selected { p.fill_fg } else { p.grey };
        let words = label(look, segment.label, if large { 11.5 } else { 11.0 }, color);
        let inner: Element<'a, Message> = match segment.icon {
            Some(icon) if large => column![icons::icon(icon, 22.0, if selected { p.fill_fg } else { p.fg }), words]
                .spacing(8)
                .align_x(Alignment::Center)
                .into(),
            _ => words,
        };
        let choice = button(container(inner).center_x(Length::Fill))
            .on_press(segment.on_press)
            .width(Length::Fill)
            .padding(if large { [14, 8] } else { [10, 12] })
            .style(move |_, status| button::Style {
                background: if selected {
                    Some(p.fill.into())
                } else {
                    hovered(status).then_some(p.s3.into())
                },
                text_color: color,
                border: Border::default().rounded(ROUND),
                ..button::Style::default()
            });
        choices = choices.push(choice);
    }
    container(choices)
        .padding(6)
        .width(Length::Fill)
        .style(move |_| container::Style {
            background: Some(p.s2.into()),
            border: cursor(p, focused, ROUND),
            ..container::Style::default()
        })
        .into()
}

/// A page in the sidebar: a dot-matrix word, large with the live dot when open.
pub fn nav_item<'a>(look: Look, word: &str, open: bool, on_press: Message) -> Element<'a, Message> {
    let p = look.p;
    let content: Element<'a, Message> = if open {
        row![dot(p.live, 14.0), num(look, word, 40.0, p.fg)]
            .spacing(14)
            .align_y(Alignment::Center)
            .into()
    } else {
        row![space().width(14), num(look, word, 26.0, p.mute)]
            .spacing(14)
            .align_y(Alignment::Center)
            .into()
    };
    button(content)
        .on_press(on_press)
        .width(Length::Fill)
        .padding([6, 10])
        .style(move |_, status| button::Style {
            background: (!open && hovered(status)).then_some(p.s1.into()),
            text_color: p.fg,
            border: Border::default().rounded(CARD),
            ..button::Style::default()
        })
        .into()
}

/// A chip: a caps label in a pill. `filled` with the fill color, for the main action, or on a
/// dark pill.
pub fn pill<'a>(look: Look, words: &str, filled: bool, on_press: Message) -> Element<'a, Message> {
    pill_with(look, words, filled, false, on_press)
}

/// A [`pill`] that can carry the keyboard cursor.
pub fn pill_with<'a>(look: Look, words: &str, filled: bool, focused: bool, on_press: Message) -> Element<'a, Message> {
    let p = look.p;
    let color = if filled { p.fill_fg } else { p.fg };
    button(label(look, words, 11.0, color))
        .on_press(on_press)
        .padding([10, 18])
        .style(move |_, status| button::Style {
            background: Some(match (filled, hovered(status)) {
                (true, _) => p.fill.into(),
                (false, true) => p.mute.into(),
                (false, false) => p.s3.into(),
            }),
            text_color: color,
            border: cursor(p, focused, ROUND),
            ..button::Style::default()
        })
        .into()
}

/// A round button with just an icon, like the pencil on a preset's row.
pub fn icon_button<'a>(look: Look, icon: Icon, focused: bool, on_press: Message) -> Element<'a, Message> {
    let p = look.p;
    button(container(icons::icon(icon, 18.0, p.fg)).center(40))
        .on_press(on_press)
        .padding(0)
        .style(move |_, status| button::Style {
            background: Some(if focused || hovered(status) { p.mute } else { p.s2 }.into()),
            text_color: p.fg,
            border: cursor(p, focused, ROUND),
            ..button::Style::default()
        })
        .into()
}

/// A wide chip, like "Refresh".
pub fn wide_button<'a>(look: Look, words: &str, on_press: Message) -> Element<'a, Message> {
    let p = look.p;
    button(container(label(look, words, 11.0, p.fg)).center_x(Length::Fill))
        .on_press(on_press)
        .width(Length::Fill)
        .padding([11, 12])
        .style(move |_, status| button::Style {
            background: Some(if hovered(status) { p.mute } else { p.s3 }.into()),
            text_color: p.fg,
            border: Border::default().rounded(ROUND),
            ..button::Style::default()
        })
        .into()
}

/// A filled circle, like the "connected" light or the live dot.
pub fn dot<'a>(color: Color, size: f32) -> Element<'a, Message> {
    container(space())
        .width(size)
        .height(size)
        .style(move |_| surface(color, size / 2.0))
        .into()
}

/// A small caps tag with a live dot, like "demo".
pub fn tag<'a>(look: Look, words: &str) -> Element<'a, Message> {
    let p = look.p;
    container(
        row![label(look, words, 10.0, p.fg), dot(p.live, 6.0)]
            .spacing(6)
            .align_y(Alignment::Center),
    )
    .padding([4, 10])
    .style(move |_| surface(p.s2, ROUND))
    .into()
}

/// A value in a small dark pill, like a bass level or a firmware version.
pub fn value<'a>(look: Look, words: &str) -> Element<'a, Message> {
    let p = look.p;
    container(label(look, words, 11.0, p.fg))
        .padding([7, 12])
        .style(move |_| surface(p.s2, ROUND))
        .into()
}
