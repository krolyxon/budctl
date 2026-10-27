//! Colors and fonts, matched to the owner's shell (Aurora): its matugen palette, JetBrains Mono, and
//! its 12 px base size.
//!
//! The palette file uses Aurora's keys (`bg`, `fg`, `primary`…), so `~/.cache/matugen/aurora.json`
//! works as it is, and the keys map to the same roles as in Aurora's `Theme.qml`. A key the file
//! lacks keeps its built-in color.

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::SystemTime;

use iced::font::{Family, Weight};
use iced::{Color, Font};
use serde::Deserialize;

/// The colors the UI uses, by role, named as in Aurora.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Palette {
    /// Window background, and text on `accent`.
    pub bg: Color,
    /// Text.
    pub fg: Color,
    /// Secondary text: hints, and the labels of bars and groups.
    pub dim: Color,
    /// What is on or chosen: filled buttons and switches, section labels, bars.
    pub accent: Color,
    /// The sidebar, and hovered setting rows.
    pub surface: Color,
    /// Hovered buttons and rows, switches when off, bar tracks.
    pub surface_high: Color,
    /// Borders and separators.
    pub line: Color,
    pub err: Color,
}

/// Near-black, with a red accent.
pub const BUILT_IN: Palette = Palette {
    bg: Color::from_rgb8(0x0D, 0x0D, 0x0F),
    fg: Color::from_rgb8(0xE4, 0xE4, 0xE8),
    dim: Color::from_rgb8(0xA5, 0xA5, 0xAE),
    accent: Color::from_rgb8(0xD9, 0x3A, 0x3A),
    surface: Color::from_rgb8(0x19, 0x19, 0x1C),
    surface_high: Color::from_rgb8(0x24, 0x24, 0x28),
    line: Color::from_rgb8(0x3B, 0x3B, 0x42),
    err: Color::from_rgb8(0xFF, 0xB4, 0xAB),
};

/// The palette file's keys. Other keys (`image`, `secondary`…) are ignored.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PaletteFile {
    bg: Option<String>,
    fg: Option<String>,
    primary: Option<String>,
    surface: Option<String>,
    surface_high: Option<String>,
    outline_variant: Option<String>,
    surface_variant_fg: Option<String>,
    err: Option<String>,
}

/// A palette from the text of a palette file.
pub fn parse(json: &str) -> Result<Palette, String> {
    let file: PaletteFile = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let pick = |key: &str, value: Option<String>, default: Color| match value {
        None => Ok(default),
        Some(hex) => parse_hex(&hex).ok_or_else(|| format!("{key}: \"{hex}\" is not a #rrggbb color")),
    };
    let b = BUILT_IN;
    Ok(Palette {
        bg: pick("bg", file.bg, b.bg)?,
        fg: pick("fg", file.fg, b.fg)?,
        dim: pick("surfaceVariantFg", file.surface_variant_fg, b.dim)?,
        accent: pick("primary", file.primary, b.accent)?,
        surface: pick("surface", file.surface, b.surface)?,
        surface_high: pick("surfaceHigh", file.surface_high, b.surface_high)?,
        line: pick("outlineVariant", file.outline_variant, b.line)?,
        err: pick("err", file.err, b.err)?,
    })
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

/// The palette file to follow when `--theme` names none: `~/.config/budctl/theme.json`, else
/// Aurora's own palette. When neither exists yet, the first, so creating it later takes effect.
pub fn default_file() -> Option<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from)?;
    let xdg = |var: &str, fallback: &str| {
        std::env::var_os(var)
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .unwrap_or_else(|| home.join(fallback))
    };
    let ours = xdg("XDG_CONFIG_HOME", ".config").join("budctl/theme.json");
    let aurora = xdg("XDG_CACHE_HOME", ".cache").join("matugen/aurora.json");
    Some(if !ours.exists() && aurora.exists() {
        aurora
    } else {
        ours
    })
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

