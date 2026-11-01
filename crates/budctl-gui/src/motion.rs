//! Motion, in Nothing's way: short, eased out, never bouncing, and fading rather than sliding.
//!
//! Small moves run inside their widget: a hover, a switch, a ring's dots. The widget starts its
//! tween when a redraw shows that what it should show has changed, asks for the frames the tween
//! needs, and then lets the window rest, so only its own patch is painted again. Fades of whole
//! parts (a page, a dialog) run in the app, which draws the view again on each frame of one.
//!
//! With motion off (`gui.motion` in the config, and in the tests), everything is at once.

use iced::advanced::layout::{self, Layout};
use iced::advanced::widget::{Operation, Tree, Widget, tree};
use iced::advanced::{Clipboard, Shell, mouse, overlay, renderer};
use iced::time::{Duration, Instant};
use iced::{Border, Color, Element, Event, Length, Rectangle, Renderer, Size, Theme, Vector, window};

use crate::Message;

/// A hover, in and out.
pub const HOVER: Duration = Duration::from_millis(150);
/// A change of state: a switch, the chosen segment, the keyboard cursor.
pub const STATE: Duration = Duration::from_millis(200);
/// A page fading in.
pub const PAGE: Duration = Duration::from_millis(240);
/// A dialog fading in.
pub const OPEN: Duration = Duration::from_millis(200);
/// A dialog fading out.
pub const CLOSE: Duration = Duration::from_millis(150);
/// A battery ring lighting its dots, one after the other.
pub const SWEEP: Duration = Duration::from_millis(600);
/// How long a blinking dot stays lit, and dark.
pub const BLINK: Duration = Duration::from_millis(500);
/// One step of the scanning dots.
pub const SCAN: Duration = Duration::from_millis(90);

/// The ease-out curve `cubic-bezier(0.25, 0.1, 0.25, 1)`: quick at first, settling gently. `t`
/// is the share of the time gone, from 0 to 1; the result, the share of the way gone.
pub fn ease(t: f32) -> f32 {
    // A cubic Bézier from (0, 0) to (1, 1) with control points (0.25, 0.1) and (0.25, 1),
    // along one axis.
    fn bezier(s: f32, a: f32, b: f32) -> f32 {
        let r = 1.0 - s;
        3.0 * r * r * s * a + 3.0 * r * s * s * b + s * s * s
    }
    if t <= 0.0 {
        return 0.0;
    }
    if t >= 1.0 {
        return 1.0;
    }
    // x grows with s, so halving finds the s where x is t.
    let (mut low, mut high) = (0.0_f32, 1.0_f32);
    for _ in 0..24 {
        let mid = (low + high) / 2.0;
        if bezier(mid, 0.25, 0.25) < t {
            low = mid;
        } else {
            high = mid;
        }
    }
    bezier((low + high) / 2.0, 0.1, 1.0)
}

/// How far a fade that began at `since` and lasts `length` is at `now`, eased: 0 to 1.
pub fn progress(since: Instant, length: Duration, now: Instant) -> f32 {
    let gone = now.saturating_duration_since(since).as_secs_f32();
    ease(gone / length.as_secs_f32().max(f32::EPSILON))
}

/// Whether a fade that began at `since` and lasts `length` is over at `now`.
pub fn over(since: Instant, length: Duration, now: Instant) -> bool {
    now.saturating_duration_since(since) >= length
}

/// What a tween can move: numbers, colors, paints.
pub trait Mix: Copy + PartialEq {
    /// `self` moved `t` (0 to 1) of the way to `to`.
    fn mix(self, to: Self, t: f32) -> Self;
}

impl Mix for f32 {
    fn mix(self, to: f32, t: f32) -> f32 {
        self + (to - self) * t
    }
}

impl Mix for Color {
    /// Transparent takes the other color's hue, so a fill fades in as itself, not from black.
    fn mix(self, to: Color, t: f32) -> Color {
        let from = if self.a == 0.0 { Color { a: 0.0, ..to } } else { self };
        let to = if to.a == 0.0 { Color { a: 0.0, ..from } } else { to };
        Color {
            r: from.r.mix(to.r, t),
            g: from.g.mix(to.g, t),
            b: from.b.mix(to.b, t),
            a: from.a.mix(to.a, t),
        }
    }
}

