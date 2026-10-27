//! The building blocks every page uses: cards, setting rows, switches, the segmented pill and
//! the sidebar's pieces. Everything takes its colors from the [`Look`].

use iced::widget::{Column, Row, button, column, container, row, rule, space, text, toggler};
use iced::{Alignment, Border, Element, Length};

use crate::Message;
use crate::icons::{self, Icon};
use crate::theme::Look;

/// A page title.
pub fn title<'a>(look: Look, title: &'a str) -> Element<'a, Message> {
    text(title).size(30).font(look.bold()).color(look.p.fg).into()
}

/// A rounded card holding rows, with a thin line between them.
pub fn card<'a>(look: Look, rows: Vec<Element<'a, Message>>) -> Element<'a, Message> {
    let p = look.p;
    let mut column = Column::new();
    for (n, item) in rows.into_iter().enumerate() {
        if n > 0 {
            column = column.push(separator(look));
        }
        column = column.push(item);
    }
    container(column)
        .padding(5)
        .width(Length::Fill)
        .style(move |_| container::Style {
            background: Some(p.surface.into()),
            border: Border {
                color: p.line,
                width: 1.0,
                radius: 18.0.into(),
            },
            ..container::Style::default()
        })
        .into()
}

/// A rounded panel like a card, around content of its own.
pub fn panel<'a>(look: Look, content: Element<'a, Message>) -> Element<'a, Message> {
    let p = look.p;
    container(content)
        .padding(24)
        .width(Length::Fill)
        .style(move |_| container::Style {
            background: Some(p.surface.into()),
            border: Border {
                color: p.line,
                width: 1.0,
                radius: 18.0.into(),
            },
            ..container::Style::default()
        })
        .into()
}

fn separator<'a>(look: Look) -> Element<'a, Message> {
    let p = look.p;
    container(rule::horizontal(1).style(move |_| rule::Style {
        color: p.line,
        radius: 0.0.into(),
        fill_mode: rule::FillMode::Full,
        snap: true,
    }))
    .padding([0, 14])
    .into()
}

/// A setting row: icon, title, an optional subtitle, and something on the right. The whole row
/// is the click target. `focused`: the keyboard cursor is on it.
pub fn setting<'a>(
    look: Look,
    icon: Icon,
    title: &'a str,
    subtitle: Option<String>,
    trailing: Element<'a, Message>,
    on_press: Message,
    focused: bool,
) -> Element<'a, Message> {
    let p = look.p;
    let words = column![text(title).size(15).font(look.medium()).color(p.fg)]
        .push(subtitle.map(|s| text(s).size(13).color(p.dim)))
        .spacing(2)
        .width(Length::Fill);
    let content = row![icons::icon(icon, 22.0, p.icon), words, trailing]
        .spacing(16)
        .align_y(Alignment::Center)
        .padding([13, 16]);
    button(content)
        .on_press(on_press)
        .padding(0)
        .width(Length::Fill)
        .style(move |_, status| button::Style {
            background: (focused || hovered(status)).then_some(p.surface_high.into()),
            text_color: p.fg,
            border: cursor(p, focused, 14.0),
            ..button::Style::default()
        })
        .into()
}

/// The keyboard cursor: a 2 px accent frame. No frame without it.
fn cursor(p: crate::theme::Palette, focused: bool, radius: f32) -> Border {
    Border {
        color: p.accent,
        width: if focused { 2.0 } else { 0.0 },
        radius: radius.into(),
    }
}

fn hovered(status: button::Status) -> bool {
    matches!(status, button::Status::Hovered | button::Status::Pressed)
}

/// An on/off switch. It only shows the state: the row around it takes the click.
pub fn switch<'a>(look: Look, on: bool) -> Element<'a, Message> {
    let p = look.p;
    toggler(on)
        .size(24)
        .style(move |_, _| toggler::Style {
            background: (if on { p.accent } else { p.surface_high }).into(),
            background_border_width: 1.0,
            background_border_color: if on { p.accent } else { p.line },
            foreground: (if on { p.accent_fg } else { p.dim }).into(),
            foreground_border_width: 0.0,
            foreground_border_color: p.accent_fg,
            text_color: None,
            border_radius: None,
            padding_ratio: 0.12,
        })
        .into()
}

/// A chevron: the row opens a page.
pub fn chevron<'a>(look: Look) -> Element<'a, Message> {
    icons::icon(Icon::Chevron, 18.0, look.p.dim)
}

