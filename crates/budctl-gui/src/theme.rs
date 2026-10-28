//! Colors and fonts: Nothing OS's shapes and type, in Material You colors.
//!
//! The colors are a built-in palette (near-black, white "on" states, one red for live things), or
//! one read from a matugen JSON file. The file uses the keys of the owner's Aurora palette
//! (`~/.cache/matugen/aurora.json`: Material 3 roles, `on_x` spelled `xFg`), so that file works as
//! it is, and the roles map as in Aurora's `Theme.qml`. A key the file lacks keeps its built-in
//! color. The fonts ship inside the binary (`fonts/`, OFL).

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use iced::font::{Family, Weight};
use iced::{Color, Font};

/// The colors the UI uses, by role: a ladder of surfaces, and the "on" states on top.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Palette {
    /// Window background (`surfaceLowest`).
    pub bg: Color,
    /// Cards and dialogs (`surfaceLow`).
    pub s1: Color,
    /// Icon circles, tracks, inputs, the switches' and pills' "off" fill (`surfaceHigh`).
    pub s2: Color,
    /// Unlit dots, hovered rows (`surfaceHighest`).
    pub s3: Color,
    /// Inactive things: the pages not open, an off switch's knob (`outlineVariant`).
    pub mute: Color,
    /// Caps labels (`outline`).
    pub grey: Color,
    /// Text (`surfaceFg`).
    pub fg: Color,
    /// Secondary text (`surfaceVariantFg`).
    pub fg_dim: Color,
    /// What is on or selected: switches, the chosen segment, lit dots, the cursor (`primary`).
    pub fill: Color,
    /// Text and icons on `fill` (`primaryFg`).
    pub fill_fg: Color,
    /// Tonal emphasis: text selection (`primaryContainer`).
    pub container: Color,
    pub container_fg: Color,
    /// Live things, one at a time: the open page's dot, "Saving…", ringing, charging (`tertiary`).
    pub live: Color,
    pub live_fg: Color,
    pub err: Color,
    pub err_fg: Color,
}

/// Near-black surfaces, white "on" states and a red for live things.
pub const BUILT_IN: Palette = Palette {
    bg: Color::from_rgb8(0x00, 0x00, 0x00),
    s1: Color::from_rgb8(0x11, 0x11, 0x12),
    s2: Color::from_rgb8(0x1C, 0x1C, 0x1E),
    s3: Color::from_rgb8(0x2A, 0x2A, 0x2D),
    mute: Color::from_rgb8(0x45, 0x45, 0x49),
    grey: Color::from_rgb8(0x8E, 0x8E, 0x93),
    fg: Color::from_rgb8(0xF2, 0xF2, 0xF2),
    fg_dim: Color::from_rgb8(0xB8, 0xB8, 0xBC),
    fill: Color::from_rgb8(0xF2, 0xF2, 0xF2),
    fill_fg: Color::from_rgb8(0x00, 0x00, 0x00),
    container: Color::from_rgb8(0x3A, 0x3A, 0x3E),
    container_fg: Color::from_rgb8(0xF2, 0xF2, 0xF2),
    live: Color::from_rgb8(0xD7, 0x19, 0x21),
    live_fg: Color::from_rgb8(0xFF, 0xFF, 0xFF),
    err: Color::from_rgb8(0xFF, 0xB4, 0xAB),
    err_fg: Color::from_rgb8(0x69, 0x00, 0x05),
};

/// Where in the palette a key's color goes.
type Role = fn(&mut Palette) -> &mut Color;

/// The palette file's keys, with each role's key and where it goes. `bg` and `fg` are the older
/// names of `surfaceLowest` and `surfaceFg`, still read. Other keys (`image`, `secondary`…) are
/// ignored.
const KEYS: [(&str, Option<&str>, Role); 16] = [
    ("surfaceLowest", Some("bg"), |p| &mut p.bg),
    ("surfaceLow", None, |p| &mut p.s1),
    ("surfaceHigh", None, |p| &mut p.s2),
    ("surfaceHighest", None, |p| &mut p.s3),
    ("outlineVariant", None, |p| &mut p.mute),
    ("outline", None, |p| &mut p.grey),
    ("surfaceFg", Some("fg"), |p| &mut p.fg),
    ("surfaceVariantFg", None, |p| &mut p.fg_dim),
    ("primary", None, |p| &mut p.fill),
    ("primaryFg", None, |p| &mut p.fill_fg),
    ("primaryContainer", None, |p| &mut p.container),
    ("primaryContainerFg", None, |p| &mut p.container_fg),
    ("tertiary", None, |p| &mut p.live),
    ("tertiaryFg", None, |p| &mut p.live_fg),
    ("err", None, |p| &mut p.err),
    ("errFg", None, |p| &mut p.err_fg),
];