/// A value on its way to another, eased.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tween<T> {
    from: T,
    to: T,
    /// When it set off; `None` once it is there, or when it went at once.
    start: Option<Instant>,
    length: Duration,
}

impl<T: Mix> Tween<T> {
    /// A value that stays where it is.
    pub fn at_rest(value: T) -> Self {
        Tween {
            from: value,
            to: value,
            start: None,
            length: Duration::ZERO,
        }
    }

    /// Sets off for `to` from wherever it is now, taking `length`; at once without `motion`.
    /// Aiming where it already goes changes nothing.
    pub fn aim(&mut self, to: T, now: Instant, length: Duration, motion: bool) {
        if to == self.to {
            return;
        }
        self.from = self.at(now);
        self.to = to;
        self.start = motion.then_some(now);
        self.length = length;
    }

    /// Where it is at `now`.
    pub fn at(&self, now: Instant) -> T {
        match self.start {
            Some(start) if !over(start, self.length, now) => self.from.mix(self.to, progress(start, self.length, now)),
            _ => self.to,
        }
    }

    /// Where it is going.
    pub fn target(&self) -> T {
        self.to
    }

    /// Whether it is still on its way at `now`.
    pub fn running(&self, now: Instant) -> bool {
        self.start.is_some_and(|start| !over(start, self.length, now))
    }
}

/// When a redraw is under way, its time.
pub fn redraw(event: &Event) -> Option<Instant> {
    match event {
        Event::Window(window::Event::RedrawRequested(now)) => Some(*now),
        _ => None,
    }
}

/// A surface's paint: its fill, and its 2 px frame (the keyboard cursor). Transparent is none.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Paint {
    pub fill: Color,
    pub frame: Color,
}

impl Paint {
    pub const NONE: Paint = Paint {
        fill: Color::TRANSPARENT,
        frame: Color::TRANSPARENT,
    };

    /// Just a fill.
    pub fn fill(color: Color) -> Paint {
        Paint {
            fill: color,
            frame: Color::TRANSPARENT,
        }
    }

    /// `self`, framed in `color` when `shown`.
    pub fn framed(self, color: Color, shown: bool) -> Paint {
        Paint {
            frame: if shown { color } else { Color::TRANSPARENT },
            ..self
        }
    }
}

impl Mix for Paint {
    fn mix(self, to: Paint, t: f32) -> Paint {
        Paint {
            fill: self.fill.mix(to.fill, t),
            frame: self.frame.mix(to.frame, t),
        }
    }
}

/// `content` on a surface painted `rest`, or `hover` while the mouse is over it (when
/// `interactive`). A change of paint fades: the hover in [`HOVER`], anything else (the chosen
/// segment, the keyboard cursor) in [`STATE`].
///
/// The content keeps its own clicks and keys; give a button in it no background or border.
pub fn eased<'a>(
    content: impl Into<Element<'a, Message>>,
    rest: Paint,
    hover: Paint,
    radius: f32,
    interactive: bool,
    motion: bool,
) -> Element<'a, Message> {
    Element::new(Eased {
        content: content.into(),
        rest,
        hover,
        radius,
        interactive,
        motion,
    })
}

/// `content` on a surface of one paint, which fades when it changes.
pub fn surface<'a>(
    content: impl Into<Element<'a, Message>>,
    paint: Paint,
    radius: f32,
    motion: bool,
) -> Element<'a, Message> {
    eased(content, paint, paint, radius, false, motion)
}

struct Eased<'a> {
    content: Element<'a, Message>,
    rest: Paint,
    hover: Paint,
    radius: f32,
    interactive: bool,
    motion: bool,
}

/// What an [`Eased`] surface shows, kept between views.
struct Shown {
    hovered: bool,
    paint: Tween<Paint>,
    /// The time of the last redraw: the paint is drawn as it is then.
    now: Option<Instant>,
}

