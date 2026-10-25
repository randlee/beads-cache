//! btit's public API is kept unchanged (REQ-BDC-0906, ADR-BDC-0007).
//!
//! `tests/fixtures/btit-api.txt` holds the public lines of btit-types and
//! btit-beads at randlee/beads-task-issue-tracker `8d8e82ae9f85`, extracted
//! by the rule in its header. This test applies the same rule to the ported
//! files and requires every snapshot line to appear, in order: sc-beads may
//! add items (none were added to these files) but never change or drop one.
//! The ported `api_freeze` tests pin the same surface at compile time.
//!
//! The one recorded deviation is ADR-BDC-0007's move from the `log` facade to
//! `tracing`: `logging.rs` re-exports `tracing` where btit re-exported `log`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a failed unwrap or index is a test failure"
)]

use std::path::Path;

const SNAPSHOT: &str = include_str!("fixtures/btit-api.txt");

/// Snapshot lines replaced on purpose, with their replacement.
const DEVIATIONS: &[(&str, &str, &str)] = &[("logging.rs", "pub use log;", "pub use tracing;")];

fn extract(name: &str, src: &str) -> Vec<String> {
    let code = src.split("\n#[cfg(test)]").next().unwrap_or_default();
    code.lines()
        .map(str::trim)
        .filter(|t| t.starts_with("pub ") || (name == "backend.rs" && t.starts_with("fn ")))
        .map(String::from)
        .collect()
}

fn snapshot() -> Vec<(String, Vec<String>)> {
    let mut files: Vec<(String, Vec<String>)> = Vec::new();
    for line in SNAPSHOT.lines() {
        if let Some(name) = line.strip_prefix("## ") {
            files.push((name.to_string(), Vec::new()));
        } else if !line.is_empty() && !line.starts_with('#') {
            if let Some((_, lines)) = files.last_mut() {
                lines.push(line.to_string());
            }
        }
    }
    files
}

#[test]
fn btit_public_surface_is_unchanged() {
    let src_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let files = snapshot();
    assert_eq!(
        files.len(),
        13,
        "snapshot covers btit-types (5) and btit-beads (8) files"
    );
    for (name, expected) in files {
        let text = std::fs::read_to_string(src_dir.join(&name))
            .unwrap_or_else(|e| panic!("ported file {name} is missing: {e}"));
        let actual = extract(&name, &text);
        let mut at = actual.iter();
        for line in &expected {
            let want = DEVIATIONS
                .iter()
                .find(|(file, from, _)| name.ends_with(file) && line == from)
                .map_or(line.as_str(), |(_, _, to)| to);
            assert!(
                at.any(|l| l == want),
                "{name}: btit API line missing or reordered: {line}"
            );
        }
    }
}
