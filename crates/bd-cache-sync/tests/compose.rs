//! `compose` resolves the phases directory against the repository root (QA-008).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code: a failed unwrap is a test failure"
)]

use std::sync::Arc;

use bd_cache_sync::{ComposeError, compose};
use sc_beads::{Clock, SystemClock};

fn repo(toml: &str) -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    let beads = root.path().join(".beads");
    std::fs::create_dir_all(&beads).unwrap();
    std::fs::write(
        beads.join("metadata.json"),
        r#"{"dolt_database": "atm-dev", "project_id": "p-1"}"#,
    )
    .unwrap();
    std::fs::write(beads.join("bd-cache.toml"), toml).unwrap();
    root
}

/// The working directory of the test is the crate's, not the repository's:
/// a relative `phases_dir`, and the default, still name the repository's.
#[test]
fn a_relative_phases_dir_and_the_default_are_rooted_at_the_repository() {
    let clock: Arc<dyn Clock> = Arc::new(SystemClock);
    let root = repo("[tick]\nteam = \"t\"\nfallback_lead = \"f\"\nphases_dir = \"phases\"\n");
    let composed = compose(root.path(), "bd", Arc::clone(&clock)).unwrap();
    assert_eq!(composed.tick.phases_dir, Some(root.path().join("phases")));
    drop(composed);

    let root = repo("[tick]\nteam = \"t\"\nfallback_lead = \"f\"\n");
    let composed = compose(root.path(), "bd", Arc::clone(&clock)).unwrap();
    assert_eq!(composed.tick.phases_dir, Some(root.path().join(".atm-bd")));
    drop(composed);

    let abs = tempfile::tempdir().unwrap();
    let root = repo(&format!(
        "[tick]\nteam = \"t\"\nfallback_lead = \"f\"\nphases_dir = '{}'\n",
        abs.path().display()
    ));
    let composed = compose(root.path(), "bd", clock).unwrap();
    assert_eq!(composed.tick.phases_dir.as_deref(), Some(abs.path()));
}

/// Errors have a code and keep their cause as the source, not in the text.
#[test]
fn compose_errors_have_codes_and_sources() {
    let clock: Arc<dyn Clock> = Arc::new(SystemClock);
    let root = repo("surprise = 1\n");
    let err = compose(root.path(), "bd", Arc::clone(&clock)).unwrap_err();
    assert!(matches!(err, ComposeError::Config(_)));
    assert_eq!(err.code(), "SYNC_CONFIG_INVALID");
    assert!(std::error::Error::source(&err).is_some());
    assert!(!err.to_string().contains("surprise"));

    let empty = tempfile::tempdir().unwrap();
    let err = compose(empty.path(), "bd", clock).unwrap_err();
    assert_eq!(err.code(), "BD_CACHE.NO_DATABASE");
}
