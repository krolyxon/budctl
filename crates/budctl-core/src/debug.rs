//! `--debug` output on stderr: a hex dump of every byte sent and received, plus notes on what
//! budctl decoded and decided. Off by default.

use std::sync::atomic::{AtomicBool, Ordering};

static ENABLED: AtomicBool = AtomicBool::new(false);

pub fn enable() {
    ENABLED.store(true, Ordering::Relaxed);
}

pub fn enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

/// Print a note when debugging is on. The message is only built if it will be printed.
pub(crate) fn note(message: impl FnOnce() -> String) {
    if enabled() {
        eprintln!("#        {}", message());
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
