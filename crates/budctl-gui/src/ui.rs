//! The building blocks every page uses, in Nothing OS's shapes: caps labels, dot-matrix words,
//! round cards, rows with their icon in a circle, pill switches, pill tracks and chips.
//! Everything takes its colors from the [`Look`].
//!
//! What can be pressed sits on a [`motion::eased`] surface, so its hover and its changes of
//! state fade rather than snap.

use iced::widget::{Column, Row, Stack, button, column, container, row, space, text};
use iced::{Alignment, Border, Color, Element, Length, Theme};

use crate::Message;
use crate::dots;
use crate::icons::{self, Icon};
use crate::motion::{self, Paint};
use crate::theme::Look;

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
    layer(
        container(Column::with_children(rows).spacing(2))
            .padding(6)
            .width(Length::Fill)
            .style(move |_| surface(p.s1, CARD)),
    )
}

/// A round card around content of its own.
pub fn panel<'a>(look: Look, content: Element<'a, Message>) -> Element<'a, Message> {
    let p = look.p;
    layer(
        container(content)
            .padding(24)
            .width(Length::Fill)
            .style(move |_| surface(p.s1, CARD)),
    )
}

/// `content` on a layer of its own, clipped to its bounds.
///
/// The CPU renderer repaints a page that scrolls patch by patch: one for each word, icon and
/// row that moved, with the whole card under each patch painted again. A layer that moved is
/// one patch, so a card is painted about once per frame instead of dozens of times.
fn layer<'a>(content: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    // The stack takes its size from `content` and draws it on a layer of its own, above the
    // empty space under it.
    Stack::new().push(content).push_under(space()).clip(true).into()
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
            // As clear as the background is: a dialog fading in brings its dimming with it.
            background: Some(Color { a: 0.74 * bg.a, ..bg }.into()),
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
    let content = row![
        disc(look, icons::icon(icon, 20.0, glyph), 44.0, circle),
        words,
        trailing
    ]
    .spacing(14)
    .align_y(Alignment::Center)
    .padding([6, 14]);
    let content = container(content).padding([0, 6]);
    let interactive = on_press.is_some();
    let rest = Paint::fill(if focused { p.s3 } else { Color::TRANSPARENT }).framed(p.fill, focused);
    let hover = Paint { fill: p.s3, ..rest };
    let row = button(content)
        .on_press_maybe(on_press)
        .padding(0)
        .width(Length::Fill)
        .style(bare(p.fg));
    motion::eased(row, rest, hover, CARD, interactive, look.motion)
}

/// `content` centered in a circle `size` across.
fn disc<'a>(look: Look, content: Element<'a, Message>, size: f32, color: Color) -> Element<'a, Message> {
    motion::surface(container(content).center(size), Paint::fill(color), ROUND, look.motion)
}

/// A button with no paint of its own: the [`motion::eased`] surface under it paints it.
fn bare(text_color: Color) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, _| button::Style {
        background: None,
        text_color,
        border: Border::default(),
        ..button::Style::default()
    }
}

/// An on/off switch: off, a dark pill with a dim knob on the left; on, the fill runs to the
/// right end, with a dot in it. It only shows the state: the row around it takes the click.
pub fn switch<'a>(look: Look, on: bool) -> Element<'a, Message> {
    dots::switch(look, on)
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
            .style(bare(color));
        // The chosen cell's fill fades out as the next one's fades in.
        let rest = Paint::fill(if selected { p.fill } else { Color::TRANSPARENT });
        let hover = Paint::fill(if selected { p.fill } else { p.s3 });
        choices = choices.push(motion::eased(choice, rest, hover, ROUND, true, look.motion));
    }
    let track = Paint::fill(p.s2).framed(p.fill, focused);
    motion::surface(
        container(choices).padding(6).width(Length::Fill),
        track,
        ROUND,
        look.motion,
    )
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
    let item = button(content)
        .on_press(on_press)
        .width(Length::Fill)
        .padding([6, 10])
        .style(bare(p.fg));
    let hover = if open { Paint::NONE } else { Paint::fill(p.s1) };
    motion::eased(item, Paint::NONE, hover, CARD, true, look.motion)
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
    let pill = button(label(look, words, 11.0, color))
        .on_press(on_press)
        .padding([10, 18])
        .style(bare(color));
    let rest = Paint::fill(if filled { p.fill } else { p.s3 }).framed(p.fill, focused);
    let hover = Paint::fill(if filled { p.fill } else { p.mute }).framed(p.fill, focused);
    motion::eased(pill, rest, hover, ROUND, true, look.motion)
}

/// A round button with just an icon, like the pencil on a preset's row.
pub fn icon_button<'a>(look: Look, icon: Icon, focused: bool, on_press: Message) -> Element<'a, Message> {
    let p = look.p;
    let round = button(container(icons::icon(icon, 18.0, p.fg)).center(40))
        .on_press(on_press)
        .padding(0)
        .style(bare(p.fg));
    let rest = Paint::fill(if focused { p.mute } else { p.s2 }).framed(p.fill, focused);
    let hover = Paint::fill(p.mute).framed(p.fill, focused);
    motion::eased(round, rest, hover, ROUND, true, look.motion)
}

/// A wide chip, like "Refresh".
pub fn wide_button<'a>(look: Look, words: &str, on_press: Message) -> Element<'a, Message> {
    let p = look.p;
    let wide = button(container(label(look, words, 11.0, p.fg)).center_x(Length::Fill))
        .on_press(on_press)
        .width(Length::Fill)
        .padding([11, 12])
        .style(bare(p.fg));
    motion::eased(wide, Paint::fill(p.s3), Paint::fill(p.mute), ROUND, true, look.motion)
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
