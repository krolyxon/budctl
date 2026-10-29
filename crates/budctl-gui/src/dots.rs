//! LED dots, like the Glyph lights: battery rings, and the sliders for the bass level and the EQ
//! bands. Drawn on a canvas, so they take any theme color, and kept between frames ([`Kept`]).

use std::f32::consts::{FRAC_PI_2, TAU};

use iced::alignment::Vertical;
use iced::widget::canvas::{self, Event, Frame, Path};
use iced::widget::text;
use iced::{Color, Element, Font, Length, Point, Rectangle, Renderer, Theme, mouse};

use crate::Message;
use crate::kept::Kept;
use crate::theme::Look;

/// A battery ring, `size` pixels across: a ring of dots, lit clockwise from the top up to the
/// `level` (a percentage), with the level in LED dots in the middle; "--" and no lit dot when it
/// is unknown. The figure is drawn with the dots, so the page needs no layer of its own for it.
pub fn ring<'a>(look: Look, level: Option<u8>, size: f32) -> Element<'a, Message> {
    let p = look.p;
    let ring = Ring {
        level,
        on: p.fill,
        off: p.s3,
        figure: if level.is_some() { p.fg } else { p.mute },
        font: look.dot(),
    };
    iced::widget::canvas(ring).width(size).height(size).into()
}

#[derive(Clone, Copy, PartialEq)]
struct Ring {
    level: Option<u8>,
    on: Color,
    off: Color,
    figure: Color,
    font: Font,
}

impl canvas::Program<Message> for Ring {
    type State = Kept<Ring>;

    fn draw(
        &self,
        kept: &Kept<Ring>,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        const N: u16 = 44;
        let geometry = kept.draw(renderer, bounds.size(), *self, |frame| {
            let center = frame.center();
            let dot = (bounds.width * 0.045).max(3.0);
            let radius = bounds.width / 2.0 - dot / 2.0 - 1.0;
            let lit = self.level.map_or(0, |level| lit_count(f32::from(level) / 100.0, N));
            for n in 0..N {
                let angle = f32::from(n) / f32::from(N) * TAU - FRAC_PI_2;
                let at = Point::new(center.x + radius * angle.cos(), center.y + radius * angle.sin());
                let color = if n < lit { self.on } else { self.off };
                frame.fill(&Path::circle(at, dot / 2.0), color);
            }
            frame.fill_text(canvas::Text {
                content: self.level.map_or_else(|| "--".to_owned(), |level| level.to_string()),
                position: center,
                color: self.figure,
                size: FIGURE.into(),
                font: self.font,
                align_x: text::Alignment::Center,
                align_y: Vertical::Center,
                ..canvas::Text::default()
            });
        });
        vec![geometry]
    }
}

/// The size of a battery ring's figure. Doto is never set under 26 px.
const FIGURE: f32 = 44.0;

/// How many of `n` dots the fraction `f` lights.
fn lit_count(f: f32, n: u16) -> u16 {
    let f = f.clamp(0.0, 1.0);
    // At most n, so the cast keeps the value.
    #[allow(clippy::cast_possible_truncation)]
    let lit = (f * f32::from(n)).round() as u16;
    lit.min(n)
}

/// A dotted slider over `low..=high`, one dot per step: the dots up to the value are lit, the
/// value's own dot is larger. With `low < 0`, as for EQ gains, the lit dots run from 0.
///
/// A press or a drag sends `change(value, false)`, letting go sends `release`, and the wheel
/// steps by one with `change(value, true)`.
pub struct Slider<F> {
    pub low: i16,
    pub high: i16,
    pub value: i16,
    pub vertical: bool,
    pub change: F,
    pub release: Message,
    pub on: Color,
    pub off: Color,
}

impl<F: Fn(i16, bool) -> Message + 'static> Slider<F> {
    /// The slider, `length` pixels along its axis.
    pub fn view<'a>(self, length: f32) -> Element<'a, Message> {
        let vertical = self.vertical;
        let canvas = iced::widget::canvas(self);
        if vertical {
            canvas.width(28).height(length).into()
        } else {
            canvas.width(length).height(Length::Fixed(24.0)).into()
        }
    }
}

impl<F> Slider<F> {
    fn steps(&self) -> u16 {
        u16::try_from(self.high.saturating_sub(self.low)).unwrap_or(0).max(1)
    }

    /// Where the dot for `value` sits along the slider, from 0 to 1.
    fn place(&self, value: i16) -> f32 {
        f32::from(value.saturating_sub(self.low)) / f32::from(self.steps())
    }

    /// The value under the cursor at `along` (0 to 1 along the slider).
    fn value_at(&self, along: f32) -> i16 {
        let along = along.clamp(0.0, 1.0);
        // Rounded and within 0..=steps, so it fits.
        #[allow(clippy::cast_possible_truncation)]
        let step = (along * f32::from(self.steps())).round() as u16;
        self.low.saturating_add(i16::try_from(step).unwrap_or(0)).min(self.high)
    }

