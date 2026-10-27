//! `budctl-gui`: control wireless earbuds from Linux, in a window.
//!
//! A thin layer over budctl-core, like the CLI. For now it shows demo data: the look comes first,
//! the live connection later (CLAUDE.md, GUI).

mod demo;
mod icons;
mod overview;
mod theme;
mod ui;

use std::path::PathBuf;
use std::process::ExitCode;

use budctl_core::status::{AncLevel, NoiseControl, NoiseMode};
use iced::widget::{Column, column, container, row, scrollable, space, text};
use iced::{Alignment, Border, Element, Font, Length, Size, Subscription, Theme, event, window};

use icons::Icon;
use theme::Look;

const USAGE: &str = "usage: budctl-gui [--theme <palette.json>]

Colors come from --theme, else from ~/.config/budctl/theme.json if it exists, else built in.
The palette file takes the keys of a matugen palette like Aurora's (bg, fg, primary, …).";

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
}

struct App {
    page: Page,
    buds: demo::Buds,
    /// The level "Noise cancelling" turns on at: the one last reported or picked.
    anc_level: AncLevel,
    look: Look,
    theme: Theme,
    palette: theme::Source,
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
        };
        app.refresh_palette();
        app
    }

    fn update(&mut self, message: Message) {
        match message {
            Message::Open(page) => self.page = page,
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
        }
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
            _ => None,
        })
    }

    fn view(&self) -> Element<'_, Message> {
        let look = self.look;
        let page = match self.page {
            Page::Overview => overview::view(look, &self.buds, self.anc_level),
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
        row![self.sidebar(), ui::divider(look), content].into()
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
        container(column![device, pages].spacing(22))
            .padding([18, 14])
            .width(268)
            .height(Length::Fill)
            .style(move |_| container::Style {
                background: Some(p.surface_low.into()),
                ..container::Style::default()
            })
            .into()
    }
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
    use std::fs;
    use std::path::Path;

    #[test]
    fn noise_cancelling_remembers_its_level() {
        let mut app = App::new(None, Font::DEFAULT);
        app.update(Message::Noise(NoiseMode::Anc(AncLevel::Light)));
        app.update(Message::Noise(NoiseMode::Transparency));
        assert_eq!(app.anc_level, AncLevel::Light);
        let now = app.buds.status.noise_control.as_ref().unwrap();
        assert_eq!((now.mode.as_str(), now.level.as_deref()), ("transparency", None));
        app.update(Message::Noise(NoiseMode::Anc(app.anc_level)));
        let now = app.buds.status.noise_control.as_ref().unwrap();
        assert_eq!((now.mode.as_str(), now.level.as_deref()), ("anc", Some("light")));
    }

    #[test]
    fn switches_change_by_key() {
        let mut app = App::new(None, Font::DEFAULT);
        app.update(Message::Switch("game", true));
        let game = app.buds.status.switches.iter().find(|s| s.key == "game").unwrap();
        assert!(game.on);
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
        let shots = Page::ALL
            .into_iter()
            .map(|page| (page, 800.0, format!("{page:?}").to_lowercase()))
            .chain([(Page::Overview, 1240.0, "overview-full".to_owned())]);
        for (page, height, name) in shots {
            let mut app = App::new(palette.clone(), font);
            app.page = page;
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
