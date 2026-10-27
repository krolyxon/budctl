//! Exit codes that need no Bluetooth (CLAUDE.md, "Exit codes").

// Test-only file: helpers may panic (clippy.toml exempts only #[test] functions themselves).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]

use std::process::Command;

fn budctl(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_budctl")).args(args).output().unwrap()
}

#[test]
fn help_and_version_exit_0() {
    for flag in ["--help", "--version"] {
        let out = budctl(&[flag]);
        assert_eq!(out.status.code(), Some(0), "{flag}");
        assert!(!out.stdout.is_empty(), "{flag}");
    }
}

#[test]
fn usage_errors_exit_1() {
    for args in [&["frobnicate"][..], &[], &["--device", "not-a-mac", "status"]] {
        let out = budctl(args);
        assert_eq!(out.status.code(), Some(1), "{args:?}");
    }
}
