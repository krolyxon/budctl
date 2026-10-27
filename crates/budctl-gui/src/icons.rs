//! Icons and battery rings, drawn as vector paths so they take any theme color and stay sharp at
//! any scale. Icons are designed on a 24 × 24 grid.

use std::f32::consts::{FRAC_PI_2, TAU};

use iced::widget::canvas::{self, Frame, LineCap, LineJoin, Path, Stroke, path::Arc};
use iced::{Color, Element, Point, Radians, Rectangle, Renderer, Size, Theme, mouse};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    Overview,
    Equalizer,
    Gestures,
    Earbud,
    Settings,
    NoiseOff,
    Anc,
    Transparency,
    Game,
    Spatial,
    Bass,
    Wind,
    Dual,
    Chevron,
    Bolt,
}

/// `icon` at `size` pixels, in `color`.
pub fn icon<'a, Message: 'a>(icon: Icon, size: f32, color: Color) -> Element<'a, Message> {
    iced::widget::canvas(Glyph { icon, color })
        .width(size)
        .height(size)
        .into()
}

struct Glyph {
    icon: Icon,
    color: Color,
}

impl<Message> canvas::Program<Message> for Glyph {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let mut pen = Pen::new(&mut frame, Point::ORIGIN, bounds.width, self.color);
        draw_icon(&mut pen, self.icon);
        vec![frame.into_geometry()]
    }
}

/// Draws on a 24 × 24 grid placed at `origin`, `size` pixels wide.
struct Pen<'f> {
    frame: &'f mut Frame,
    origin: Point,
    scale: f32,
    color: Color,
    width: f32,
}

impl<'f> Pen<'f> {
    fn new(frame: &'f mut Frame, origin: Point, size: f32, color: Color) -> Self {
        let scale = size / 24.0;
        Pen {
            frame,
            origin,
            scale,
            color,
            width: 1.9 * scale,
        }
    }

    fn at(&self, x: f32, y: f32) -> Point {
        Point::new(self.origin.x + x * self.scale, self.origin.y + y * self.scale)
    }

    fn stroke(&mut self, path: &Path) {
        let stroke = Stroke::default()
            .with_color(self.color)
            .with_width(self.width)
            .with_line_cap(LineCap::Round)
            .with_line_join(LineJoin::Round);
        self.frame.stroke(path, stroke);
    }

    fn polyline(&mut self, points: &[(f32, f32)], closed: bool) {
        let path = Path::new(|b| {
            for (n, &(x, y)) in points.iter().enumerate() {
                if n == 0 {
                    b.move_to(self.at(x, y));
                } else {
                    b.line_to(self.at(x, y));
                }
            }
            if closed {
                b.close();
            }
        });
        self.stroke(&path);
    }

    fn line(&mut self, from: (f32, f32), to: (f32, f32)) {
        self.polyline(&[from, to], false);
    }

    fn circle(&mut self, x: f32, y: f32, r: f32) {
        let path = Path::circle(self.at(x, y), r * self.scale);
        self.stroke(&path);
    }

    fn dot(&mut self, x: f32, y: f32, r: f32) {
        let path = Path::circle(self.at(x, y), r * self.scale);
        self.frame.fill(&path, self.color);
    }

    /// Degrees, clockwise from three o'clock. `to` may be below `from` to go the other way.
    fn arc(&mut self, x: f32, y: f32, r: f32, from: f32, to: f32) {
        let path = Path::new(|b| {
            b.arc(Arc {
                center: self.at(x, y),
                radius: r * self.scale,
                start_angle: Radians(from.to_radians()),
                end_angle: Radians(to.to_radians()),
            });
        });
        self.stroke(&path);
    }

    fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32) {
        let path = self.rounded(x, y, w, h, r);
        self.stroke(&path);
    }

    fn fill_rect(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32) {
        let path = self.rounded(x, y, w, h, r);
        self.frame.fill(&path, self.color);
    }

    fn rounded(&self, x: f32, y: f32, w: f32, h: f32, r: f32) -> Path {
        Path::rounded_rectangle(
            self.at(x, y),
            Size::new(w * self.scale, h * self.scale),
            (r * self.scale).into(),
        )
    }
}

