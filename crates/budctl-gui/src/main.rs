//! `budctl-gui`: control wireless earbuds from Linux, in a window.
//!
//! A thin layer over budctl-core, like the CLI: `worker.rs` talks to the buds on a thread of its
//! own, and the window shows what they report. The mouse comes first; vim-like keys work on top
//! (`keys.rs`). `--demo` shows made-up buds instead.

mod demo;
mod icons;
mod keys;
mod overview;
mod theme;
mod ui;
mod worker;

use std::collections::VecDeque;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

use budctl_core::Kind;
use budctl_core::bt::BdAddr;
use budctl_core::status::{AncLevel, EqPresets, NoiseControl, NoiseMode, Switch};
use iced::keyboard::{self, Key, Modifiers};
use iced::widget::operation::{self, RelativeOffset};
use iced::widget::{Column, column, container, mouse_area, opaque, row, scrollable, space, stack, text};
use iced::{Alignment, Border, Color, Element, Font, Length, Size, Subscription, Task, Theme, event, mouse, window};

use icons::Icon;
use keys::Action;
use theme::Look;
use worker::{Command, Event, Reading};

const USAGE: &str = "usage: budctl-gui [--device <MAC>] [--theme <palette.json>] [--demo] [--debug]

--device picks the buds when more than one supported pair is connected.
--demo shows made-up buds, without Bluetooth.
--debug hex-dumps every frame sent and received on stderr, as budctl --debug does.
Colors come from --theme, else from ~/.config/budctl/theme.json if it exists, else built in.
The palette file takes the keys of a matugen palette like Aurora's (bg, fg, primary, …).
Press ? in the window for the keys.";

/// The page's scrolling area, so the cursor can be kept in view.
const SCROLL: &str = "page";

/// When the window gets focus and the last reading is older than this, the buds are read again:
/// the phone may have changed something meanwhile.
const STALE: Duration = Duration::from_secs(30);

fn main() -> ExitCode {
    let options = match parse_args(std::env::args().skip(1)) {
        Ok(Args::Run(options)) => options,
        Ok(Args::Help) => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Err(message) => {
            eprintln!("budctl-gui: {message}\n{USAGE}");
            return ExitCode::from(1);
        }
    };
    if options.debug {
        budctl_core::debug::enable();
    }
    let theme_file = options.theme.clone().or_else(theme::default_file);
    let font = theme::system_font();
    let boot = move || App::new(theme_file.clone(), font, options.device, options.demo);
    let result = iced::application(boot, App::update, App::view)
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

#[derive(Default)]
struct Options {
    theme: Option<PathBuf>,
    device: Option<BdAddr>,
    demo: bool,
    debug: bool,
}

enum Args {
    Run(Options),
    Help,
}

fn parse_args(mut args: impl Iterator<Item = String>) -> Result<Args, String> {
    let mut options = Options::default();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--theme" => options.theme = Some(PathBuf::from(args.next().ok_or("--theme needs a file")?)),
            "--device" => {
                let address = args.next().ok_or("--device needs an address")?;
                options.device = Some(address.parse().map_err(|e| format!("--device: {e}"))?);
            }
            "--demo" => options.demo = true,
            "--debug" => options.debug = true,
            "-h" | "--help" => return Ok(Args::Help),
            other => return Err(format!("unknown argument {other:?}")),
        }
    }
    Ok(Args::Run(options))
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

/// What the keyboard cursor can rest on, and what a write changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Item {
    NoiseMode,
    AncLevel,
    /// A switch, by its key.
    Switch(&'static str),
    /// A row that opens a page.
    Open(Page),
}

/// Where the connection to the buds stands.
#[derive(Debug, Clone, PartialEq)]
enum Link {
    /// Made-up buds (`--demo`): nothing talks to Bluetooth.
    Demo,
    /// Looking for the buds, or reading them for the first time.
    Connecting,
    /// The buds answered the last command.
    Live,
    /// The last command failed.
    Failed { kind: Kind, message: String },
}

