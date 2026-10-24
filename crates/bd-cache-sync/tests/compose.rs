//! `compose` resolves the phases directory against the repository root (QA-008),
//! and its errors carry this crate's stable codes (S34).
//!
//! Every case composes through [`compose_with`] with no `$BEADS_DIR` (lead
//! ruling S36, FTQ-028): the tests do not read the environment, so a
//! `BEADS_DIR` set on the machine cannot point them at a real cache.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code: a failed unwrap is a test failure"
)]

use std::path::Path;
use std::sync::Arc;

use bd_cache_store::ConfigLoadError;
use bd_cache_sync::{ComposeError, Composed, compose_with};
use sc_bd_cli::{BdCli, ProjectLocks};
use sc_beads::{Clock, SystemClock};

/// `compose` without the environment: a `bd` that is never run, and no
/// `$BEADS_DIR` (S36).
fn compose(root: &Path, clock: &Arc<dyn Clock>) -> Result<Composed, ComposeError> {
    let backend = BdCli::new_with_clock("bd", Arc::new(ProjectLocks::new()), Arc::clone(clock));
    compose_with(root, backend, Arc::clone(clock), None)
}

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
    let composed = compose(root.path(), &clock).unwrap();
    assert_eq!(composed.tick.phases_dir, Some(root.path().join("phases")));
    drop(composed);

    let root = repo("[tick]\nteam = \"t\"\nfallback_lead = \"f\"\n");
    let composed = compose(root.path(), &clock).unwrap();
    assert_eq!(composed.tick.phases_dir, Some(root.path().join(".atm-bd")));
    drop(composed);

    let abs = tempfile::tempdir().unwrap();
    let root = repo(&format!(
        "[tick]\nteam = \"t\"\nfallback_lead = \"f\"\nphases_dir = '{}'\n",
        abs.path().display()
    ));
    let composed = compose(root.path(), &clock).unwrap();
    assert_eq!(composed.tick.phases_dir.as_deref(), Some(abs.path()));
}

/// Errors have a code and keep their cause as the source, not in the text.
#[test]
fn compose_errors_have_codes_and_sources() {
    let clock: Arc<dyn Clock> = Arc::new(SystemClock);
    let root = repo("surprise = 1\n");
    let err = compose(root.path(), &clock).unwrap_err();
    let ComposeError::Config(loader) = &err else {
        panic!("{err:?}");
    };
    // This crate's stable code, mapped from the variant, not the store's own
    // code passed through (S34); the loader's fix, one text per way of
    // failing (RBP-F040).
    assert!(
        matches!(loader, ConfigLoadError::Parse { .. }),
        "{loader:?}"
    );
    assert_eq!(err.code(), "sync_config_invalid_toml");
    assert_eq!(loader.code(), "BD_CACHE.CONFIG_INVALID");
    assert_eq!(err.fix(), loader.fix());
    assert!(err.fix().contains("remove unknown keys"), "{}", err.fix());
    assert!(std::error::Error::source(&err).is_some());
    assert!(!err.to_string().contains("surprise"));

    let empty = tempfile::tempdir().unwrap();
    let err = compose(empty.path(), &clock).unwrap_err();
    assert_eq!(err.code(), "BD_CACHE.NO_DATABASE");
    // The cache error's own fix, not a generic text (RBP-F030).
    let ComposeError::Cache(inner) = &err else {
        panic!("{err:?}");
    };
    assert_eq!(err.fix(), inner.fix());

    // A scopes entry that is not a phase name has its own code and fix (RBP-F034).
    let root = repo("scopes = [\"not a phase\"]\n");
    let err = compose(root.path(), &clock).unwrap_err();
    assert_eq!(err.code(), "sync_config_bad_scope", "{err:?}");
    let ComposeError::Config(loader) = &err else {
        panic!("{err:?}");
    };
    assert_eq!(loader.code(), "BD_CACHE.CONFIG_BAD_SCOPE");
    assert_eq!(err.fix(), loader.fix());
    assert!(err.fix().contains("phase name"), "{}", err.fix());
    // It states the model's one phase-name rule (RBP-F037).
    assert!(
        err.fix().contains(bd_cache_model::PhaseName::RULE),
        "{}",
        err.fix()
    );

    // A file that cannot be read (a directory in its place).
    let root = repo("");
    let file = root.path().join(".beads").join("bd-cache.toml");
    std::fs::remove_file(&file).unwrap();
    std::fs::create_dir(&file).unwrap();
    let err = compose(root.path(), &clock).unwrap_err();
    assert_eq!(err.code(), "sync_config_unreadable", "{err:?}");
    let ComposeError::Config(loader) = &err else {
        panic!("{err:?}");
    };
    assert_eq!(loader.code(), "BD_CACHE.CONFIG_UNREADABLE");
    assert_eq!(err.fix(), loader.fix());
    assert!(err.fix().contains("readable"), "{}", err.fix());
}
