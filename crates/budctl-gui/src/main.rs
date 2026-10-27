//! `budctl-gui`: control wireless earbuds from Linux, in a window.
//!
//! A thin layer over budctl-core, like the CLI. For now it shows demo data: the look comes first,
//! the live connection later (CLAUDE.md, GUI). The mouse comes first; vim-like keys work on top
//! (`keys.rs`).

mod demo;
mod icons;
mod keys;
mod overview;
mod theme;
mod ui;

use std::path::PathBuf;
use std::process::ExitCode;

use budctl_core::status::{AncLevel, NoiseControl, NoiseMode};
use iced::keyboard::{self, Key, Modifiers};
use iced::widget::operation::{self, RelativeOffset};
use iced::widget::{Column, column, container, mouse_area, opaque, row, scrollable, space, stack, text};
use iced::{Alignment, Border, Color, Element, Font, Length, Size, Subscription, Task, Theme, event, mouse, window};

use icons::Icon;
use keys::Action;
use theme::Look;

const USAGE: &str = "usage: budctl-gui [--theme <palette.json>]

Colors come from --theme, else from ~/.config/budctl/theme.json if it exists, else built in.
The palette file takes the keys of a matugen palette like Aurora's (bg, fg, primary, …).
Press ? in the window for the keys.";

/// The page's scrolling area, so the cursor can be kept in view.
const SCROLL: &str = "page";

fn main() -> ExitCode {
    let theme_file = match parse_args(std::env::args().skip(1)) {
        Ok(Args::Run { theme }) => theme.or_else(theme::default_file),
        Ok(Args::Help) => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Err(message) => {
            eprintln!("budctl-gui: {message}\n{USAGE}");
            return ExitCode::from(1);
        }
    };
    let font = theme::system_font();
    let result = iced::application(move || App::new(theme_file.clone(), font), App::update, App::view)
        .title("budctl")
        .theme(App::theme)
        .subscription(App::subscription)
        .default_font(font)
        .window(window::Settings {
            size: Size::new(1040.0, 800.0),
            min_size: Some(Size::new(760.0, 540.0)),
            platform_specific: window::settings::PlatformSpecific {
                application_id: "budctl".to_owned(),
                ..window::settings::PlatformSpecific::default()
            },
            ..window::Settings::default()
        })
        .run();
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("budctl-gui: {e}");
            ExitCode::FAILURE
        }
    }
}

enum Args {
    Run { theme: Option<PathBuf> },
    Help,
}

fn parse_args(mut args: impl Iterator<Item = String>) -> Result<Args, String> {
    let mut theme = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--theme" => theme = Some(PathBuf::from(args.next().ok_or("--theme needs a file")?)),
            "-h" | "--help" => return Ok(Args::Help),
            other => return Err(format!("unknown argument {other:?}")),
        }
    }
    Ok(Args::Run { theme })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Page {
    Overview,
    Equalizer,
    Gestures,
    Earbuds,
    Settings,
}

impl Page {
    const ALL: [Page; 5] = [
        Page::Overview,
        Page::Equalizer,
        Page::Gestures,
        Page::Earbuds,
        Page::Settings,
    ];

    fn title(self) -> &'static str {
        match self {
            Page::Overview => "Overview",
            Page::Equalizer => "Equalizer",
            Page::Gestures => "Gestures",
            Page::Earbuds => "Earbud settings",
            Page::Settings => "App settings",
        }
    }

    fn icon(self) -> Icon {
        match self {
            Page::Overview => Icon::Overview,
            Page::Equalizer => Icon::Equalizer,
            Page::Gestures => Icon::Gestures,
            Page::Earbuds => Icon::Earbud,
            Page::Settings => Icon::Settings,
        }
    }

    /// The next page in the sidebar, or the previous one, going round.
    fn step(self, forward: bool) -> Page {
        let count = Page::ALL.len();
        let at = Page::ALL.iter().position(|&page| page == self).unwrap_or(0);
        let to = if forward { at + 1 } else { at + count - 1 };
        Page::ALL.get(to % count).copied().unwrap_or(self)
    }
}

/// What the keyboard cursor can rest on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Item {
    NoiseMode,
    AncLevel,
    /// A switch, by its key.
    Switch(&'static str),
    /// A row that opens a page.
    Open(Page),
}

