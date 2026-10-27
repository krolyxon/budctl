//! The controls of the owner's shell (Aurora), rebuilt in iced. Aurora is flat and dense: square
//! corners, 1 px lines, accent fills with background-colored text, uppercase accent section
//! labels, lowercase words.

use iced::alignment::{Horizontal, Vertical};
use iced::widget::{Button, Column, Row, button, column, container, progress_bar, row, rule, scrollable, space, text};
use iced::{Alignment, Border, Color, Element, Length, Padding};

use crate::Message;
use crate::glyphs;
use crate::theme::Look;

/// The page title: uppercase and bold.
pub fn title<'a>(look: Look, title: &str) -> Element<'a, Message> {
    text(title.to_uppercase())
        .size(look.px(4.0))
        .font(look.bold())
        .color(look.p.fg)
        .into()
}

/// An uppercase accent label, with a line filling the rest of the row.
pub fn section<'a>(look: Look, label: &str) -> Element<'a, Message> {
    let p = look.p;
    let line = rule::horizontal(1).style(move |_| rule::Style {
        color: p.line,
        radius: 0.0.into(),
        fill_mode: rule::FillMode::Full,
        snap: true,
    });
    row![text(label.to_uppercase()).size(look.px(-2.0)).color(p.accent), line]
        .spacing(8)
        .align_y(Alignment::Center)
        .into()
}

/// Text in the base size and color.
pub fn label<'a>(look: Look, words: impl text::IntoFragment<'a>) -> text::Text<'a> {
    text(words).size(look.size).color(look.p.fg)
}

/// Smaller, dimmer text, for hints.
pub fn hint<'a>(look: Look, words: impl text::IntoFragment<'a>) -> text::Text<'a> {
    text(words).size(look.px(-2.0)).color(look.p.dim)
}

fn hovered(status: button::Status) -> bool {
    matches!(status, button::Status::Hovered | button::Status::Pressed)
}

/// An on/off setting: its label, an optional hint below, and a square switch on the right. The
/// whole row is the click target.
pub fn switch_row<'a>(
    look: Look,
    name: String,
    about: Option<&'a str>,
    on: bool,
    toggle: Message,
) -> Element<'a, Message> {
    let p = look.p;
    let words = column![label(look, name)]
        .push(about.map(|about| hint(look, about)))
        .width(Length::Fill);
    let content = row![words, switch(look, on)]
        .spacing(10)
        .align_y(Alignment::Center)
        .padding([5, 0]);
    button(content)
        .on_press(toggle)
        .width(Length::Fill)
        .padding(0)
        .style(move |_, status| button::Style {
            background: hovered(status).then_some(p.surface.into()),
            text_color: p.fg,
            ..button::Style::default()
        })
        .into()
}

/// A square track with a square knob: left and dim when off, right on the accent when on.
fn switch<'a>(look: Look, on: bool) -> Element<'a, Message> {
    let p = look.p;
    let knob = container(space())
        .width(12)
        .height(12)
        .style(move |_| container::Style {
            background: Some((if on { p.bg } else { p.dim }).into()),
            ..container::Style::default()
        });
    container(knob)
        .width(36)
        .height(18)
        .padding(3)
        .align_x(if on { Horizontal::Right } else { Horizontal::Left })
        .align_y(Vertical::Center)
        .style(move |_| container::Style {
            background: Some((if on { p.accent } else { p.surface_high }).into()),
            border: Border {
                color: if on { p.accent } else { p.line },
                width: 1.0,
                radius: 0.0.into(),
            },
            ..container::Style::default()
        })
        .into()
}

/// A button with a 1 px border, an optional glyph and a label. `active` fills it with the accent.
pub fn button_<'a>(
    look: Look,
    glyph: Option<char>,
    words: &'a str,
    active: bool,
    on_press: Message,
) -> Button<'a, Message> {
    let p = look.p;
    let color = if active { p.bg } else { p.fg };
    let content = Row::new()
        .push(glyph.map(|g| text(g.to_string()).size(15).color(color)))
        .push((!words.is_empty()).then(|| text(words).size(look.px(-1.0)).color(color)))
        .spacing(8)
        .align_y(Alignment::Center);
    button(container(content).center(Length::Fill))
        .on_press(on_press)
        .height(26)
        .padding(0)
        .style(move |_, status| {
            let hover = hovered(status);
            button::Style {
                background: if active {
                    Some(p.accent.into())
                } else {
                    hover.then_some(p.surface_high.into())
                },
                text_color: color,
                border: Border {
                    color: if active || hover { p.accent } else { p.line },
                    width: 1.0,
                    radius: 0.0.into(),
                },
                ..button::Style::default()
            }
        })
}

/// One option of a [`choice`].
pub struct Opt<'a> {
    pub glyph: Option<char>,
    pub words: &'a str,
    pub chosen: bool,
    pub on_press: Message,
}

/// A label, then one button per option, all the same width, the chosen one filled.
pub fn choice<'a>(look: Look, name: &'a str, options: Vec<Opt<'a>>) -> Element<'a, Message> {
    let buttons = options.into_iter().fold(Row::new().spacing(2), |buttons, o| {
        buttons.push(button_(look, o.glyph, o.words, o.chosen, o.on_press).width(Length::FillPortion(1)))
    });
    row![label(look, name).width(150), buttons]
        .spacing(8)
        .align_y(Alignment::Center)
        .into()
}