/// One choice of a [`segmented`] control.
pub struct Segment<'a> {
    pub icon: Option<Icon>,
    pub label: &'a str,
    pub selected: bool,
    pub on_press: Message,
}

/// A pill of choices with the selected one filled, like the noise-control modes. `large` puts an
/// icon above each label. `focused`: the keyboard cursor is on it.
pub fn segmented<'a>(look: Look, segments: Vec<Segment<'a>>, large: bool, focused: bool) -> Element<'a, Message> {
    let p = look.p;
    let mut choices = Row::new().spacing(4);
    for segment in segments {
        let selected = segment.selected;
        let color = if selected { p.accent_fg } else { p.dim };
        let label = text(segment.label)
            .size(if large { 13.0 } else { 13.5 })
            .font(look.medium())
            .color(color);
        let inner: Element<'a, Message> = match segment.icon {
            Some(icon) if large => column![icons::icon(icon, 22.0, color), label]
                .spacing(6)
                .align_x(Alignment::Center)
                .into(),
            _ => label.into(),
        };
        let choice = button(container(inner).center_x(Length::Fill))
            .on_press(segment.on_press)
            .width(Length::Fill)
            .padding(if large { [11, 8] } else { [7, 12] })
            .style(move |_, status| button::Style {
                background: if selected {
                    Some(p.accent.into())
                } else {
                    hovered(status).then_some(p.surface_high.into())
                },
                text_color: color,
                border: Border::default().rounded(999),
                ..button::Style::default()
            });
        choices = choices.push(choice);
    }
    container(choices)
        .padding(5)
        .width(Length::Fill)
        .style(move |_| container::Style {
            background: Some(p.surface.into()),
            border: if focused {
                cursor(p, true, 999.0)
            } else {
                Border {
                    color: p.line,
                    width: 1.0,
                    radius: 999.0.into(),
                }
            },
            ..container::Style::default()
        })
        .into()
}

/// A page in the sidebar.
pub fn nav_item<'a>(look: Look, icon: Icon, label: &'a str, open: bool, on_press: Message) -> Element<'a, Message> {
    let p = look.p;
    let (color, icon_color) = if open { (p.fg, p.icon) } else { (p.dim, p.dim) };
    let content = row![
        icons::icon(icon, 19.0, icon_color),
        text(label)
            .size(14.5)
            .font(if open { look.medium() } else { look.font })
            .color(color)
    ]
    .spacing(13)
    .align_y(Alignment::Center);
    button(content)
        .on_press(on_press)
        .width(Length::Fill)
        .padding([10, 14])
        .style(move |_, status| button::Style {
            background: if open {
                Some(p.accent_dim.into())
            } else {
                hovered(status).then_some(p.surface_high.into())
            },
            text_color: color,
            border: Border::default().rounded(12),
            ..button::Style::default()
        })
        .into()
}

/// A wide button with an outline, like "Disconnect".
pub fn outline_button<'a>(look: Look, label: &'a str, on_press: Message) -> Element<'a, Message> {
    let p = look.p;
    button(container(text(label).size(13.5).font(look.medium()).color(p.fg)).center_x(Length::Fill))
        .on_press(on_press)
        .width(Length::Fill)
        .padding([8, 12])
        .style(move |_, status| button::Style {
            background: hovered(status).then_some(p.surface_high.into()),
            text_color: p.fg,
            border: Border {
                color: p.line,
                width: 1.0,
                radius: 999.0.into(),
            },
            ..button::Style::default()
        })
        .into()
}

/// A small filled circle, like the "connected" light.
pub fn light<'a>(color: iced::Color) -> Element<'a, Message> {
    container(space())
        .width(8)
        .height(8)
        .style(move |_| container::Style {
            background: Some(color.into()),
            border: Border::default().rounded(4),
            ..container::Style::default()
        })
        .into()
}

/// A small outlined tag, like "demo".
pub fn tag<'a>(look: Look, label: &'a str) -> Element<'a, Message> {
    let p = look.p;
    container(text(label).size(11).font(look.medium()).color(p.icon))
        .padding([1, 8])
        .style(move |_| container::Style {
            border: Border {
                color: p.icon,
                width: 1.0,
                radius: 999.0.into(),
            },
            ..container::Style::default()
        })
        .into()
}

/// A thin vertical line, between the sidebar and the page.
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