fn draw_icon(pen: &mut Pen, icon: Icon) {
    match icon {
        Icon::Overview => {
            for (x, y) in [(3.0, 3.0), (13.5, 3.0), (3.0, 13.5), (13.5, 13.5)] {
                pen.rect(x, y, 7.5, 7.5, 2.0);
            }
        }
        Icon::Equalizer => {
            for (x, knob) in [(6.0, 14.0), (12.0, 8.0), (18.0, 16.0)] {
                pen.line((x, 3.5), (x, 20.5));
                pen.line((x - 2.5, knob), (x + 2.5, knob));
            }
        }
        Icon::Gestures => {
            pen.rect(9.5, 11.0, 5.0, 11.0, 2.5);
            pen.arc(12.0, 11.5, 4.0, 200.0, 340.0);
            pen.arc(12.0, 11.5, 7.5, 210.0, 330.0);
        }
        Icon::Earbud => earbud(pen, false, None),
        Icon::Settings => {
            for (y, knob) in [(6.0, 15.0), (12.0, 8.0), (18.0, 13.0)] {
                pen.line((3.5, y), (20.5, y));
                pen.dot(knob, y, 2.6);
            }
        }
        Icon::NoiseOff => {
            pen.circle(12.0, 12.0, 8.5);
            pen.line((6.0, 6.0), (18.0, 18.0));
        }
        Icon::Anc => {
            pen.circle(12.0, 12.0, 8.5);
            pen.circle(12.0, 12.0, 4.5);
            pen.dot(12.0, 12.0, 1.4);
        }
        Icon::Transparency => {
            for n in 0..12u8 {
                let angle = f32::from(n) * 30f32.to_radians();
                pen.dot(12.0 + 8.5 * angle.cos(), 12.0 + 8.5 * angle.sin(), 1.15);
            }
            pen.dot(12.0, 12.0, 2.2);
        }
        Icon::Game | Icon::Bolt => pen.polyline(
            &[
                (13.5, 2.5),
                (5.0, 13.5),
                (11.5, 13.5),
                (10.5, 21.5),
                (19.0, 10.5),
                (12.5, 10.5),
            ],
            true,
        ),
        Icon::Spatial => {
            pen.dot(12.0, 12.0, 2.4);
            for (r, spread) in [(5.5, 40.0), (9.5, 35.0)] {
                pen.arc(12.0, 12.0, r, -spread, spread);
                pen.arc(12.0, 12.0, r, 180.0 - spread, 180.0 + spread);
            }
        }
        Icon::Bass => {
            pen.polyline(
                &[
                    (3.5, 9.5),
                    (7.5, 9.5),
                    (12.5, 5.0),
                    (12.5, 19.0),
                    (7.5, 14.5),
                    (3.5, 14.5),
                ],
                true,
            );
            pen.arc(13.0, 12.0, 4.0, -50.0, 50.0);
            pen.arc(13.0, 12.0, 8.0, -50.0, 50.0);
        }
        Icon::Wind => {
            pen.line((3.0, 8.5), (13.0, 8.5));
            pen.arc(13.0, 6.0, 2.5, 90.0, -180.0);
            pen.line((3.0, 12.5), (17.5, 12.5));
            pen.arc(17.5, 15.5, 3.0, -90.0, 180.0);
            pen.line((3.0, 16.5), (9.0, 16.5));
        }
        Icon::Dual => {
            pen.rect(2.5, 5.0, 12.5, 9.0, 1.5);
            pen.line((1.5, 17.5), (16.0, 17.5));
            pen.rect(17.5, 8.0, 4.5, 11.0, 1.2);
        }
        Icon::Chevron => pen.polyline(&[(9.0, 5.5), (15.5, 12.0), (9.0, 18.5)], false),
    }
}

/// A filled earbud: the head, the ear tip pointing in, and the stem going down. The tip is
/// drawn in `tip`, or in the part's color when there is none.
fn earbud(pen: &mut Pen, mirrored: bool, tip: Option<Color>) {
    let x = |x: f32, w: f32| if mirrored { 24.0 - x - w } else { x };
    let color = pen.color;
    pen.color = tip.unwrap_or(color);
    pen.fill_rect(x(14.5, 6.0), 5.0, 6.0, 7.0, 2.8);
    pen.color = color;
    pen.dot(x(11.0, 0.0), 8.5, 6.3);
    pen.fill_rect(x(6.6, 4.4), 10.0, 4.4, 12.5, 2.2);
}

/// What a battery ring shows in its middle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Part {
    Left,
    Right,
    Case,
}

/// A battery ring: the track, an arc for the charge (none when unknown) and the part in the
/// middle.
pub fn battery_ring<'a, Message: 'a>(
    part: Part,
    level: Option<u8>,
    size: f32,
    colors: RingColors,
) -> Element<'a, Message> {
    iced::widget::canvas(Ring { part, level, colors })
        .width(size)
        .height(size)
        .into()
}

#[derive(Debug, Clone, Copy)]
pub struct RingColors {
    pub track: Color,
    pub charge: Color,
    pub part: Color,
    /// The earbuds' ear tips.
    pub tip: Color,
    /// Details cut into the case (the lid line, the light).
    pub cutout: Color,
}

struct Ring {
    part: Part,
    level: Option<u8>,
    colors: RingColors,
}

impl<Message> canvas::Program<Message> for Ring {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let center = frame.center();
        let width = bounds.width * 0.06;
        let radius = bounds.width / 2.0 - width / 2.0 - 1.0;
        let ring = |color: Color| {
            Stroke::default()
                .with_color(color)
                .with_width(width)
                .with_line_cap(LineCap::Round)
        };
        frame.stroke(&Path::circle(center, radius), ring(self.colors.track));
        if let Some(level) = self.level.filter(|&level| level > 0) {
            let sweep = f32::from(level.min(100)) / 100.0 * TAU;
            let arc = Path::new(|b| {
                b.arc(Arc {
                    center,
                    radius,
                    start_angle: Radians(-FRAC_PI_2),
                    end_angle: Radians(-FRAC_PI_2 + sweep),
                });
            });
            frame.stroke(&arc, ring(self.colors.charge));
        }

        let art = bounds.width * 0.44;
        let origin = Point::new(center.x - art / 2.0, center.y - art / 2.0);
        let mut pen = Pen::new(&mut frame, origin, art, self.colors.part);
        match self.part {
            Part::Left => earbud(&mut pen, false, Some(self.colors.tip)),
            Part::Right => earbud(&mut pen, true, Some(self.colors.tip)),
            Part::Case => {
                pen.fill_rect(2.0, 5.5, 20.0, 13.5, 6.0);
                pen.color = self.colors.cutout;
                pen.width = 0.9 * pen.scale;
                pen.line((2.0, 10.0), (22.0, 10.0));
                pen.dot(12.0, 14.5, 1.0);
            }
        }
        vec![frame.into_geometry()]
    }
}