#[derive(Debug, Clone)]
enum Message {
    Open(Page),
    Noise(NoiseMode),
    /// A switch's key, and whether to turn it on.
    Switch(&'static str, bool),
    Connect(bool),
    /// The window got focus: the palette file may have changed meanwhile.
    Focused,
    Key(Key, Modifiers),
    /// A mouse button went down: the mouse is in use, so the keyboard cursor hides.
    Pointer,
    CloseHelp,
}

struct App {
    page: Page,
    buds: demo::Buds,
    /// The level "Noise cancelling" turns on at: the one last reported or picked.
    anc_level: AncLevel,
    look: Look,
    theme: Theme,
    palette: theme::Source,
    keys: keys::Keys,
    /// Where the keyboard cursor is, among [`App::items`].
    cursor: usize,
    /// Whether it shows. Only keys show it; a click hides it.
    cursor_shown: bool,
    /// Whether the list of keys is open.
    help: bool,
}

impl App {
    fn new(theme_file: Option<PathBuf>, font: Font) -> Self {
        let buds = demo::buds();
        let anc_level = buds
            .status
            .noise_control
            .as_ref()
            .and_then(|now| now.level.as_deref())
            .and_then(anc_level)
            .unwrap_or(AncLevel::Deep);
        let mut app = App {
            page: Page::Overview,
            buds,
            anc_level,
            look: Look {
                p: theme::BUILT_IN,
                font,
            },
            theme: theme::iced_theme(&theme::BUILT_IN),
            palette: theme::Source::new(theme_file),
            keys: keys::Keys::default(),
            cursor: 0,
            cursor_shown: false,
            help: false,
        };
        app.refresh_palette();
        app
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Open(page) => return self.open(page),
            Message::Noise(mode) => {
                let level = match mode {
                    NoiseMode::Anc(level) => {
                        self.anc_level = level;
                        Some(level.name().to_owned())
                    }
                    NoiseMode::Off | NoiseMode::Transparency => None,
                };
                self.buds.status.noise_control = Some(NoiseControl {
                    mode: mode.name().to_owned(),
                    level,
                });
            }
            Message::Switch(key, on) => {
                if let Some(switch) = self.buds.status.switches.iter_mut().find(|s| s.key == key) {
                    switch.on = on;
                }
            }
            Message::Connect(on) => self.buds.connected = on,
            Message::Focused => self.refresh_palette(),
            Message::Key(key, modifiers) => return self.key(&key, modifiers),
            Message::Pointer => self.cursor_shown = false,
            Message::CloseHelp => self.help = false,
        }
        Task::none()
    }

    /// Shows `page` from its top, with the cursor on its first item.
    fn open(&mut self, page: Page) -> Task<Message> {
        self.page = page;
        self.cursor = 0;
        operation::snap_to(SCROLL, RelativeOffset::START)
    }

    fn key(&mut self, key: &Key, modifiers: Modifiers) -> Task<Message> {
        let Some(action) = self.keys.press(key, modifiers) else {
            return Task::none();
        };
        if self.help {
            // The list of keys is in front of everything: a key only closes it.
            self.help = false;
            return Task::none();
        }
        match action {
            Action::Quit => iced::exit(),
            Action::Help => {
                self.help = true;
                Task::none()
            }
            Action::Escape => {
                self.cursor_shown = false;
                Task::none()
            }
            Action::NextPage => self.open(self.page.step(true)),
            Action::PreviousPage => self.open(self.page.step(false)),
            Action::Page(n) => match Page::ALL.get(n) {
                Some(&page) => self.open(page),
                None => Task::none(),
            },
            Action::Down
            | Action::Up
            | Action::First
            | Action::Last
            | Action::Left
            | Action::Right
            | Action::Activate => self.on_cursor(action),
        }
    }

    /// What the current page offers the keyboard cursor, top to bottom.
    fn items(&self) -> Vec<Item> {
        match self.page {
            Page::Overview => overview::items(&self.buds),
            _ => Vec::new(),
        }
    }