#[derive(Debug, Clone)]
enum Message {
    Open(Page),
    Noise(NoiseMode),
    /// A switch's key, and whether to turn it on.
    Switch(&'static str, bool),
    /// Read the buds again.
    Refresh,
    Worker(Event),
    /// The window got focus: the palette file may have changed meanwhile, and so may the buds.
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
    /// `--device`, for the worker.
    device: Option<BdAddr>,
    link: Link,
    /// Where commands go, once the worker is up.
    worker: Option<Sender<Command>>,
    /// The commands sent and not answered yet, oldest first: the item each one writes, or `None`
    /// for a read. The worker answers them in order.
    in_flight: VecDeque<Option<Item>>,
    /// When the buds were last read.
    read_at: Option<Instant>,
}

impl App {
    fn new(theme_file: Option<PathBuf>, font: Font, device: Option<BdAddr>, demo: bool) -> Self {
        let buds = if demo { demo::buds() } else { demo::unknown() };
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
            device,
            link: if demo { Link::Demo } else { Link::Connecting },
            worker: None,
            in_flight: VecDeque::new(),
            read_at: None,
        };
        app.refresh_palette();
        app
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Open(page) => return self.open(page),
            Message::Noise(mode) => self.write(Item::NoiseMode, Command::Noise(mode)),
            Message::Switch(key, on) => self.write(Item::Switch(key), Command::Switch(key, on)),
            Message::Refresh => self.refresh(),
            Message::Worker(event) => self.answer(event),
            Message::Focused => {
                self.refresh_palette();
                if self.read_at.is_some_and(|at| at.elapsed() >= STALE) && self.in_flight.is_empty() {
                    self.refresh();
                }
            }
            Message::Key(key, modifiers) => return self.key(&key, modifiers),
            Message::Pointer => self.cursor_shown = false,
            Message::CloseHelp => self.help = false,
        }
        Task::none()
    }

    /// Changes a setting: at once with the made-up buds, else through the worker. Until the buds
    /// answer, the item shows that it is saving and takes no other write.
    fn write(&mut self, item: Item, command: Command) {
        if self.link == Link::Demo {
            match command {
                Command::Noise(mode) => self.apply_noise(NoiseControl {
                    mode: mode.name().to_owned(),
                    level: match mode {
                        NoiseMode::Anc(level) => Some(level.name().to_owned()),
                        NoiseMode::Off | NoiseMode::Transparency => None,
                    },
                }),
                Command::Switch(key, on) => {
                    if let Some(switch) = self.buds.status.switches.iter().find(|s| s.key == key) {
                        let switch = Switch { on, ..switch.clone() };
                        self.apply_switch(switch);
                    }
                }
                Command::Read => {}
            }
        } else if !self.pending().contains(&item) {
            self.send(command, Some(item));
        }
    }

    /// Reads the buds again; with the made-up buds, starts them over.
    fn refresh(&mut self) {
        match self.link {
            Link::Demo => self.buds = demo::buds(),
            _ if self.in_flight.contains(&None) => {}
            _ => {
                if self.read_at.is_none() {
                    self.link = Link::Connecting;
                }
                self.send(Command::Read, None);
            }
        }
    }

    fn send(&mut self, command: Command, item: Option<Item>) {
        let Some(worker) = &self.worker else {
            return;
        };
        if worker.send(command).is_ok() {
            self.in_flight.push_back(item);
        }
    }

    /// Takes in what the worker reports.
    fn answer(&mut self, event: Event) {
        match event {
            Event::Ready(worker) => {
                self.worker = Some(worker);
                self.send(Command::Read, None);
            }
            Event::Found(name) => self.buds.name = name,
            Event::Lost(message) => {
                self.buds.connected = false;
                self.link = Link::Failed {
                    kind: Kind::NotFound,
                    message,
                };
            }
            Event::Read(reading) => {
                self.in_flight.pop_front();
                self.apply_reading(*reading);
            }
            Event::Noise(now) => {
                self.in_flight.pop_front();
                self.apply_noise(now);
                self.link = Link::Live;
            }
            Event::Switch(switch) => {
                self.in_flight.pop_front();
                self.apply_switch(switch);
                self.link = Link::Live;
            }
            Event::Failed { kind, message } => {
                self.in_flight.pop_front();
                if kind == Kind::NotFound {
                    self.buds.connected = false;
                }
                self.link = Link::Failed { kind, message };
            }
        }
    }

    fn apply_reading(&mut self, reading: Reading) {
        self.buds.eq_name = eq_name(reading.status.eq.as_deref(), reading.eq.as_ref());
        if let Some(level) = reading
            .status
            .noise_control
            .as_ref()
            .and_then(|now| now.level.as_deref())
            .and_then(anc_level)
        {
            self.anc_level = level;
        }
        self.buds.status = reading.status;
        self.buds.connected = true;
        self.link = Link::Live;
        self.read_at = Some(Instant::now());
    }

    fn apply_noise(&mut self, now: NoiseControl) {
        if let Some(level) = now.level.as_deref().and_then(anc_level) {
            self.anc_level = level;
        }
        self.buds.status.noise_control = Some(now);
    }

    fn apply_switch(&mut self, switch: Switch) {
        if let Some(shown) = self.buds.status.switches.iter_mut().find(|s| s.key == switch.key) {
            *shown = switch;
        }
    }

    /// The items with a write on its way.
    fn pending(&self) -> Vec<Item> {
        self.in_flight.iter().flatten().copied().collect()
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
            Action::Refresh => {
                self.refresh();
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
            Page::Overview if self.has_reading() => overview::items(&self.buds),
            _ => Vec::new(),
        }
    }

    /// Whether there is anything to show: the made-up buds, or a reading of the real ones.
    fn has_reading(&self) -> bool {
        self.link == Link::Demo || self.read_at.is_some()
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
        let events = event::listen_with(|event, _status, _window| match event {
            iced::Event::Window(window::Event::Focused) => Some(Message::Focused),
            iced::Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) => {
                Some(Message::Key(key, modifiers))
            }
            iced::Event::Mouse(mouse::Event::ButtonPressed(_)) => Some(Message::Pointer),
            _ => None,
        });
        if self.link == Link::Demo {
            return events;
        }
        let worker = Subscription::run_with(self.device, worker::run).map(Message::Worker);
        Subscription::batch([events, worker])
    }

    fn view(&self) -> Element<'_, Message> {
        let look = self.look;
        let pending = self.pending();
        let page = match self.page {
            Page::Overview if self.has_reading() => {
                let marks = overview::Marks {
                    focus: self.focus(),
                    pending: &pending,
                };
                overview::view(look, &self.buds, self.anc_level, marks)
            }
            Page::Overview => self.connecting(),
            page => column![
                ui::title(look, page.title()),
                text("This page comes in a later step.").size(14).color(look.p.dim)
            ]
            .spacing(12)
            .into(),
        };
        let page = column![self.problem(), page].spacing(18);
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

    /// The Overview before the first reading: connecting, or why it did not work.
    fn connecting(&self) -> Element<'_, Message> {
        let look = self.look;
        let p = look.p;
        let (headline, detail) = match &self.link {
            Link::Failed {
                kind: Kind::NotFound,
                message,
            } => ("Your buds are not connected", message.as_str()),
            Link::Failed {
                kind: Kind::Busy,
                message,
            } => ("The control channel is busy", message.as_str()),
            Link::Failed { message, .. } => ("Something went wrong", message.as_str()),
            _ => (
                "Connecting to your buds…",
                "budctl looks for the paired buds it supports and reads them.",
            ),
        };
        let retry = matches!(self.link, Link::Failed { .. })
            .then(|| container(ui::outline_button(look, "Try again", Message::Refresh)).width(180));
        let words = column![
            text(headline).size(20).font(look.bold()).color(p.fg),
            text(detail).size(14).color(p.dim)
        ]
        .spacing(8);
        column![
            ui::title(look, "Overview"),
            ui::panel(look, column![words].push(retry).spacing(18).into())
        ]
        .spacing(18)
        .into()
    }

    /// A failure after the buds were read once: what went wrong, above the page.
    fn problem(&self) -> Option<Element<'_, Message>> {
        let Link::Failed { message, .. } = &self.link else {
            return None;
        };
        // Before the first reading, the Overview says it already.
        self.read_at?;
        let look = self.look;
        let p = look.p;
        let content = row![
            text(message.as_str()).size(14).color(p.err).width(Length::Fill),
            container(ui::outline_button(look, "Try again", Message::Refresh)).width(140)
        ]
        .spacing(16)
        .align_y(Alignment::Center);
        Some(
            container(content)
                .padding([12, 16])
                .width(Length::Fill)
                .style(move |_| container::Style {
                    background: Some(p.surface.into()),
                    border: Border {
                        color: p.err,
                        width: 1.0,
                        radius: 14.0.into(),
                    },
                    ..container::Style::default()
                })
                .into(),
        )
    }

    fn sidebar(&self) -> Element<'_, Message> {
        let look = self.look;
        let p = look.p;
        let (light, state) = match &self.link {
            Link::Demo | Link::Live => (p.ok, "Connected"),
            Link::Connecting => (p.dim, "Connecting…"),
            Link::Failed { kind: Kind::Busy, .. } => (p.err, "Busy"),
            Link::Failed { .. } if !self.buds.connected => (p.dim, "Not connected"),
            Link::Failed { .. } => (p.err, "Error"),
        };
        let device = container(
            column![
                text(self.buds.name.as_str()).size(16).font(look.bold()).color(p.fg),
                row![ui::light(light), text(state).size(13).color(p.dim), space::horizontal()]
                    .push((self.link == Link::Demo).then(|| ui::tag(look, "demo")))
                    .spacing(8)
                    .align_y(Alignment::Center),
                ui::outline_button(look, "Refresh", Message::Refresh),
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

/// The name to show for the EQ preset in use: a custom preset's own name (`custom-4` →
/// "Custom1"), else the preset's name in words (`pure-bass` → "Pure bass").
fn eq_name(current: Option<&str>, presets: Option<&EqPresets>) -> String {
    let Some(current) = current else {
        return "Unknown".to_owned();
    };
    let custom = current
        .strip_prefix("custom-")
        .and_then(|id| id.parse::<u8>().ok())
        .and_then(|id| presets?.presets.iter().find(|p| p.custom && p.id == id))
        .map(|preset| preset.name.clone());
    custom.unwrap_or_else(|| {
        let words = current.replace('-', " ");
        let mut letters = words.chars();
        letters
            .next()
            .map(|first| first.to_uppercase().chain(letters).collect())
            .unwrap_or_default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use budctl_core::status::EqPreset;
    use iced::keyboard::key::Named;
    use std::fs;
    use std::path::Path;
    use std::sync::mpsc::{self, Receiver};

    /// With the made-up buds.
    fn app() -> App {
        App::new(None, Font::DEFAULT, None, true)
    }

    /// Talking to a worker that the test plays: the receiver gets what the app sends it.
    fn live() -> (App, Receiver<Command>) {
        let mut app = App::new(None, Font::DEFAULT, None, false);
        let (worker, commands) = mpsc::channel();
        let _ = app.update(Message::Worker(Event::Ready(worker)));
        (app, commands)
    }

    fn reading() -> Reading {
        Reading {
            status: demo::buds().status,
            eq: None,
        }
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
    fn live_buds_are_read_first() {
        let (mut app, commands) = live();
        assert!(matches!(commands.try_recv(), Ok(Command::Read)));
        assert_eq!(app.link, Link::Connecting);
        assert!(app.items().is_empty(), "nothing to move over before the first reading");
        // A second refresh while the read is on its way sends nothing.
        press(&mut app, "r");
        assert!(commands.try_recv().is_err());

        let _ = app.update(Message::Worker(Event::Found("realme Buds T310".to_owned())));
        let _ = app.update(Message::Worker(Event::Read(Box::new(reading()))));
        assert_eq!(app.link, Link::Live);
        assert_eq!(app.buds.name, "realme Buds T310");
        assert!(app.buds.connected);
        assert_eq!(app.items().len(), 10);
    }

    #[test]
    fn live_writes_wait_for_the_buds() {
        let (mut app, commands) = live();
        let _ = commands.try_recv();
        let _ = app.update(Message::Worker(Event::Read(Box::new(reading()))));

        let _ = app.update(Message::Switch("game", true));
        assert!(matches!(commands.try_recv(), Ok(Command::Switch("game", true))));
        assert!(!switch(&app, "game"), "the switch changes when the buds say so");
        assert_eq!(app.pending(), [Item::Switch("game")]);
        // While it is on its way, the same switch takes no other write.
        press(&mut app, "j j j Enter");
        assert!(commands.try_recv().is_err());

        let game = Switch {
            key: "game",
            label: "Game mode",
            on: true,
        };
        let _ = app.update(Message::Worker(Event::Switch(game)));
        assert!(switch(&app, "game"));
        assert!(app.pending().is_empty());

        let _ = app.update(Message::Noise(NoiseMode::Transparency));
        assert!(matches!(
            commands.try_recv(),
            Ok(Command::Noise(NoiseMode::Transparency))
        ));
        assert_eq!(mode(&app), "anc");
        let now = NoiseControl {
            mode: "transparency".to_owned(),
            level: None,
        };
        let _ = app.update(Message::Worker(Event::Noise(now)));
        assert_eq!(mode(&app), "transparency");
    }

    #[test]
    fn failures_say_why_and_clear_the_write() {
        let (mut app, commands) = live();
        let _ = app.update(Message::Worker(Event::Failed {
            kind: Kind::Busy,
            message: "control channel busy".to_owned(),
        }));
        assert!(matches!(app.link, Link::Failed { kind: Kind::Busy, .. }));
        assert!(app.problem().is_none(), "the Overview says it, not a banner");

        let _ = commands.try_recv();
        let _ = app.update(Message::Refresh);
        assert!(matches!(commands.try_recv(), Ok(Command::Read)));
        assert_eq!(app.link, Link::Connecting);
        let _ = app.update(Message::Worker(Event::Read(Box::new(reading()))));

        let _ = app.update(Message::Switch("dual", false));
        let _ = app.update(Message::Worker(Event::Failed {
            kind: Kind::NotFound,
            message: "the buds closed the control link".to_owned(),
        }));
        assert!(app.pending().is_empty());
        assert!(switch(&app, "dual"));
        assert!(!app.buds.connected);
        assert!(app.problem().is_some());
    }

    #[test]
    fn eq_names() {
        let presets = EqPresets {
            current: Some("custom-4".to_owned()),
            presets: vec![EqPreset {
                name: "Custom1".to_owned(),
                id: 4,
                custom: true,
                bands: Vec::new(),
            }],
            warnings: Vec::new(),
        };
        assert_eq!(eq_name(Some("custom-4"), Some(&presets)), "Custom1");
        assert_eq!(eq_name(Some("custom-4"), None), "Custom 4");
        assert_eq!(eq_name(Some("pure-bass"), Some(&presets)), "Pure bass");
        assert_eq!(eq_name(None, None), "Unknown");
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
        assert!(matches!(args(&[]), Ok(Args::Run(Options { demo: false, .. }))));
        assert!(matches!(args(&["--demo"]), Ok(Args::Run(Options { demo: true, .. }))));
        assert!(matches!(args(&["--debug"]), Ok(Args::Run(Options { debug: true, .. }))));
        assert!(matches!(
            args(&["--theme", "x.json"]),
            Ok(Args::Run(Options { theme: Some(_), .. }))
        ));
        assert!(matches!(
            args(&["--device", "88:0E:85:46:FE:5C"]),
            Ok(Args::Run(Options { device: Some(_), .. }))
        ));
        assert!(matches!(args(&["--help"]), Ok(Args::Help)));
        assert!(args(&["--theme"]).is_err());
        assert!(args(&["--device", "nope"]).is_err());
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
            let mut app = App::new(palette.clone(), font, None, true);
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
        let mut connecting = App::new(palette.clone(), font, None, false);
        connecting.link = Link::Connecting;
        shots.push(("connecting".to_owned(), 800.0, connecting));
        let mut busy = App::new(palette.clone(), font, None, false);
        busy.answer(Event::Failed {
            kind: Kind::Busy,
            message: "control channel busy: is another app connected?".to_owned(),
        });
        shots.push(("busy".to_owned(), 800.0, busy));
        let (mut saving, _commands) = live();
        saving.answer(Event::Read(Box::new(reading())));
        saving.answer(Event::Failed {
            kind: Kind::Failed,
            message: "the buds did not answer: no reply to command 0x0403".to_owned(),
        });
        let _ = saving.update(Message::Switch("spatial", false));
        let _ = saving.update(Message::Noise(NoiseMode::Off));
        shots.push(("saving".to_owned(), 1240.0, saving));
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
