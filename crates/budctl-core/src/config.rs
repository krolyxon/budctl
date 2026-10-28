//! The user's config: `~/.config/budctl/config.json`, shared by the CLI and the GUI.
//!
//! It only holds preferences. Settings live on the buds, so nothing here mirrors them (CLAUDE.md,
//! goals). A missing file is written with the defaults, so there is something to edit; a file
//! that cannot be used is reported and the defaults are used instead, so a typo never stops a
//! status bar's `budctl --json status`.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::bt::BdAddr;

/// What the file holds. A key the file lacks keeps its default; an unknown key is an error, so a
/// misspelt one is not silently ignored.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// The buds to use when `--device` is not given, as `AA:BB:CC:DD:EE:FF`.
    pub device: Option<String>,
    pub gui: Gui,
}

/// `budctl-gui` only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Gui {
    /// Seconds the GUI keeps the control channel open after the last command.
    pub idle_seconds: u64,
    /// The page the window opens on: `overview`, `equalizer`, `gestures`, `earbuds` or `settings`.
    pub start_page: String,
}

impl Default for Gui {
    fn default() -> Self {
        Gui {
            idle_seconds: 10,
            start_page: "overview".to_owned(),
        }
    }
}

/// The longest `gui.idle_seconds` may be: the channel is meant to be free for other apps.
const MAX_IDLE_SECONDS: u64 = 600;

impl Config {
    /// The default device, checked.
    pub fn device(&self) -> Result<Option<BdAddr>, String> {
        self.device
            .as_deref()
            .map(|address| address.parse().map_err(|e| format!("device: {e}")))
            .transpose()
    }

    fn check(&self) -> Result<(), String> {
        self.device()?;
        if !(1..=MAX_IDLE_SECONDS).contains(&self.gui.idle_seconds) {
            return Err(format!("gui.idle_seconds: must be 1 to {MAX_IDLE_SECONDS}"));
        }
        Ok(())
    }
}

/// A config from the text of a config file.
pub fn parse(json: &str) -> Result<Config, String> {
    let config: Config = serde_json::from_str(json).map_err(|e| e.to_string())?;
    config.check()?;
    Ok(config)
}

/// budctl's directory under `$XDG_CONFIG_HOME`, else `~/.config`.
pub fn dir() -> Option<PathBuf> {
    xdg_dir("XDG_CONFIG_HOME", ".config")
}

/// budctl's directory under `$XDG_STATE_HOME`, else `~/.local/state`: logs.
pub fn state_dir() -> Option<PathBuf> {
    xdg_dir("XDG_STATE_HOME", ".local/state")
}

fn xdg_dir(variable: &str, fallback: &str) -> Option<PathBuf> {
    let base = std::env::var_os(variable)
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|home| Path::new(&home).join(fallback)))?;
    Some(base.join("budctl"))
}

/// `config.json` in [`dir`].
pub fn file() -> Option<PathBuf> {
    dir().map(|dir| dir.join("config.json"))
}

/// The config, and why the file could not be used if it could not (the defaults are used then).
/// A missing file is written with the defaults; failing to write it is not a problem.
pub fn load() -> (Config, Option<String>) {
    let Some(file) = file() else {
        return (Config::default(), None);
    };
    load_from(&file)
}

fn load_from(file: &Path) -> (Config, Option<String>) {
    match fs::read_to_string(file) {
        Ok(json) => match parse(&json) {
            Ok(config) => (config, None),
            Err(e) => (Config::default(), Some(format!("{}: {e}", file.display()))),
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let _ = create(file, &defaults_json());
            (Config::default(), None)
        }
        Err(e) => (Config::default(), Some(format!("{}: {e}", file.display()))),
    }
}

/// The default config, as written to a new file.
pub fn defaults_json() -> String {
    // A plain struct of strings and numbers always serializes.
    let mut json = serde_json::to_string_pretty(&Config::default()).unwrap_or_default();
    json.push('\n');
    json
}

/// Write `contents` to `file`, creating its directory, unless the file already exists.
pub fn create(file: &Path, contents: &str) -> std::io::Result<()> {
    if let Some(dir) = file.parent() {
        fs::create_dir_all(dir)?;
    }
    let mut new = fs::OpenOptions::new().write(true).create_new(true).open(file)?;
    std::io::Write::write_all(&mut new, contents.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_read_back_as_the_defaults() {
        assert_eq!(parse(&defaults_json()).unwrap(), Config::default());
        assert_eq!(parse("{}").unwrap(), Config::default());
    }

    #[test]
    fn keys_override_the_defaults() {
        let config = parse(r#"{"device": "AA:BB:CC:DD:EE:FF", "gui": {"idle_seconds": 30}}"#).unwrap();
        assert_eq!(config.device().unwrap(), Some("AA:BB:CC:DD:EE:FF".parse().unwrap()));
        assert_eq!(config.gui.idle_seconds, 30);
        assert_eq!(config.gui.start_page, "overview");
    }

    #[test]
    fn bad_files_are_errors() {
        assert!(parse("not json").is_err());
        assert!(
            parse(r#"{"devise": "AA:BB:CC:DD:EE:FF"}"#)
                .unwrap_err()
                .contains("devise")
        );
        assert!(parse(r#"{"device": "nope"}"#).unwrap_err().contains("device"));
        assert!(
            parse(r#"{"gui": {"idle_seconds": 0}}"#)
                .unwrap_err()
                .contains("idle_seconds")
        );
    }

    #[test]
    fn a_missing_file_is_created_and_an_existing_one_kept() {
        let dir = std::env::temp_dir().join(format!("budctl-config-test-{}", std::process::id()));
        let file = dir.join("nested/config.json");
        let _ = fs::remove_dir_all(&dir);

        assert_eq!(load_from(&file), (Config::default(), None));
        assert_eq!(fs::read_to_string(&file).unwrap(), defaults_json());

        fs::write(&file, r#"{"gui": {"start_page": "equalizer"}}"#).unwrap();
        assert_eq!(load_from(&file).0.gui.start_page, "equalizer");
        assert!(create(&file, "{}").is_err(), "an existing file is never overwritten");

        fs::write(&file, "{").unwrap();
        let (config, problem) = load_from(&file);
        assert_eq!(config, Config::default());
        assert!(problem.unwrap().contains("config.json"));
        let _ = fs::remove_dir_all(&dir);
    }
}