    /// The item under the cursor, when the cursor shows.
    fn focus(&self) -> Option<Item> {
        let items = self.items();
        let last = items.len().checked_sub(1)?;
        self.cursor_shown.then(|| items.get(self.cursor.min(last)).copied())?
    }

    fn on_cursor(&mut self, action: Action) -> Task<Message> {
        let items = self.items();
        let Some(last) = items.len().checked_sub(1) else {
            return Task::none();
        };
        self.cursor = self.cursor.min(last);
        if !self.cursor_shown {
            // The first key only shows the cursor, so nothing changes where it cannot be seen.
            self.cursor_shown = true;
            return self.follow();
        }
        match action {
            Action::Down => self.cursor = (self.cursor + 1).min(last),
            Action::Up => self.cursor = self.cursor.saturating_sub(1),
            Action::First => self.cursor = 0,
            Action::Last => self.cursor = last,
            _ => {
                let change = items.get(self.cursor).and_then(|&item| self.change(item, action));
                return match change {
                    Some(message) => self.update(message),
                    None => Task::none(),
                };
            }
        }
        self.follow()
    }

    /// What `action` (`h`, `l` or Enter) does to `item`, if anything.
    fn change(&self, item: Item, action: Action) -> Option<Message> {
        match item {
            Item::NoiseMode => {
                let modes = [NoiseMode::Off, NoiseMode::Anc(self.anc_level), NoiseMode::Transparency];
                let now = self.buds.status.noise_control.as_ref()?;
                let at = modes.iter().position(|mode| mode.name() == now.mode);
                step(&modes, at, action).map(Message::Noise)
            }
            Item::AncLevel => {
                let levels = [AncLevel::Light, AncLevel::Medium, AncLevel::Deep];
                let at = levels.iter().position(|&level| level == self.anc_level);
                step(&levels, at, action).map(|level| Message::Noise(NoiseMode::Anc(level)))
            }
            Item::Switch(key) => {
                let on = self.buds.status.switches.iter().find(|s| s.key == key)?.on;
                let want = match action {
                    Action::Left => false,
                    Action::Right => true,
                    _ => !on,
                };
                (want != on).then_some(Message::Switch(key, want))
            }
            Item::Open(page) => (action != Action::Left).then_some(Message::Open(page)),
        }
    }

    /// Scrolls the page so the cursor stays in view: the first item at the top, the last at the
    /// bottom, the others in between.
    fn follow(&self) -> Task<Message> {
        let count = self.items().len();
        if !self.cursor_shown || count < 2 {
            return Task::none();
        }
        let y = self.cursor as f32 / (count - 1) as f32;
        operation::snap_to(SCROLL, RelativeOffset { x: 0.0, y })
    }

    fn refresh_palette(&mut self) {
        let palette = match self.palette.refresh() {
            None => return,
            Some(Ok(palette)) => palette,
            Some(Err(e)) => {
                eprintln!("budctl-gui: {e}; using the built-in colors");
                theme::BUILT_IN
            }
        };
        self.look.p = palette;
        self.theme = theme::iced_theme(&palette);
    }

    fn theme(&self) -> Theme {
        self.theme.clone()
    }

