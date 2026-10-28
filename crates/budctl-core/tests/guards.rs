//! Checks for the CLAUDE.md hard rules that the compiler and clippy cannot make on their own.
//!
//! - Rule 3: every `Spec: <file>.md#<anchor>` comment points at an anchor that exists.
//!
//! The CLI's exit codes are checked in crates/budctl-cli/tests/cli.rs.

// Test-only file: helpers may panic (clippy.toml exempts only #[test] functions themselves).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]

use std::fs;
use std::path::{Path, PathBuf};

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Every `.rs` file under `crates/*/src`.
fn sources() -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push(path);
            }
        }
    }
    let mut files = Vec::new();
    for krate in fs::read_dir(workspace().join("crates")).unwrap() {
        let src = krate.unwrap().path().join("src");
        if src.is_dir() {
            walk(&src, &mut files);
        }
    }
    assert!(!files.is_empty(), "found no sources to check");
    files
}

#[test]
fn spec_references_point_at_existing_anchors() {
    let docs = workspace().join("docs/protocol");
    let marker = "Spec: ";
    let mut checked = 0;
    let mut broken = Vec::new();
    for file in sources() {
        let text = fs::read_to_string(&file).unwrap();
        for (n, line) in text.lines().enumerate() {
            let mut rest = line;
            while let Some(at) = rest.find(marker) {
                rest = &rest[at + marker.len()..];
                let reference: String = rest
                    .chars()
                    .take_while(|c| c.is_ascii_alphanumeric() || "._-#".contains(*c))
                    .collect();
                let Some((doc, anchor)) = reference.split_once('#') else {
                    continue;
                };
                if !doc.ends_with(".md") {
                    continue;
                }
                checked += 1;
                let spec = fs::read_to_string(docs.join(doc)).unwrap_or_default();
                if !spec.contains(&format!("id=\"{anchor}\"")) {
                    broken.push(format!("{}:{}: {reference}", file.display(), n + 1));
                }
            }
        }
    }
    assert!(checked > 0, "found no Spec: references at all");
    assert!(
        broken.is_empty(),
        "Spec: references with no matching anchor:\n{}",
        broken.join("\n")
    );
}
