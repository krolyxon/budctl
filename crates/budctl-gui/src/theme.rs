//! Colors and fonts. The colors are a built-in dark palette, or one read from a matugen JSON file.
//!
//! The file uses the keys of the owner's Aurora palette (`bg`, `fg`, `primary`…), so
//! `~/.cache/matugen/aurora.json` works as it is, and the keys map to the same roles as in Aurora's
//! `Theme.qml`, plus `secondary` for icons. A key the file lacks keeps its built-in color.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;

use iced::font::Weight;
use iced::{Color, Font};
use serde::Deserialize;

/// The colors the UI uses, by role.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Palette {
    /// Window background.
    pub bg: Color,
    /// Text.
    pub fg: Color,
    /// Secondary text, and icons at rest.
    pub dim: Color,
    /// What is on or selected: switches, the noise-control pill, battery rings.
    pub accent: Color,
    /// Icons and small marks on cards and the sidebar. matugen's `primary` (Aurora's key) is a
    /// container color, too dark to draw thin lines with on a dark card.
    pub icon: Color,
    /// Text and icons on `accent`.
    pub accent_fg: Color,
    /// A quiet accent: the open page in the sidebar.
    pub accent_dim: Color,
    /// The sidebar.
    pub surface_low: Color,
    /// Cards.
    pub surface: Color,
    /// Hovered rows, and switches when off.
    pub surface_high: Color,
    /// Battery ring tracks.
    pub surface_highest: Color,
    /// Card borders and separators.
    pub line: Color,
    pub err: Color,
    /// "Connected". matugen has no such role, so it stays green.
    pub ok: Color,
}

/// Dark, with a red accent.
pub const BUILT_IN: Palette = Palette {
    bg: Color::from_rgb8(0x12, 0x12, 0x14),
    fg: Color::from_rgb8(0xEE, 0xEE, 0xF1),
    dim: Color::from_rgb8(0x9C, 0x9C, 0xA6),
    accent: Color::from_rgb8(0xE0, 0x30, 0x2E),
    icon: Color::from_rgb8(0xE0, 0x30, 0x2E),
    accent_fg: Color::from_rgb8(0xFF, 0xFF, 0xFF),
    accent_dim: Color::from_rgb8(0x3A, 0x1A, 0x1C),
    surface_low: Color::from_rgb8(0x17, 0x17, 0x1A),
    surface: Color::from_rgb8(0x1E, 0x1E, 0x22),
    surface_high: Color::from_rgb8(0x29, 0x29, 0x2E),
    surface_highest: Color::from_rgb8(0x33, 0x33, 0x39),
    line: Color::from_rgb8(0x2D, 0x2D, 0x33),
    err: Color::from_rgb8(0xFF, 0xB4, 0xAB),
    ok: Color::from_rgb8(0x4C, 0xC3, 0x6A),
};

/// The palette file's keys. Other keys (`image`, `secondary`…) are ignored.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PaletteFile {
    bg: Option<String>,
    fg: Option<String>,
    primary: Option<String>,
    primary_fg: Option<String>,
    primary_container: Option<String>,
    secondary: Option<String>,
    surface_low: Option<String>,
    surface: Option<String>,
    surface_high: Option<String>,
    surface_highest: Option<String>,
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
        icon: pick("secondary", file.secondary, b.icon)?,
        accent_fg: pick("primaryFg", file.primary_fg, b.accent_fg)?,
        accent_dim: pick("primaryContainer", file.primary_container, b.accent_dim)?,
        surface_low: pick("surfaceLow", file.surface_low, b.surface_low)?,
        surface: pick("surface", file.surface, b.surface)?,
        surface_high: pick("surfaceHigh", file.surface_high, b.surface_high)?,
        surface_highest: pick("surfaceHighest", file.surface_highest, b.surface_highest)?,
        line: pick("outlineVariant", file.outline_variant, b.line)?,
        err: pick("err", file.err, b.err)?,
        ok: b.ok,
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

/// The palette file to follow: `--theme <file>`, else `~/.config/budctl/theme.json`.
pub fn default_file() -> Option<PathBuf> {
    budctl_core::config::dir().map(|dir| dir.join("theme.json"))
}

/// A palette as a palette file, with every key [`parse`] reads: the built-in one is written to
/// [`default_file`] when there is none, to start a matugen template from.
pub fn to_json(p: &Palette) -> String {
    let keys = [
        ("bg", p.bg),
        ("fg", p.fg),
        ("primary", p.accent),
        ("primaryFg", p.accent_fg),
        ("primaryContainer", p.accent_dim),
        ("secondary", p.icon),
        ("surfaceLow", p.surface_low),
        ("surface", p.surface),
        ("surfaceHigh", p.surface_high),
        ("surfaceHighest", p.surface_highest),
        ("outlineVariant", p.line),
        ("surfaceVariantFg", p.dim),
        ("err", p.err),
    ];
    let lines: Vec<String> = keys
        .iter()
        .map(|(key, color)| format!("  \"{key}\": \"{}\"", to_hex(*color)))
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

/// The palette and the fonts, handed to everything that draws.
#[derive(Debug, Clone, Copy)]
pub struct Look {
    pub p: Palette,
    pub font: Font,
}

impl Look {
    pub fn medium(self) -> Font {
        Font {
            weight: Weight::Medium,
            ..self.font
        }
    }

    pub fn bold(self) -> Font {
        Font {
            weight: Weight::Bold,
            ..self.font
        }
    }
}

/// The desktop's sans-serif font, as fontconfig names it. iced's own default is a font many
/// systems lack.
pub fn system_font() -> Font {
    let family = Command::new("fc-match")
        .args(["--format", "%{family[0]}", "sans-serif"])
        .output()
        .ok()
        .filter(|out| out.status.success())
        .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_owned())
        .filter(|name| !name.is_empty());
    match family {
        // Font names are &'static; this runs once.
        Some(name) => Font::with_name(Box::leak(name.into_boxed_str())),
        None => Font::DEFAULT,
    }
}

/// The iced theme for the widgets drawn with their default style.
pub fn iced_theme(p: &Palette) -> iced::Theme {
    iced::Theme::custom(
        "budctl",
        iced::theme::Palette {
            background: p.bg,
            text: p.fg,
            primary: p.accent,
            success: p.ok,
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
        assert_eq!(palette.accent_dim, Color::from_rgb8(0x5c, 0x38, 0x23));
        assert_eq!(palette.icon, Color::from_rgb8(0xe2, 0xbf, 0xae));
        assert_eq!(palette.line, Color::from_rgb8(0x52, 0x44, 0x3d));
        assert_eq!(palette.dim, Color::from_rgb8(0xd6, 0xc3, 0xb9));
        assert_eq!(palette.ok, BUILT_IN.ok);
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
    fn the_written_palette_reads_back_the_same() {
        assert_eq!(parse(&to_json(&BUILT_IN)).unwrap(), BUILT_IN);
        let see_through = Palette {
            accent: Color::from_rgba8(0x11, 0x22, 0x33, 0.8),
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