    fn subscription(&self) -> Subscription<Message> {
        event::listen_with(|event, _status, _window| match event {
            iced::Event::Window(window::Event::Focused) => Some(Message::Focused),
            iced::Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) => {
                Some(Message::Key(key, modifiers))
            }
            iced::Event::Mouse(mouse::Event::ButtonPressed(_)) => Some(Message::Pointer),
            _ => None,
        })
    }

    fn view(&self) -> Element<'_, Message> {
        let look = self.look;
        let page = match self.page {
            Page::Overview => overview::view(look, &self.buds, self.anc_level, self.focus()),
            page => column![
                ui::title(look, page.title()),
                text("This page comes in a later step.").size(14).color(look.p.dim)
            ]
            .spacing(12)
            .into(),
        };
        let p = look.p;
        let content = scrollable(
            container(container(page).max_width(860))
                .center_x(Length::Fill)
                .padding([30, 36]),
        )
        .id(SCROLL)
        .height(Length::Fill)
        .style(move |theme, status| {
            let mut style = scrollable::default(theme, status);
            for rail in [&mut style.vertical_rail, &mut style.horizontal_rail] {
                rail.background = None;
                rail.border = Border::default();
                rail.scroller.background = p.surface_highest.into();
                rail.scroller.border = Border::default().rounded(4);
            }
            style
        });
        let window = row![self.sidebar(), ui::divider(look), content];
        if self.help {
            stack![window, self.help()].into()
        } else {
            window.into()
        }
    }

    fn sidebar(&self) -> Element<'_, Message> {
        let look = self.look;
        let p = look.p;
        let connected = self.buds.connected;
        let (light, state) = if connected {
            (p.ok, "Connected")
        } else {
            (p.dim, "Not connected")
        };
        let device = container(
            column![
                text(self.buds.name.as_str()).size(16).font(look.bold()).color(p.fg),
                row![
                    ui::light(light),
                    text(state).size(13).color(p.dim),
                    space::horizontal(),
                    ui::tag(look, "demo")
                ]
                .spacing(8)
                .align_y(Alignment::Center),
                ui::outline_button(
                    look,
                    if connected { "Disconnect" } else { "Connect" },
                    Message::Connect(!connected)
                ),
            ]
            .spacing(12),
        )
        .padding(16)
        .style(move |_| container::Style {
            background: Some(p.surface.into()),
            border: Border {
                color: p.line,
                width: 1.0,
                radius: 18.0.into(),
            },
            ..container::Style::default()
        });
        let pages = Page::ALL.into_iter().fold(Column::new().spacing(3), |pages, page| {
            pages.push(ui::nav_item(
                look,
                page.icon(),
                page.title(),
                page == self.page,
                Message::Open(page),
            ))
        });
        let hint = text("Press ? for keys").size(12.5).color(p.dim);
        container(column![device, pages, space::vertical(), hint].spacing(22))
            .padding([18, 14])
            .width(268)
            .height(Length::Fill)
            .style(move |_| container::Style {
                background: Some(p.surface_low.into()),
                ..container::Style::default()
            })
            .into()
    }

    /// The list of keys, over a dimmed window. A key or a click closes it.
    fn help(&self) -> Element<'_, Message> {
        let look = self.look;
        let p = look.p;
        let rows = keys::HELP.iter().fold(Column::new().spacing(9), |rows, &(keys, what)| {
            rows.push(row![
                text(keys).size(14).font(look.medium()).color(p.fg).width(170),
                text(what).size(14).color(p.dim)
            ])
        });
        let list = ui::panel(look, column![ui::title(look, "Keys"), rows].spacing(18).into());
        let backdrop = container(container(list).width(560))
            .center(Length::Fill)
            .style(|_| container::Style {
                background: Some(Color { a: 0.6, ..Color::BLACK }.into()),
                ..container::Style::default()
            });
        opaque(mouse_area(backdrop).on_press(Message::CloseHelp))
    }
}

/// The choice `action` moves to from the one at `at`: `h` and `l` stop at the ends, Enter goes
/// round. With nothing chosen, any of them picks the first.
fn step<T: Copy>(choices: &[T], at: Option<usize>, action: Action) -> Option<T> {
    let to = match (action, at) {
        (_, None) => 0,
        (Action::Left, Some(at)) => at.checked_sub(1)?,
        (Action::Right, Some(at)) => at + 1,
        (Action::Activate, Some(at)) => (at + 1).checked_rem(choices.len())?,
        _ => return None,
    };
    choices.get(to).copied()
}