/// A palette from the text of a palette file.
pub fn parse(json: &str) -> Result<Palette, String> {
    let file: HashMap<String, serde_json::Value> = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let mut palette = BUILT_IN;
    for (key, old, role) in KEYS {
        let found = [Some(key), old]
            .into_iter()
            .flatten()
            .find_map(|name| file.get(name).map(|value| (name, value)));
        let Some((name, value)) = found else {
            continue;
        };
        let color = value
            .as_str()
            .and_then(parse_hex)
            .ok_or_else(|| format!("{name}: {value} is not a #rrggbb color"))?;
        *role(&mut palette) = color;
    }
    Ok(palette)
}

/// `#rrggbb` or `#rrggbbaa`.
fn parse_hex(hex: &str) -> Option<Color> {
    let digits = hex.strip_prefix('#')?;
    let byte = |at: usize| u8::from_str_radix(digits.get(at..at + 2)?, 16).ok();
    match digits.len() {
        6 => Some(Color::from_rgb8(byte(0)?, byte(2)?, byte(4)?)),
        8 => Some(Color::from_rgba8(
            byte(0)?,
            byte(2)?,
            byte(4)?,
            f32::from(byte(6)?) / 255.0,
        )),
        _ => None,
    }
}

/// The palette file to follow: `--theme <file>`, else `~/.config/budctl/theme.json`.
pub fn default_file() -> Option<PathBuf> {
    budctl_core::config::dir().map(|dir| dir.join("theme.json"))
}

/// A palette as a palette file, with every key [`parse`] reads: the built-in one is written to
/// [`default_file`] when there is none, to start a matugen template from.
pub fn to_json(p: &Palette) -> String {
    let mut p = *p;
    let lines: Vec<String> = KEYS
        .iter()
        .map(|(key, _, role)| format!("  \"{key}\": \"{}\"", to_hex(*role(&mut p))))
        .collect();
    format!("{{\n{}\n}}\n", lines.join(",\n"))
}

/// `#rrggbb`, or `#rrggbbaa` when not opaque.
fn to_hex(color: Color) -> String {
    let [r, g, b, a] = color.into_rgba8();
    if a == u8::MAX {
        format!("#{r:02x}{g:02x}{b:02x}")
    } else {
        format!("#{r:02x}{g:02x}{b:02x}{a:02x}")
    }
}

/// The palette file and the version of it last read, so it is only read again when it changes.
pub struct Source {
    file: Option<PathBuf>,
    stamp: Option<SystemTime>,
}

impl Source {
    pub fn new(file: Option<PathBuf>) -> Self {
        Source { file, stamp: None }
    }

    /// The palette file followed, if any.
    pub fn file(&self) -> Option<&Path> {
        self.file.as_deref()
    }

    /// The palette, if the file changed since the last call: built-in when the file is gone, an
    /// error when it cannot be used. `None` when nothing changed.
    pub fn refresh(&mut self) -> Option<Result<Palette, String>> {
        let file = self.file.as_ref()?;
        let stamp = fs::metadata(file).and_then(|m| m.modified()).ok();
        if stamp == self.stamp {
            return None;
        }
        self.stamp = stamp;
        if stamp.is_none() {
            return Some(Ok(BUILT_IN));
        }
        Some(
            fs::read_to_string(file)
                .map_err(|e| e.to_string())
                .and_then(|json| parse(&json))
                .map_err(|e| format!("{}: {e}", file.display())),
        )
    }
}

/// The fonts the binary carries, to load at start.
pub const FONTS: [&[u8]; 5] = [
    include_bytes!("../fonts/SpaceGrotesk-Regular.otf"),
    include_bytes!("../fonts/SpaceGrotesk-Medium.otf"),
    include_bytes!("../fonts/SpaceGrotesk-SemiBold.otf"),
    include_bytes!("../fonts/SpaceMono-Regular.ttf"),
    include_bytes!("../fonts/DotoRounded-Black.ttf"),
];

/// UI text: titles semibold, body regular.
pub const UI: Font = Font::with_name("Space Grotesk");
/// Caps labels.
const MONO: Font = Font::with_name("Space Mono");
/// Numbers and short words, as round LED dots. Never under 26 px.
const DOT: Font = Font {
    family: Family::Name("Doto Rounded"),
    weight: Weight::Black,
    ..Font::DEFAULT
};

/// The palette and the fonts, handed to everything that draws.
#[derive(Debug, Clone, Copy)]
pub struct Look {
    pub p: Palette,
}