/// A label, a level bar and the value, like a battery's charge.
pub fn stat_bar<'a>(look: Look, name: &'a str, fraction: f32, value: String, fill: Color) -> Element<'a, Message> {
    let p = look.p;
    let bar = progress_bar(0.0..=1.0, fraction)
        .girth(6)
        .style(move |_| progress_bar::Style {
            background: p.surface_high.into(),
            bar: fill.into(),
            border: Border::default(),
        });
    row![
        text(name).size(look.px(-1.0)).color(p.dim).width(72),
        bar,
        text(value)
            .size(look.px(-1.0))
            .color(p.fg)
            .width(96)
            .align_x(Horizontal::Right)
    ]
    .spacing(10)
    .align_y(Alignment::Center)
    .into()
}

/// A row that opens a page: its label, an optional hint below, and on the right a value and an
/// arrow. Lined up with the switch rows.
pub fn link_row<'a>(
    look: Look,
    name: &'a str,
    about: Option<&'a str>,
    value: Option<String>,
    on_press: Message,
) -> Element<'a, Message> {
    let p = look.p;
    let words = column![label(look, name)]
        .push(about.map(|about| hint(look, about)))
        .width(Length::Fill);
    let content = row![words]
        .push(value.map(|value| text(value).size(look.px(-1.0)).color(p.dim)))
        .push(text(glyphs::CHEVRON.to_string()).size(look.px(4.0)).color(p.dim))
        .spacing(10)
        .align_y(Alignment::Center)
        .padding([5, 0]);
    button(content)
        .on_press(on_press)
        .width(Length::Fill)
        .padding(0)
        .style(move |_, status| button::Style {
            background: hovered(status).then_some(p.surface.into()),
            text_color: p.fg,
            ..button::Style::default()
        })
        .into()
}

/// The sidebar's title: a glyph and a name, bold, on the accent.
pub fn sidebar_title<'a>(look: Look, glyph: char, name: &str) -> Element<'a, Message> {
    text(format!("{glyph}  {name}"))
        .size(look.px(1.0))
        .font(look.bold())
        .color(look.p.accent)
        .into()
}

/// A group heading in the sidebar.
pub fn group<'a>(look: Look, name: &str) -> Element<'a, Message> {
    text(name.to_uppercase()).size(look.px(-3.0)).color(look.p.dim).into()
}

/// A page in the sidebar. The open one is filled with the accent.
pub fn nav_item<'a>(look: Look, glyph: char, name: &str, open: bool, on_press: Message) -> Element<'a, Message> {
    let p = look.p;
    let words = text(format!("{glyph}   {name}"))
        .size(look.size)
        .font(if open { look.bold() } else { look.font })
        .color(if open { p.bg } else { p.fg });
    button(container(words).height(Length::Fill).align_y(Vertical::Center))
        .on_press(on_press)
        .width(Length::Fill)
        .height(30)
        .padding([0, 10])
        .style(move |_, status| button::Style {
            background: if open {
                Some(p.accent.into())
            } else {
                hovered(status).then_some(p.surface_high.into())
            },
            text_color: if open { p.bg } else { p.fg },
            ..button::Style::default()
        })
        .into()
}

/// The buds in the sidebar: a glyph, the name and a status line, in a 1 px frame that is the
/// accent while they are connected.
pub fn device<'a>(look: Look, name: &'a str, status: String, connected: bool) -> Element<'a, Message> {
    let p = look.p;
    let mark = if connected { p.accent } else { p.dim };
    let content = row![
        text(glyphs::HEADPHONES.to_string()).size(17).color(mark),
        column![
            text(name).size(look.size).font(look.bold()).color(p.fg),
            text(status).size(look.px(-3.0)).color(mark)
        ]
    ]
    .spacing(10)
    .align_y(Alignment::Center);
    container(content)
        .padding([7, 10])
        .width(Length::Fill)
        .style(move |_| container::Style {
            border: Border {
                color: if connected { p.accent } else { p.line },
                width: 1.0,
                radius: 0.0.into(),
            },
            ..container::Style::default()
        })
        .into()
}

/// A column of sections with Aurora's spacing: 8 px between rows, a little more above each section.
pub fn sections<'a>(sections: Vec<Vec<Element<'a, Message>>>) -> Element<'a, Message> {
    let mut page = Column::new().spacing(14);
    for rows in sections {
        page = page.push(Column::with_children(rows).spacing(8));
    }
    page.into()
}

/// A scrolling area with a thin, square scrollbar, clear of the content.
pub fn scroll<'a>(look: Look, content: Element<'a, Message>) -> Element<'a, Message> {
    let p = look.p;
    let padded = container(content).padding(Padding {
        top: 0.0,
        right: 14.0,
        bottom: 8.0,
        left: 0.0,
    });
    scrollable(padded)
        .height(Length::Fill)
        .style(move |theme, status| {
            let mut style = scrollable::default(theme, status);
            for rail in [&mut style.vertical_rail, &mut style.horizontal_rail] {
                rail.background = None;
                rail.border = Border::default();
                rail.scroller.background = p.surface_high.into();
                rail.scroller.border = Border::default();
            }
            style
        })
        .into()
}

/// A 1 px vertical line, between the sidebar and the page.
pub fn divider<'a>(look: Look) -> Element<'a, Message> {
    let p = look.p;
    rule::vertical(1)
        .style(move |_| rule::Style {
            color: p.line,
            radius: 0.0.into(),
            fill_mode: rule::FillMode::Full,
            snap: true,
        })
        .into()
}
