//! Canvas drawings kept between frames.
//!
//! The CPU renderer takes a canvas drawn afresh as changed on every frame, shape by shape, and
//! repaints each shape's patch of the window on its own: with a few dozen icons and dots on a
//! page, one hovered row repaints the window dozens of times over. A kept drawing stays the same
//! drawing until what it shows changes, so only what really changed is painted again.

use std::cell::Cell;

use iced::widget::canvas::{Cache, Frame, Geometry};
use iced::{Renderer, Size};

/// A canvas's drawing of `K` (what it shows: its colors and values), drawn again only when `K`
/// or the canvas's size changes. It lives in the canvas's state, so it outlasts the view.
pub struct Kept<K> {
    cache: Cache,
    shows: Cell<Option<K>>,
}

impl<K> Default for Kept<K> {
    fn default() -> Self {
        Kept {
            cache: Cache::new(),
            shows: Cell::new(None),
        }
    }
}

impl<K: Copy + PartialEq> Kept<K> {
    /// The drawing of `shows`, `size` large: the kept one, or a new one made by `draw`.
    pub fn draw(&self, renderer: &Renderer, size: Size, shows: K, draw: impl FnOnce(&mut Frame)) -> Geometry {
        if self.shows.get() != Some(shows) {
            self.cache.clear();
            self.shows.set(Some(shows));
        }
        self.cache.draw(renderer, size, draw)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::{Font, Pixels};

    #[test]
    fn draws_again_only_when_what_it_shows_changes() {
        let renderer = Renderer::new(Font::DEFAULT, Pixels(16.0));
        let kept = Kept::default();
        let small = Size::new(10.0, 10.0);
        let mut drawn = 0;
        for (size, shows) in [
            (small, 1),
            (small, 1),
            (small, 2),
            (small, 2),
            (Size::new(20.0, 10.0), 2),
        ] {
            let _ = kept.draw(&renderer, size, shows, |_| drawn += 1);
        }
        assert_eq!(drawn, 3, "drawn for 1, for 2, and for 2 at the new size");
    }
}
