//! LED dots, like the Glyph lights: battery rings, switches, the sliders for the bass level and
//! the EQ bands, and the dots that blink or scan while something is under way. Drawn on a canvas,
//! so they take any theme color, and kept between frames ([`Kept`]).
//!
//! Each one moves on its own ([`motion`]): it asks for the frames it needs and nothing more, so
//! the window rests once it has settled.

use std::f32::consts::{FRAC_PI_2, TAU};

use iced::alignment::Vertical;
use iced::time::{Duration, Instant};
use iced::widget::canvas::{self, Event, Frame, Path};
use iced::widget::text;
use iced::{Color, Element, Font, Length, Point, Rectangle, Renderer, Size, Theme, mouse};

use crate::Message;
use crate::kept::Kept;
use crate::motion::{self, Mix, Tween};
use crate::theme::Look;

/// A battery ring, `size` pixels across: a ring of dots, lit clockwise from the top up to the
/// `level` (a percentage), with the level in LED dots in the middle; "--" and no lit dot when it
/// is unknown. The figure is drawn with the dots, so the page needs no layer of its own for it.
///
/// The dots light one after the other, like the Glyph lights: when the ring first shows, and
/// when the level changes. The figure is the level at once.
pub fn ring<'a>(look: Look, level: Option<u8>, size: f32) -> Element<'a, Message> {
    let p = look.p;
    let ring = Ring {
        level,
        on: p.fill,
        off: p.s3,
        figure: if level.is_some() { p.fg } else { p.mute },
        font: look.dot(),
        motion: look.motion,
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
    motion: bool,
}

/// The dots in a ring.
const RING_DOTS: u16 = 44;

impl Ring {
    /// How many dots the level lights.
    fn lit(&self) -> u16 {
        self.level
            .map_or(0, |level| lit_count(f32::from(level) / 100.0, RING_DOTS))
    }
}

/// A ring's dots on their way to the level.
#[derive(Default)]
pub struct Sweep {
    kept: Kept<(u16, Ring)>,
    /// How many dots are lit, as a number that moves; `None` before the first frame.
    lit: Option<Tween<f32>>,
    now: Option<Instant>,
}

impl Sweep {
    /// The dots lit now: `ring`'s own when nothing has moved yet.
    fn lit(&self, ring: &Ring) -> u16 {
        match (self.lit, self.now) {
            (Some(lit), Some(now)) => lit_count(lit.at(now) / f32::from(RING_DOTS), RING_DOTS),
            _ => ring.lit(),
        }
    }
}

impl canvas::Program<Message> for Ring {
    type State = Sweep;

    fn update(
        &self,
        state: &mut Sweep,
        event: &Event,
        _bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        let now = motion::redraw(event)?;
        state.now = Some(now);
        // First seen, the ring starts dark and sweeps up to the level.
        let lit = state.lit.get_or_insert_with(|| Tween::at_rest(0.0));
        lit.aim(f32::from(self.lit()), now, motion::SWEEP, self.motion);
        lit.running(now).then(canvas::Action::request_redraw)
    }

    fn draw(
        &self,
        state: &Sweep,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        const N: u16 = RING_DOTS;
        let lit = state.lit(self);
        let geometry = state.kept.draw(renderer, bounds.size(), (lit, *self), |frame| {
            let center = frame.center();
            let dot = (bounds.width * 0.045).max(3.0);
            let radius = bounds.width / 2.0 - dot / 2.0 - 1.0;
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
    pub motion: bool,
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

/// What a slider's drawing shows: its range, its value, which way it runs, its colors, and the
/// value it is leaving with how far the value's dot has grown.
type Shows = (i16, i16, i16, bool, Color, Color, Option<i16>, f32);

#[derive(Default)]
pub struct Drag {
    held: bool,
    kept: Kept<Shows>,
    /// The value last drawn.
    value: Option<i16>,
    /// The value before it, whose large dot shrinks as the new one's grows.
    left: Option<i16>,
    grow: Option<Tween<f32>>,
    now: Option<Instant>,
}

impl Drag {
    /// How far the value's dot has grown, 0 to 1.
    fn grown(&self) -> f32 {
        match (self.grow, self.now) {
            (Some(grow), Some(now)) => grow.at(now),
            _ => 1.0,
        }
    }
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
        if let Some(now) = motion::redraw(event) {
            return self.follow(state, now);
        }
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
        let grown = state.grown();
        let left = state.left.filter(|_| grown < 1.0);
        let shows = (
            self.low,
            self.high,
            self.value,
            self.vertical,
            self.on,
            self.off,
            left,
            grown,
        );
        vec![
            state
                .kept
                .draw(renderer, bounds.size(), shows, |frame| self.dots(frame, left, grown)),
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
    /// At a redraw: when the value has changed, its dot starts growing and the last one's
    /// shrinking. No travel: the large dot fades from one place to the other.
    fn follow(&self, state: &mut Drag, now: Instant) -> Option<canvas::Action<Message>> {
        state.now = Some(now);
        if let Some(last) = state.value
            && last != self.value
        {
            state.left = Some(last);
            let mut grow = Tween::at_rest(0.0);
            grow.aim(1.0, now, motion::HOVER, self.motion);
            state.grow = Some(grow);
        }
        state.value = Some(self.value);
        state
            .grow
            .is_some_and(|grow| grow.running(now))
            .then(canvas::Action::request_redraw)
    }

    /// One dot per step; the value's dot is larger, grown `grown` of the way (0 to 1), and the
    /// dot of the value `left` is shrinking back.
    fn dots(&self, frame: &mut Frame, left: Option<i16>, grown: f32) {
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
                (DOT.mix(DOT_BIG, grown), self.on)
            } else if left == Some(value) {
                (DOT_BIG.mix(DOT, grown), if lit { self.on } else { self.off })
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

/// An on/off switch, 64 by 34: off, a dark pill with a dim knob on the left; on, the fill runs
/// to the right end, with a dot in it. Turned, the knob stretches into the fill (or back) as its
/// color changes and the dot fades.
pub fn switch<'a>(look: Look, on: bool) -> Element<'a, Message> {
    let p = look.p;
    let switch = Switch {
        on,
        track: p.s2,
        off: p.mute,
        fill: p.fill,
        dot: p.fill_fg,
        motion: look.motion,
    };
    iced::widget::canvas(switch).width(64).height(34).into()
}

#[derive(Clone, Copy, PartialEq)]
struct Switch {
    on: bool,
    track: Color,
    off: Color,
    fill: Color,
    dot: Color,
    motion: bool,
}

impl Switch {
    fn target(&self) -> f32 {
        if self.on { 1.0 } else { 0.0 }
    }
}

/// How far a switch is turned on: 0 off, 1 on.
#[derive(Default)]
pub struct Turn {
    kept: Kept<(f32, Switch)>,
    on: Option<Tween<f32>>,
    now: Option<Instant>,
}

impl canvas::Program<Message> for Switch {
    type State = Turn;

    fn update(
        &self,
        state: &mut Turn,
        event: &Event,
        _bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        let now = motion::redraw(event)?;
        state.now = Some(now);
        // First seen, it is as it is; it moves only when turned.
        let on = state.on.get_or_insert_with(|| Tween::at_rest(self.target()));
        on.aim(self.target(), now, motion::STATE, self.motion);
        on.running(now).then(canvas::Action::request_redraw)
    }

    fn draw(
        &self,
        state: &Turn,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let t = match (state.on, state.now) {
            (Some(on), Some(now)) => on.at(now),
            _ => self.target(),
        };
        let geometry = state.kept.draw(renderer, bounds.size(), (t, *self), |frame| {
            let size = frame.size();
            let track = Path::rounded_rectangle(Point::ORIGIN, size, (size.height / 2.0).into());
            frame.fill(&track, self.track);
            // The knob, 26 across, stretching to the track's inner width.
            const PAD: f32 = 4.0;
            let height = size.height - 2.0 * PAD;
            let width = height.mix(size.width - 2.0 * PAD, t);
            let knob = Path::rounded_rectangle(Point::new(PAD, PAD), Size::new(width, height), (height / 2.0).into());
            frame.fill(&knob, self.off.mix(self.fill, t));
            if t > 0.0 {
                let center = Point::new(PAD + width - 8.0 - 5.0, size.height / 2.0);
                let dot = Color {
                    a: self.dot.a * t,
                    ..self.dot
                };
                frame.fill(&Path::circle(center, 5.0), dot);
            }
        });
        vec![geometry]
    }
}

/// A dot that blinks hard, lit and dark in turn, like a recording light: something is under way.
/// Without motion, it stays lit.
pub fn blink<'a>(look: Look, color: Color, size: f32) -> Element<'a, Message> {
    let blink = Blink {
        color,
        motion: look.motion,
    };
    iced::widget::canvas(blink).width(size).height(size).into()
}

#[derive(Clone, Copy, PartialEq)]
struct Blink {
    color: Color,
    motion: bool,
}

#[derive(Default)]
pub struct Blinking {
    kept: Kept<(bool, Blink)>,
    since: Option<Instant>,
    dark: bool,
}

impl canvas::Program<Message> for Blink {
    type State = Blinking;

    fn update(
        &self,
        state: &mut Blinking,
        event: &Event,
        _bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        let now = motion::redraw(event)?;
        if !self.motion {
            state.dark = false;
            return None;
        }
        let since = *state.since.get_or_insert(now);
        let (step, next) = steps(since, motion::BLINK, now);
        state.dark = step % 2 == 1;
        Some(next.map_or_else(canvas::Action::request_redraw, canvas::Action::request_redraw_at))
    }

    fn draw(
        &self,
        state: &Blinking,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let geometry = state.kept.draw(renderer, bounds.size(), (state.dark, *self), |frame| {
            if !state.dark {
                let size = frame.size();
                frame.fill(
                    &Path::circle(frame.center(), size.width.min(size.height) / 2.0),
                    self.color,
                );
            }
        });
        vec![geometry]
    }
}

/// Dots that scan from left to right with a fading tail, one step at a time, like a hardware
/// spinner: the buds are being looked for. Without motion, the middle dot is lit and still.
pub fn scan<'a>(look: Look, color: Color) -> Element<'a, Message> {
    let scan = Scan {
        on: color,
        off: look.p.s3,
        motion: look.motion,
    };
    let width = f32::from(SCAN_DOTS) * SCAN_PITCH - (SCAN_PITCH - SCAN_DOT);
    iced::widget::canvas(scan).width(width).height(SCAN_DOT).into()
}

const SCAN_DOTS: u16 = 9;
const SCAN_DOT: f32 = 6.0;
const SCAN_PITCH: f32 = 12.0;

#[derive(Clone, Copy, PartialEq)]
struct Scan {
    on: Color,
    off: Color,
    motion: bool,
}

#[derive(Default)]
pub struct Scanning {
    kept: Kept<(u16, Scan)>,
    since: Option<Instant>,
    /// The lit dot, by its place; past the last one, only its tail shows.
    head: Option<u16>,
}

impl canvas::Program<Message> for Scan {
    type State = Scanning;

    fn update(
        &self,
        state: &mut Scanning,
        event: &Event,
        _bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        let now = motion::redraw(event)?;
        if !self.motion {
            state.head = Some(SCAN_DOTS / 2);
            return None;
        }
        let since = *state.since.get_or_insert(now);
        let (step, next) = steps(since, motion::SCAN, now);
        // The head runs past the end until its tail of two has left too.
        let cycle = u128::from(SCAN_DOTS + 2);
        state.head = u16::try_from(step % cycle).ok();
        Some(next.map_or_else(canvas::Action::request_redraw, canvas::Action::request_redraw_at))
    }

    fn draw(
        &self,
        state: &Scanning,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let head = state.head.unwrap_or(SCAN_DOTS / 2);
        let geometry = state.kept.draw(renderer, bounds.size(), (head, *self), |frame| {
            let y = frame.size().height / 2.0;
            for n in 0..SCAN_DOTS {
                // The head at full strength, then 60% and 30%: Nothing's three steps of opacity.
                let color = match head.checked_sub(n) {
                    Some(0) => self.on,
                    Some(1) => Color {
                        a: self.on.a * 0.6,
                        ..self.on
                    },
                    Some(2) => Color {
                        a: self.on.a * 0.3,
                        ..self.on
                    },
                    _ => self.off,
                };
                let x = SCAN_DOT / 2.0 + f32::from(n) * SCAN_PITCH;
                frame.fill(&Path::circle(Point::new(x, y), SCAN_DOT / 2.0), color);
            }
        });
        vec![geometry]
    }
}

/// For something that changes every `every` from `since`: how many steps have gone by at
/// `now`, and when the next one is due (`None` past what an instant can hold).
fn steps(since: Instant, every: Duration, now: Instant) -> (u128, Option<Instant>) {
    let step = now.saturating_duration_since(since).as_millis() / every.as_millis().max(1);
    let next = u32::try_from(step.saturating_add(1))
        .ok()
        .and_then(|n| every.checked_mul(n))
        .and_then(|gone| since.checked_add(gone));
    (step, next)
}

#[cfg(test)]
mod tests {
    use super::*;
    use budctl_core::status::BassBand;

    fn slider(low: i16, high: i16) -> Slider<fn(i16, bool) -> Message> {
        Slider {
            low,
            high,
            value: 0,
            vertical: false,
            change: |_, _| Message::SaveBass(BassBand::Low),
            release: Message::SaveBass(BassBand::Low),
            on: Color::WHITE,
            off: Color::BLACK,
            motion: false,
        }
    }

    #[test]
    fn steps_come_on_time() {
        let since = Instant::now();
        let every = Duration::from_millis(100);
        assert_eq!(steps(since, every, since), (0, Some(since + every)));
        let (step, next) = steps(since, every, since + Duration::from_millis(250));
        assert_eq!(step, 2);
        assert_eq!(next, Some(since + Duration::from_millis(300)));
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