    /// 0 to 1 along the slider for a point inside `bounds`: left to right, or bottom to top.
    fn along(&self, bounds: Rectangle, at: Point) -> f32 {
        let pad = DOT_BIG;
        if self.vertical {
            (bounds.y + bounds.height - pad - at.y) / (bounds.height - 2.0 * pad).max(1.0)
        } else {
            (at.x - bounds.x - pad) / (bounds.width - 2.0 * pad).max(1.0)
        }
    }
}

/// The value's dot, and the others.
const DOT_BIG: f32 = 11.0;
const DOT: f32 = 5.0;

/// What a slider's drawing shows: its range, its value, which way it runs, and its colors.
type Shows = (i16, i16, i16, bool, Color, Color);

#[derive(Default)]
pub struct Drag {
    held: bool,
    kept: Kept<Shows>,
}

impl<F: Fn(i16, bool) -> Message> canvas::Program<Message> for Slider<F> {
    type State = Drag;

    fn update(
        &self,
        state: &mut Drag,
        event: &Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        let Event::Mouse(event) = event else {
            return None;
        };
        let change = |at: Point| {
            let value = self.value_at(self.along(bounds, at));
            canvas::Action::publish((self.change)(value, false)).and_capture()
        };
        match event {
            mouse::Event::ButtonPressed(mouse::Button::Left) => {
                let at = cursor.position_over(bounds)?;
                state.held = true;
                Some(change(at))
            }
            mouse::Event::CursorMoved { .. } if state.held => Some(change(cursor.position()?)),
            mouse::Event::ButtonReleased(mouse::Button::Left) if state.held => {
                state.held = false;
                Some(canvas::Action::publish(self.release.clone()).and_capture())
            }
            mouse::Event::WheelScrolled { delta } => {
                cursor.position_over(bounds)?;
                let y = match delta {
                    mouse::ScrollDelta::Lines { y, .. } | mouse::ScrollDelta::Pixels { y, .. } => *y,
                };
                let to = if y > 0.0 {
                    self.value.saturating_add(1).min(self.high)
                } else if y < 0.0 {
                    self.value.saturating_sub(1).max(self.low)
                } else {
                    return None;
                };
                (to != self.value).then(|| canvas::Action::publish((self.change)(to, true)).and_capture())
            }
            _ => None,
        }
    }

    fn draw(
        &self,
        state: &Drag,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let shows = (self.low, self.high, self.value, self.vertical, self.on, self.off);
        vec![
            state
                .kept
                .draw(renderer, bounds.size(), shows, |frame| self.dots(frame)),
        ]
    }

    fn mouse_interaction(&self, state: &Drag, bounds: Rectangle, cursor: mouse::Cursor) -> mouse::Interaction {
        if state.held || cursor.is_over(bounds) {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::default()
        }
    }
}

impl<F> Slider<F> {
    /// One dot per step; the value's dot is larger.
    fn dots(&self, frame: &mut Frame) {
        let size = frame.size();
        let at = |along: f32| {
            if self.vertical {
                Point::new(
                    size.width / 2.0,
                    size.height - DOT_BIG - along * (size.height - 2.0 * DOT_BIG),
                )
            } else {
                Point::new(DOT_BIG + along * (size.width - 2.0 * DOT_BIG), size.height / 2.0)
            }
        };
        let base = self.low.max(0).min(self.high);
        let (from, to) = (base.min(self.value), base.max(self.value));
        let mut value = self.low;
        while value <= self.high {
            let lit = (from..=to).contains(&value);
            let (diameter, color) = if value == self.value {
                (DOT_BIG, self.on)
            } else if lit {
                (DOT, self.on)
            } else {
                (DOT, self.off)
            };
            frame.fill(&Path::circle(at(self.place(value)), diameter / 2.0), color);
            value = match value.checked_add(1) {
                Some(next) => next,
                None => break,
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slider(low: i16, high: i16) -> Slider<fn(i16, bool) -> Message> {
        Slider {
            low,
            high,
            value: 0,
            vertical: false,
            change: |_, _| Message::SaveBass,
            release: Message::SaveBass,
            on: Color::WHITE,
            off: Color::BLACK,
        }
    }

    #[test]
    fn rings_light_their_share() {
        assert_eq!(lit_count(0.8, 44), 35);
        assert_eq!(lit_count(1.5, 44), 44);
        assert_eq!(lit_count(-1.0, 44), 0);
    }

    #[test]
    fn sliders_find_the_value_under_the_cursor() {
        let eq = slider(-6, 6);
        assert_eq!(eq.value_at(0.0), -6);
        assert_eq!(eq.value_at(0.5), 0);
        assert_eq!(eq.value_at(1.0), 6);
        assert_eq!(eq.value_at(2.0), 6);
        assert_eq!(eq.place(0), 0.5);
        // A slider with one value does not divide by zero.
        assert_eq!(slider(3, 3).value_at(0.7), 3);
    }
}