/// The palette and the font, handed to everything that draws.
#[derive(Debug, Clone, Copy)]
pub struct Look {
    pub p: Palette,
    pub font: Font,
    /// The base text size (Aurora's `fontSize`). Other sizes are relative to it.
    pub size: f32,
}

impl Look {
    pub fn new(p: Palette, font: Font) -> Self {
        Look { p, font, size: 12.0 }
    }

    /// The base size plus `delta` pixels.
    pub fn px(self, delta: f32) -> f32 {
        self.size + delta
    }

    pub fn bold(self) -> Font {
        Font {
            weight: Weight::Bold,
            ..self.font
        }
    }
}

/// Aurora's font. Its Nerd Font glyphs are the icons.
const FONT: &str = "JetBrainsMono Nerd Font";

/// [`FONT`] when it is installed, else the desktop's monospace font as fontconfig names it.
pub fn font() -> Font {
    let installed = Command::new("fc-list")
        .args(["-q", FONT])
        .status()
        .is_ok_and(|status| status.success());
    if installed {
        return Font::with_name(FONT);
    }
    let family = Command::new("fc-match")
        .args(["--format", "%{family[0]}", "monospace"])
        .output()
        .ok()
        .filter(|out| out.status.success())
        .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_owned())
        .filter(|name| !name.is_empty());
    Font {
        // Font names are &'static; this runs once.
        family: family.map_or(Family::Monospace, |name| Family::Name(Box::leak(name.into_boxed_str()))),
        ..Font::DEFAULT
    }
}

/// The iced theme, for what is drawn with iced's default styles (the scrollbar).
pub fn iced_theme(p: &Palette) -> iced::Theme {
    iced::Theme::custom(
        "budctl",
        iced::theme::Palette {
            background: p.bg,
            text: p.fg,
            primary: p.accent,
            success: p.accent,
            warning: p.accent,
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
                "bg": "#0f0c0a", "fg": "#ebe0dc",
                "primary": "#7b4b2f", "primaryFg": "#fffaff", "primaryContainer": "#5c3823",
                "secondary": "#e2bfae",
                "surfaceLow": "#181311", "surface": "#1e1815", "surfaceHigh": "#2a211d",
                "surfaceHighest": "#362a26", "outline": "#9f8d85", "outlineVariant": "#52443d",
                "surfaceVariantFg": "#d6c3b9", "err": "#ffb4ab"
            }"##,
        )
        .unwrap();
        assert_eq!(palette.bg, Color::from_rgb8(0x0f, 0x0c, 0x0a));
        assert_eq!(palette.accent, Color::from_rgb8(0x7b, 0x4b, 0x2f));
        assert_eq!(palette.surface_high, Color::from_rgb8(0x2a, 0x21, 0x1d));
        assert_eq!(palette.line, Color::from_rgb8(0x52, 0x44, 0x3d));
        assert_eq!(palette.dim, Color::from_rgb8(0xd6, 0xc3, 0xb9));
    }

    #[test]
    fn missing_keys_keep_the_built_in_colors() {
        let palette = parse(r##"{"primary": "#112233cc"}"##).unwrap();
        assert_eq!(palette.accent, Color::from_rgba8(0x11, 0x22, 0x33, 0.8));
        assert_eq!(palette.bg, BUILT_IN.bg);
        assert_eq!(parse("{}").unwrap(), BUILT_IN);
    }

    #[test]
    fn bad_files_are_errors() {
        assert!(parse("not json").is_err());
        assert!(parse(r#"{"bg": "red"}"#).unwrap_err().contains("bg"));
        assert!(parse(r##"{"bg": "#12345"}"##).is_err());
        assert!(parse(r##"{"bg": "#ggggggg"}"##).is_err());
    }

    #[test]
    fn a_missing_file_changes_nothing() {
        let mut source = Source::new(Some(PathBuf::from("/nonexistent/budctl/theme.json")));
        // No file now, and none read before: the built-in palette stays.
        assert_eq!(source.refresh(), None);
        assert_eq!(Source::new(None).refresh(), None);
    }
}