impl Look {
    pub fn medium(self) -> Font {
        Font {
            weight: Weight::Medium,
            ..UI
        }
    }

    pub fn semibold(self) -> Font {
        Font {
            weight: Weight::Semibold,
            ..UI
        }
    }

    pub fn mono(self) -> Font {
        MONO
    }

    pub fn dot(self) -> Font {
        DOT
    }
}

/// The iced theme for the widgets drawn with their default style.
pub fn iced_theme(p: &Palette) -> iced::Theme {
    iced::Theme::custom(
        "budctl",
        iced::theme::Palette {
            background: p.bg,
            text: p.fg,
            primary: p.fill,
            success: p.fill,
            warning: p.live,
            danger: p.err,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_aurora_keys() {
        let palette = parse(
            r##"{
                "image": "/some/wallpaper.jpg",
                "primary": "#d7c2be", "primaryFg": "#3b2d2b", "primaryContainer": "#52433f",
                "primaryContainerFg": "#f4ddd9", "secondary": "#d0c4c2",
                "tertiary": "#e0c38c", "tertiaryFg": "#3f2e04",
                "surfaceLowest": "#0f0e0e", "surfaceLow": "#1d1b1b", "surfaceContainer": "#211f1f",
                "surfaceHigh": "#2c2929", "surfaceHighest": "#373434", "surfaceFg": "#e7e1e0",
                "surfaceVariantFg": "#d2c3c1", "outline": "#9b8e8c", "outlineVariant": "#4e4443",
                "err": "#ffb4ab", "errFg": "#690005"
            }"##,
        )
        .unwrap();
        assert_eq!(palette.bg, Color::from_rgb8(0x0f, 0x0e, 0x0e));
        assert_eq!(palette.s1, Color::from_rgb8(0x1d, 0x1b, 0x1b));
        assert_eq!(palette.s3, Color::from_rgb8(0x37, 0x34, 0x34));
        assert_eq!(palette.fill, Color::from_rgb8(0xd7, 0xc2, 0xbe));
        assert_eq!(palette.live, Color::from_rgb8(0xe0, 0xc3, 0x8c));
        assert_eq!(palette.grey, Color::from_rgb8(0x9b, 0x8e, 0x8c));
        assert_eq!(palette.mute, Color::from_rgb8(0x4e, 0x44, 0x43));
        assert_eq!(palette.err_fg, Color::from_rgb8(0x69, 0x00, 0x05));
    }

    #[test]
    fn reads_the_older_names() {
        let palette = parse(r##"{"bg": "#0f0e0e", "fg": "#e7e1e0", "surface": "#211f1f"}"##).unwrap();
        assert_eq!(palette.bg, Color::from_rgb8(0x0f, 0x0e, 0x0e));
        assert_eq!(palette.fg, Color::from_rgb8(0xe7, 0xe1, 0xe0));
        // The newer name wins.
        let both = parse(r##"{"bg": "#000001", "surfaceLowest": "#000002"}"##).unwrap();
        assert_eq!(both.bg, Color::from_rgb8(0, 0, 2));
    }

    #[test]
    fn missing_keys_keep_the_built_in_colors() {
        let palette = parse(r##"{"primary": "#112233cc"}"##).unwrap();
        assert_eq!(palette.fill, Color::from_rgba8(0x11, 0x22, 0x33, 0.8));
        assert_eq!(palette.bg, BUILT_IN.bg);
        assert_eq!(parse("{}").unwrap(), BUILT_IN);
    }

    #[test]
    fn bad_files_are_errors() {
        assert!(parse("not json").is_err());
        assert!(parse(r#"{"bg": "red"}"#).unwrap_err().contains("bg"));
        assert!(parse(r#"{"primary": 3}"#).unwrap_err().contains("primary"));
        assert!(parse(r##"{"bg": "#12345"}"##).is_err());
        assert!(parse(r##"{"bg": "#ggggggg"}"##).is_err());
    }

    #[test]
    fn the_written_palette_reads_back_the_same() {
        assert_eq!(parse(&to_json(&BUILT_IN)).unwrap(), BUILT_IN);
        let see_through = Palette {
            fill: Color::from_rgba8(0x11, 0x22, 0x33, 0.8),
            ..BUILT_IN
        };
        assert_eq!(parse(&to_json(&see_through)).unwrap(), see_through);
    }

    #[test]
    fn a_missing_file_changes_nothing() {
        let mut source = Source::new(Some(PathBuf::from("/nonexistent/budctl/theme.json")));
        // No file now, and none read before: the built-in palette stays.
        assert_eq!(source.refresh(), None);
        assert_eq!(Source::new(None).refresh(), None);
    }
}
