//! `~/.local/state/budctl/gui.log`: errors and session events (connected, each write and its
//! answer, closed), so a "Try again" can be explained after the fact even when the window was
//! started from a launcher and stderr went nowhere. Each launch starts the file afresh.
//!
//! Lines start with the seconds since launch, like `--debug` output. Frames are not logged;
//! `--debug` still hex-dumps them on stderr.

use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

struct Log {
    file: Mutex<File>,
    started: Instant,
}

static LOG: OnceLock<Log> = OnceLock::new();

/// `gui.log` in budctl's state directory.
pub fn file() -> Option<PathBuf> {
    budctl_core::config::state_dir().map(|dir| dir.join("gui.log"))
}

/// Start the log at `path`, replacing the last launch's. Until this runs, and if it fails,
/// [`line`] does nothing.
pub fn start(path: &Path) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let mut file = File::create(path)?;
    let since_epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default();
    writeln!(
        file,
        "budctl-gui {} started (unix time {since_epoch}, pid {})",
        env!("CARGO_PKG_VERSION"),
        std::process::id()
    )?;
    let _ = LOG.set(Log {
        file: Mutex::new(file),
        started: Instant::now(),
    });
    Ok(())
}

/// Add a line to the log, if it is open.
pub fn line(message: impl AsRef<str>) {
    let Some(log) = LOG.get() else {
        return;
    };
    let elapsed = log.started.elapsed().as_secs_f64();
    if let Ok(mut file) = log.file.lock() {
        // A log that cannot be written must not take the window down with it.
        let _ = writeln!(file, "{elapsed:>9.3} {}", message.as_ref());
    }
}