/// The level a [`NoiseControl::level`] names.
fn anc_level(name: &str) -> Option<AncLevel> {
    [AncLevel::Light, AncLevel::Medium, AncLevel::Deep]
        .into_iter()
        .find(|level| level.name() == name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::keyboard::key::Named;
    use std::fs;
    use std::path::Path;

    fn app() -> App {
        App::new(None, Font::DEFAULT)
    }

    /// Presses keys, given by name and separated by spaces: `j`, `G`, `Enter`, `Esc`.
    fn press(app: &mut App, keys: &str) {
        for name in keys.split_whitespace() {
            let (key, modifiers) = match name {
                "Enter" => (Key::Named(Named::Enter), Modifiers::empty()),
                "Esc" => (Key::Named(Named::Escape), Modifiers::empty()),
                c if c.chars().all(char::is_uppercase) => (Key::Character(c.into()), Modifiers::SHIFT),
                c => (Key::Character(c.into()), Modifiers::empty()),
            };
            let _ = app.update(Message::Key(key, modifiers));
        }
    }

    fn mode(app: &App) -> &str {
        &app.buds.status.noise_control.as_ref().unwrap().mode
    }

    fn switch(app: &App, key: &str) -> bool {
        app.buds.status.switches.iter().find(|s| s.key == key).unwrap().on
    }

    #[test]
    fn noise_cancelling_remembers_its_level() {
        let mut app = app();
        let _ = app.update(Message::Noise(NoiseMode::Anc(AncLevel::Light)));
        let _ = app.update(Message::Noise(NoiseMode::Transparency));
        assert_eq!(app.anc_level, AncLevel::Light);
        let now = app.buds.status.noise_control.as_ref().unwrap();
        assert_eq!((now.mode.as_str(), now.level.as_deref()), ("transparency", None));
        let _ = app.update(Message::Noise(NoiseMode::Anc(app.anc_level)));
        let now = app.buds.status.noise_control.as_ref().unwrap();
        assert_eq!((now.mode.as_str(), now.level.as_deref()), ("anc", Some("light")));
    }

    #[test]
    fn switches_change_by_key() {
        let mut app = app();
        let _ = app.update(Message::Switch("game", true));
        assert!(switch(&app, "game"));
    }

    #[test]
    fn the_overview_items_in_order() {
        let mut app = app();
        assert_eq!(
            app.items(),
            [
                Item::NoiseMode,
                Item::AncLevel,
                Item::Switch("game"),
                Item::Switch("spatial"),
                Item::Switch("bass-boost"),
                Item::Switch("wind"),
                Item::Switch("dual"),
                Item::Open(Page::Equalizer),
                Item::Open(Page::Gestures),
                Item::Open(Page::Earbuds),
            ]
        );
        let _ = app.update(Message::Noise(NoiseMode::Off));
        assert!(!app.items().contains(&Item::AncLevel));
    }

    #[test]
    fn the_first_key_only_shows_the_cursor() {
        let mut app = app();
        assert_eq!(app.focus(), None);
        press(&mut app, "l");
        assert_eq!(app.focus(), Some(Item::NoiseMode));
        assert_eq!(mode(&app), "anc");
    }

    #[test]
    fn h_and_l_step_through_choices_and_enter_goes_round() {
        let mut app = app();
        press(&mut app, "j l");
        assert_eq!(mode(&app), "transparency");
        press(&mut app, "l");
        assert_eq!(mode(&app), "transparency");
        press(&mut app, "h h h");
        assert_eq!(mode(&app), "off");
        press(&mut app, "Enter Enter");
        assert_eq!(mode(&app), "transparency");
        press(&mut app, "Enter");
        assert_eq!(mode(&app), "off");
        // On the strength: deep at first.
        press(&mut app, "l j");
        assert_eq!(app.focus(), Some(Item::AncLevel));
        press(&mut app, "h h h");
        assert_eq!(app.anc_level, AncLevel::Light);
        press(&mut app, "Enter");
        assert_eq!(app.anc_level, AncLevel::Medium);
        assert_eq!(
            app.buds.status.noise_control.as_ref().unwrap().level.as_deref(),
            Some("medium")
        );
    }

    #[test]
    fn switches_take_h_l_and_enter() {
        let mut app = app();
        press(&mut app, "j j j");
        assert_eq!(app.focus(), Some(Item::Switch("game")));
        press(&mut app, "l");
        assert!(switch(&app, "game"));
        press(&mut app, "l");
        assert!(switch(&app, "game"));
        press(&mut app, "h");
        assert!(!switch(&app, "game"));
        press(&mut app, "Enter");
        assert!(switch(&app, "game"));
    }

    #[test]
    fn moving_stops_at_the_ends() {
        let mut app = app();
        press(&mut app, "k k");
        assert_eq!(app.focus(), Some(Item::NoiseMode));
        press(&mut app, "G j j");
        assert_eq!(app.focus(), Some(Item::Open(Page::Earbuds)));
        press(&mut app, "g g");
        assert_eq!(app.focus(), Some(Item::NoiseMode));
    }

    #[test]
    fn rows_open_pages_and_the_cursor_starts_over() {
        let mut app = app();
        press(&mut app, "G G h");
        assert_eq!(app.page, Page::Overview);
        press(&mut app, "l");
        assert_eq!(app.page, Page::Earbuds);
        assert_eq!(app.cursor, 0);
        assert!(app.cursor_shown);
    }

    #[test]
    fn pages_by_key() {
        let mut app = app();
        press(&mut app, "J");
        assert_eq!(app.page, Page::Equalizer);
        press(&mut app, "K K");
        assert_eq!(app.page, Page::Settings);
        press(&mut app, "1");
        assert_eq!(app.page, Page::Overview);
        press(&mut app, "g t");
        assert_eq!(app.page, Page::Equalizer);
        press(&mut app, "g T");
        assert_eq!(app.page, Page::Overview);
        press(&mut app, "9");
        assert_eq!(app.page, Page::Overview);
    }

    #[test]
    fn esc_and_clicks_hide_the_cursor() {
        let mut app = app();
        press(&mut app, "j j");
        assert!(app.focus().is_some());
        press(&mut app, "Esc");
        assert_eq!(app.focus(), None);
        press(&mut app, "j");
        let _ = app.update(Message::Pointer);
        assert_eq!(app.focus(), None);
        // It comes back where it was.
        press(&mut app, "j");
        assert_eq!(app.focus(), Some(Item::AncLevel));
    }

    #[test]
    fn the_list_of_keys_closes_on_any_key() {
        let mut app = app();
        press(&mut app, "?");
        assert!(app.help);
        press(&mut app, "j");
        assert!(!app.help);
        assert_eq!(app.focus(), None);
        press(&mut app, "?");
        let _ = app.update(Message::CloseHelp);
        assert!(!app.help);
    }

    #[test]
    fn steps() {
        let abc = ['a', 'b', 'c'];
        assert_eq!(step(&abc, Some(0), Action::Left), None);
        assert_eq!(step(&abc, Some(0), Action::Right), Some('b'));
        assert_eq!(step(&abc, Some(2), Action::Right), None);
        assert_eq!(step(&abc, Some(2), Action::Activate), Some('a'));
        assert_eq!(step(&abc, None, Action::Left), Some('a'));
    }

    #[test]
    fn arguments() {
        let args = |list: &[&str]| parse_args(list.iter().map(|s| (*s).to_owned()));
        assert!(matches!(args(&[]), Ok(Args::Run { theme: None })));
        assert!(matches!(args(&["--theme", "x.json"]), Ok(Args::Run { theme: Some(_) })));
        assert!(matches!(args(&["--help"]), Ok(Args::Help)));
        assert!(args(&["--theme"]).is_err());
        assert!(args(&["--bogus"]).is_err());
    }

    /// Renders every page to `target/gui-snapshots/`, so the look can be checked without a
    /// display. `BUDCTL_THEME=<file>` renders with that palette.
    #[test]
    fn pages_render() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/gui-snapshots");
        let _ = fs::remove_dir_all(&dir);
        let font = theme::system_font();
        let palette = std::env::var_os("BUDCTL_THEME").map(PathBuf::from);
        let on = |page: Page, keys: &str| {
            let mut app = App::new(palette.clone(), font);
            app.page = page;
            press(&mut app, keys);
            app
        };
        let mut shots: Vec<(String, f32, App)> = Page::ALL
            .into_iter()
            .map(|page| (format!("{page:?}").to_lowercase(), 800.0, on(page, "")))
            .collect();
        shots.push(("overview-full".to_owned(), 1240.0, on(Page::Overview, "")));
        shots.push(("overview-cursor".to_owned(), 800.0, on(Page::Overview, "j j j j")));
        shots.push(("keys".to_owned(), 800.0, on(Page::Overview, "?")));
        for (name, height, app) in shots {
            let settings = iced::Settings {
                default_font: font,
                ..iced::Settings::default()
            };
            let mut ui = iced_test::Simulator::with_size(settings, Size::new(1040.0, height), app.view());
            let snapshot = ui.snapshot(&app.theme).unwrap();
            assert!(snapshot.matches_image(dir.join(name)).unwrap());
        }
    }
}
