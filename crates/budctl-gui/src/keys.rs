//! The keyboard layer: vim-like keys on top of a mouse-first window.
//!
//! A cursor moves over the things a page can change. It stays hidden until a key is pressed and
//! hides again on a click, so mouse users never see it. This module only turns key presses into
//! [`Action`]s; `App::update` decides what an action does on the current page.

use iced::keyboard::key::Named;
use iced::keyboard::{Key, Modifiers};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Down,
    Up,
    First,
    Last,
    /// The previous choice, or off.
    Left,
    /// The next choice, or on, or open what the item leads to.
    Right,
    /// Toggle, step to the next choice, or open.
    Activate,
    NextPage,
    PreviousPage,
    /// A page by its place in the sidebar, from 0.
    Page(usize),
    /// Read the buds again.
    Refresh,
    /// Show or hide the list of keys.
    Help,
    /// Close the list of keys, else hide the cursor.
    Escape,
    Quit,
}

/// The keys, as the list of keys shows them.
pub const HELP: [(&str, &str); 11] = [
    ("j  k   ↓  ↑", "move the cursor"),
    ("h  l   ←  →", "previous / next choice, off / on"),
    ("Enter  Space", "toggle, next choice, or open"),
    ("gg  G", "first / last"),
    ("J  K   Ctrl+Tab", "next / previous page"),
    ("gt  gT", "next / previous page"),
    ("1 … 5", "go to a page"),
    ("r", "read the buds again"),
    ("Esc", "hide the cursor"),
    ("?", "these keys"),
    ("q", "quit"),
];

/// Turns key presses into actions. It remembers a `g`, for `gg`, `gt` and `gT`.
#[derive(Debug, Default)]
pub struct Keys {
    after_g: bool,
}

impl Keys {
    pub fn press(&mut self, key: &Key, modifiers: Modifiers) -> Option<Action> {
        let after_g = std::mem::take(&mut self.after_g);
        let shift = modifiers.shift();
        match key {
            Key::Named(named) => Some(match named {
                Named::ArrowDown => Action::Down,
                Named::ArrowUp => Action::Up,
                Named::ArrowLeft => Action::Left,
                Named::ArrowRight => Action::Right,
                Named::Tab if modifiers.control() && shift => Action::PreviousPage,
                Named::Tab if modifiers.control() => Action::NextPage,
                Named::Tab if shift => Action::Up,
                Named::Tab => Action::Down,
                Named::Enter | Named::Space => Action::Activate,
                Named::Home => Action::First,
                Named::End => Action::Last,
                Named::Escape => Action::Escape,
                _ => return None,
            }),
            Key::Character(c) if modifiers.control() => match c.as_str() {
                "j" => Some(Action::NextPage),
                "k" => Some(Action::PreviousPage),
                "q" => Some(Action::Quit),
                _ => None,
            },
            Key::Character(_) if modifiers.alt() || modifiers.logo() => None,
            Key::Character(c) => match c.as_str() {
                "g" if after_g => Some(Action::First),
                "t" if after_g => Some(Action::NextPage),
                "T" if after_g => Some(Action::PreviousPage),
                "g" => {
                    self.after_g = true;
                    None
                }
                "j" => Some(Action::Down),
                "k" => Some(Action::Up),
                "h" => Some(Action::Left),
                "l" => Some(Action::Right),
                "G" => Some(Action::Last),
                "J" => Some(Action::NextPage),
                "K" => Some(Action::PreviousPage),
                " " => Some(Action::Activate),
                "r" => Some(Action::Refresh),
                "?" => Some(Action::Help),
                "q" => Some(Action::Quit),
                digit => match digit.parse::<usize>() {
                    Ok(n @ 1..=9) => Some(Action::Page(n - 1)),
                    _ => None,
                },
            },
            Key::Unidentified => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(keys: &mut Keys, key: &str) -> Option<Action> {
        let (key, modifiers) = match key {
            "Down" => (Key::Named(Named::ArrowDown), Modifiers::empty()),
            "Enter" => (Key::Named(Named::Enter), Modifiers::empty()),
            "Space" => (Key::Named(Named::Space), Modifiers::empty()),
            "Esc" => (Key::Named(Named::Escape), Modifiers::empty()),
            "Tab" => (Key::Named(Named::Tab), Modifiers::empty()),
            "S-Tab" => (Key::Named(Named::Tab), Modifiers::SHIFT),
            "C-Tab" => (Key::Named(Named::Tab), Modifiers::CTRL),
            "C-S-Tab" => (Key::Named(Named::Tab), Modifiers::CTRL | Modifiers::SHIFT),
            "C-j" => (Key::Character("j".into()), Modifiers::CTRL),
            "A-j" => (Key::Character("j".into()), Modifiers::ALT),
            c if c.chars().next().is_some_and(char::is_uppercase) => (Key::Character(c.into()), Modifiers::SHIFT),
            c => (Key::Character(c.into()), Modifiers::empty()),
        };
        keys.press(&key, modifiers)
    }

    #[test]
    fn single_keys() {
        let mut keys = Keys::default();
        let cases = [
            ("j", Some(Action::Down)),
            ("Down", Some(Action::Down)),
            ("Tab", Some(Action::Down)),
            ("k", Some(Action::Up)),
            ("S-Tab", Some(Action::Up)),
            ("h", Some(Action::Left)),
            ("l", Some(Action::Right)),
            ("Enter", Some(Action::Activate)),
            ("Space", Some(Action::Activate)),
            ("G", Some(Action::Last)),
            ("J", Some(Action::NextPage)),
            ("K", Some(Action::PreviousPage)),
            ("C-Tab", Some(Action::NextPage)),
            ("C-S-Tab", Some(Action::PreviousPage)),
            ("C-j", Some(Action::NextPage)),
            ("3", Some(Action::Page(2))),
            ("0", None),
            ("r", Some(Action::Refresh)),
            ("?", Some(Action::Help)),
            ("Esc", Some(Action::Escape)),
            ("q", Some(Action::Quit)),
            ("x", None),
            ("A-j", None),
        ];
        for (key, action) in cases {
            assert_eq!(press(&mut keys, key), action, "{key}");
        }
    }

    #[test]
    fn g_waits_for_the_next_key() {
        let mut keys = Keys::default();
        assert_eq!(press(&mut keys, "g"), None);
        assert_eq!(press(&mut keys, "g"), Some(Action::First));
        assert_eq!(press(&mut keys, "g"), None);
        assert_eq!(press(&mut keys, "t"), Some(Action::NextPage));
        assert_eq!(press(&mut keys, "g"), None);
        assert_eq!(press(&mut keys, "T"), Some(Action::PreviousPage));
        // Anything else after a g counts on its own, and the g is forgotten.
        assert_eq!(press(&mut keys, "g"), None);
        assert_eq!(press(&mut keys, "j"), Some(Action::Down));
        assert_eq!(press(&mut keys, "t"), None);
    }
}