impl Widget<Message, Theme, Renderer> for Eased<'_> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<Shown>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(Shown {
            hovered: false,
            paint: Tween::at_rest(self.rest),
            now: None,
        })
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_ref(&self.content));
    }

    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn size_hint(&self) -> Size<Length> {
        self.content.as_widget().size_hint()
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &layout::Limits) -> layout::Node {
        match tree.children.first_mut() {
            Some(child) => self.content.as_widget_mut().layout(child, renderer, limits),
            None => layout::Node::new(Size::ZERO),
        }
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        if let Some(child) = tree.children.first_mut() {
            self.content
                .as_widget_mut()
                .update(child, event, layout, cursor, renderer, clipboard, shell, viewport);
        }
        let shown = tree.state.downcast_mut::<Shown>();
        // A fade starts now, not at the last redraw: that may be long gone, and the fade with it.
        let now = match redraw(event) {
            Some(now) => {
                shown.now = Some(now);
                now
            }
            None => Instant::now(),
        };
        let hovered = self.interactive && cursor.is_over(layout.bounds());
        let length = if hovered == shown.hovered { STATE } else { HOVER };
        shown.hovered = hovered;
        let target = if hovered { self.hover } else { self.rest };
        if target != shown.paint.target() || shown.paint.running(now) {
            shown.paint.aim(target, now, length, self.motion);
            shell.request_redraw();
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        use iced::advanced::Renderer as _;

        let shown = tree.state.downcast_ref::<Shown>();
        let paint = shown.now.map_or(shown.paint.target(), |now| shown.paint.at(now));
        if paint.fill.a > 0.0 || paint.frame.a > 0.0 {
            renderer.fill_quad(
                renderer::Quad {
                    bounds: layout.bounds(),
                    border: Border {
                        color: paint.frame,
                        width: if paint.frame.a > 0.0 { 2.0 } else { 0.0 },
                        radius: self.radius.into(),
                    },
                    ..renderer::Quad::default()
                },
                paint.fill,
            );
        }
        if let Some(child) = tree.children.first() {
            self.content
                .as_widget()
                .draw(child, renderer, theme, style, layout, cursor, viewport);
        }
    }

    fn operate(&mut self, tree: &mut Tree, layout: Layout<'_>, renderer: &Renderer, operation: &mut dyn Operation) {
        if let Some(child) = tree.children.first_mut() {
            self.content.as_widget_mut().operate(child, layout, renderer, operation);
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        tree.children.first().map_or_else(mouse::Interaction::default, |child| {
            self.content
                .as_widget()
                .mouse_interaction(child, layout, cursor, viewport, renderer)
        })
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        let child = tree.children.first_mut()?;
        self.content
            .as_widget_mut()
            .overlay(child, layout, renderer, viewport, translation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_curve_eases_out() {
        assert!(ease(0.0).abs() < 1e-4);
        assert!((ease(1.0) - 1.0).abs() < 1e-4);
        // Out of range is held at the ends.
        assert!(ease(-1.0).abs() < 1e-4);
        assert!((ease(2.0) - 1.0).abs() < 1e-4);
        // Most of the way is gone by half the time: it settles gently.
        assert!(ease(0.5) > 0.75);
        let mut last = 0.0;
        for step in 1..=100 {
            let now = ease(step as f32 / 100.0);
            assert!(now >= last, "never goes back, so never bounces");
            assert!(now <= 1.0 + 1e-4, "never overshoots");
            last = now;
        }
    }

    #[test]
    fn tweens_go_where_they_are_aimed() {
        let start = Instant::now();
        let mut tween = Tween::at_rest(0.0_f32);
        assert!(!tween.running(start));
        tween.aim(10.0, start, STATE, true);
        assert!(tween.running(start));
        assert_eq!(tween.at(start), 0.0);
        let halfway = tween.at(start + STATE / 2);
        assert!(halfway > 5.0 && halfway < 10.0, "{halfway}");
        assert_eq!(tween.at(start + STATE), 10.0);
        assert!(!tween.running(start + STATE));

        // Aimed elsewhere on the way, it sets off from where it is.
        tween.aim(0.0, start + STATE / 2, STATE, true);
        assert!((tween.at(start + STATE / 2) - halfway).abs() < 1e-3);

        // Without motion, it is there at once.
        let mut still = Tween::at_rest(0.0_f32);
        still.aim(1.0, start, STATE, false);
        assert!(!still.running(start));
        assert_eq!(still.at(start), 1.0);
    }

    #[test]
    fn colors_fade_in_as_themselves() {
        let red = Color::from_rgb(1.0, 0.0, 0.0);
        let half = Color::TRANSPARENT.mix(red, 0.5);
        assert_eq!(half, Color { a: 0.5, ..red });
        let half = red.mix(Color::TRANSPARENT, 0.5);
        assert_eq!(half, Color { a: 0.5, ..red });
        assert_eq!(red.mix(Color::WHITE, 1.0), Color::WHITE);
    }
}
