//! `compose` resolves the phases directory against the repository root (QA-008),
//! and its errors carry this crate's stable `sync_config_*` codes (S34), one
//! per `ConfigLoadError` variant, beside the store's own `store_config_*`
//! codes (S37).
//!
//! Every case composes through [`compose_with`] with an explicit `$BEADS_DIR`
//! (lead ruling S36, FTQ-028): the tests do not read the environment, so a
//! `BEADS_DIR` set on the machine cannot point them at a real cache.
//!
//! The `bd` child sees the value the cache resolved from, not the process's
//! (lead ruling S40, RSH-033): on Unix the backend runs `/bin/sh`, which
//! prints the `BEADS_DIR` it was given. One case runs in a child process of
//! this test binary whose environment sets `BEADS_DIR` (no test mutates its
//! own environment).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code: a failed unwrap is a test failure"
)]

use std::path::Path;
use std::sync::Arc;
#[cfg(unix)]
use std::time::{Duration, Instant};

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
    // `store_config_*` code passed through (S34, S37); the loader's fix, one
    // text per way of failing (RBP-F040).
    assert!(
        matches!(loader, ConfigLoadError::Parse { .. }),
        "{loader:?}"
    );
    assert_eq!(err.code(), "sync_config_invalid_toml");
    assert_eq!(loader.code(), "store_config_invalid");
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
    assert_eq!(loader.code(), "store_config_bad_scope");
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
    assert_eq!(loader.code(), "store_config_unreadable");
    assert_eq!(err.fix(), loader.fix());
    assert!(err.fix().contains("readable"), "{}", err.fix());
}

/// Set (to `1`) in the child process of
/// [`the_bd_child_has_no_beads_dir_when_the_explicit_value_is_none_though_the_environment_sets_one`].
#[cfg(unix)]
const CHILD_MARK: &str = "BD_CACHE_SYNC_COMPOSE_S40_CHILD";

/// The `$BEADS_DIR` the composed backend's child sees, `<unset>` when it has
/// none: the backend runs `/bin/sh` in the repository (S40).
#[cfg(unix)]
fn beads_dir_seen_by_the_child(root: &Path, beads_dir_env: Option<&std::ffi::OsStr>) -> String {
    use sc_beads::ProjectRef;
    use sc_beads::backend::CliBackend;

    // Built on the system clock; compose_with rebinds it to the given one (S40).
    let backend = BdCli::new("/bin/sh", Arc::new(ProjectLocks::new()));
    let clock: Arc<dyn Clock> = Arc::new(SystemClock);
    let composed = compose_with(root, backend, clock, beads_dir_env).unwrap();
    let project = ProjectRef::local(Some(root.to_string_lossy().into_owned()));
    let out = bd_cache::testing::backend(&composed.cache)
        .run_raw(&project, &["-c", "printf %s \"${BEADS_DIR-<unset>}\""])
        .unwrap();
    assert!(out.success, "{out:?}");
    out.stdout
}

/// An explicit `$BEADS_DIR` that differs from the process's (here unset, or
/// whatever the machine sets) is the one the `bd` child sees (S40, RSH-033).
#[cfg(unix)]
#[test]
fn the_bd_child_sees_the_explicit_beads_dir_not_the_environments() {
    let root = repo("");
    let other = tempfile::tempdir().unwrap();
    std::fs::write(
        other.path().join("metadata.json"),
        r#"{"dolt_database": "atm-dev", "project_id": "p-1"}"#,
    )
    .unwrap();
    assert_ne!(
        std::env::var_os("BEADS_DIR").as_deref(),
        Some(other.path().as_os_str()),
        "the explicit value differs from the environment"
    );

    let seen = beads_dir_seen_by_the_child(root.path(), Some(other.path().as_os_str()));

    assert_eq!(Path::new(&seen), other.path());
}

/// The environment sets `$BEADS_DIR`, the explicit value is `None`: the cache
/// resolves without it and the `bd` child does not see it either (S40,
/// RSH-033). Runs in a child process of this test binary whose environment
/// sets `BEADS_DIR`; in that process it asserts, here it checks the child
/// passed.
#[cfg(unix)]
#[test]
fn the_bd_child_has_no_beads_dir_when_the_explicit_value_is_none_though_the_environment_sets_one() {
    const NAME: &str = "the_bd_child_has_no_beads_dir_when_the_explicit_value_is_none_though_the_environment_sets_one";
    const ELSEWHERE: &str = "/nonexistent/bd-cache-sync-s40-elsewhere";
    if std::env::var_os(CHILD_MARK).is_some() {
        assert_eq!(
            std::env::var_os("BEADS_DIR").as_deref(),
            Some(std::ffi::OsStr::new(ELSEWHERE)),
            "the child's environment sets BEADS_DIR"
        );
        let root = repo("");
        assert_eq!(beads_dir_seen_by_the_child(root.path(), None), "<unset>");
        return;
    }
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args([NAME, "--exact", "--test-threads=1"])
        .env(CHILD_MARK, "1")
        .env("BEADS_DIR", ELSEWHERE)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let limit = Instant::now() + Duration::from_secs(120);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= limit {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("the child test did not finish within 120 s");
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let out = child.wait_with_output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        status.success(),
        "{stdout}\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    // The child ran the case, not an empty filter.
    assert!(stdout.contains("1 passed"), "{stdout}");
}
