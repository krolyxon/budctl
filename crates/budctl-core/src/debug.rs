//! `--debug` output: a hex dump of every byte sent and received, plus notes on what budctl
//! decoded and decided. Off by default. It goes to stderr, unless a front end sends it elsewhere
//! (the Android app, whose stderr goes nowhere).

use std::sync::RwLock;
use std::sync::atomic::{AtomicBool, Ordering};

static ENABLED: AtomicBool = AtomicBool::new(false);

/// Where debug lines go instead of stderr, once set.
type Output = Box<dyn Fn(&str) + Send + Sync>;
static OUTPUT: RwLock<Option<Output>> = RwLock::new(None);

pub fn enable() {
    ENABLED.store(true, Ordering::Relaxed);
}

pub fn disable() {
    ENABLED.store(false, Ordering::Relaxed);
}

pub fn enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

/// Send debug lines to `output` instead of stderr. A later call replaces an earlier one.
pub fn set_output(output: Output) {
    if let Ok(mut slot) = OUTPUT.write() {
        *slot = Some(output);
    }
}

/// Print one debug line.
pub(crate) fn line(text: &str) {
    if let Ok(slot) = OUTPUT.read()
        && let Some(output) = slot.as_ref()
    {
        output(text);
    } else {
        eprintln!("{text}");
    }
}

/// Print a note when debugging is on. The message is only built if it will be printed.
pub(crate) fn note(message: impl FnOnce() -> String) {
    if enabled() {
        line(&format!("#        {}", message()));
    }
}

/// `AA 07 00 …`
pub fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 3);
    for (i, byte) in bytes.iter().enumerate() {
        if i > 0 {
            out.push(' ');
        }
        out.push_str(&format!("{byte:02X}"));
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn hex_is_spaced_uppercase() {
        assert_eq!(super::hex(&[0xAA, 0x07, 0x00]), "AA 07 00");
        assert_eq!(super::hex(&[]), "");
    }
}
