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

#[test]
fn find_start_needs_yes_before_connecting() {
    let out = budctl(&["find", "start"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("--yes"));
}

#[test]
fn touch_changes_need_side_gesture_and_action() {
    for args in [
        &["touch", "left"][..],
        &["touch", "left", "hold"],
        &["touch", "--dry-run"],
        &["touch", "middle", "hold", "none"],
    ] {
        let out = budctl(args);
        assert_eq!(out.status.code(), Some(1), "{args:?}");
    }
}

#[test]
fn the_hold_cycle_needs_two_or_three_known_modes() {
    for args in [
        &["touch", "cycle"][..],
        &["touch", "cycle", "anc"],
        &["touch", "cycle", "anc", "deep"],
        &["touch", "cycle", "off", "transparency", "anc", "off"],
        &["touch", "cycle", "anc", "off", "--dry-run"],
    ] {
        let out = budctl(args);
        assert_eq!(out.status.code(), Some(1), "{args:?}");
    }
}

#[test]
fn bass_needs_a_level_from_minus_5_to_5() {
    // Out of range fails while parsing, before anything connects.
    for args in [
        &["bass"][..],
        &["bass", "loud"],
        &["bass", "300"],
        &["bass", "6"],
        &["bass", "-6"],
    ] {
        let out = budctl(args);
        assert_eq!(out.status.code(), Some(1), "{args:?}");
    }
}

#[test]
fn eq_bands_must_be_hz_equals_gain() {
    for args in [
        &["eq", "custom-4", "50"][..],
        &["eq", "custom-4", "fifty=3"],
        &["eq", "custom-4", "50=loud"],
        &["eq", "custom-4", "50=+3.5"],
        &["eq", "custom-4", "50=300"],
    ] {
        let out = budctl(args);
        assert_eq!(out.status.code(), Some(1), "{args:?}");
    }
}
