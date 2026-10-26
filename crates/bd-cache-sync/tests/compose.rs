//! `compose` names the cache file, the beads directory and the schedule table,
//! and its errors carry this crate's stable `sync_config_*` codes (S34), one
//! per `ConfigLoadError` variant, beside the store's own `store_config_*`
//! codes (S37).
//!
//! Every case composes through [`compose_with`] with an explicit `$BEADS_DIR`
//! (lead ruling S36, FTQ-028): the tests do not read the environment, so a
//! `BEADS_DIR` set on the machine cannot point them at a real cache.
//!
//! The `bd` child sees the value the cache resolved from, not the process's
//! (lead ruling S40, RSH-033), normalized once (lead ruling S44, RSH-035,
//! QA9-003): an empty value is unset, a relative one is absolute against the
//! process's working directory. On Unix the backend runs `/bin/sh`, which
//! prints the `BEADS_DIR` it was given. Two cases run in a child process of
//! this test binary, one whose environment sets `BEADS_DIR` and one whose
//! working directory is not the repository's (no test changes its own
//! environment or working directory). The child's output is drained on
//! reader threads while it runs, bounded by 120 s, and printed on failure
//! (FTQ-031).
//!
//! The composed backend measures deadlines on the clock `compose_with` is
//! given, not the one it was built with (S40, RBP-F046).
#![expect(
    clippy::unwrap_used,
    reason = "test code: a failed unwrap is a test failure"
)]
// The only `panic!` outside a `#[test]` function is in the Unix-only child
// runner, so the expectation exists only where that code is compiled.
#![cfg_attr(
    unix,
    expect(
        clippy::panic,
        reason = "test code: a child that times out is a test failure"
    )
)]

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use bd_cache_store::ConfigLoadError;
use bd_cache_sync::{ComposeError, Composed, compose_with};
use sc_bd_cli::{BdCli, ProjectLocks};
use sc_beads::testing::ManualClock;
use sc_beads::{BeadsCacheError, CacheBackend, Clock, Deadline, ProjectRef, SystemClock};

/// `compose` without the environment: a `bd` that is never run, and no
/// `$BEADS_DIR` (S36).
fn compose(root: &Path, clock: &Arc<dyn Clock>) -> Result<Composed, ComposeError> {
    compose_with(
        root,
        |locks| BdCli::new_with_clock("bd", locks, Arc::clone(clock)),
        Arc::new(ProjectLocks::new()),
        Arc::clone(clock),
        None,
    )
}

fn repo(toml: &str) -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    let beads = root.path().join(".beads");
    std::fs::create_dir_all(&beads).unwrap();
    std::fs::write(
        beads.join("metadata.json"),
        r#"{"dolt_database": "dev", "project_id": "p-1"}"#,
    )
    .unwrap();
    std::fs::write(beads.join("bd-cache.toml"), toml).unwrap();
    root
}

/// The composition exposes the cache file, the beads directory and the
/// schedule table of the cache file (ADR-BDC-0014).
#[test]
fn the_composition_names_the_cache_file_the_beads_dir_and_its_schedule_table() {
    let root = repo("");
    let clock: Arc<dyn Clock> = Arc::new(sc_beads::SystemClock);
    let composed = compose(root.path(), &clock).unwrap();
    assert!(
        composed
            .cache_path
            .ends_with(".bd-cache/dev.bd-cache.sqlite3"),
        "{:?}",
        composed.cache_path
    );
    assert!(composed.schedule_store().is_ok());
    assert!(composed.beads_dir.is_absolute());
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
    assert!(err.fix().contains("name each scope"), "{}", err.fix());
    // It states the model's one scope-name rule (RBP-F037).
    assert!(
        err.fix().contains(bd_cache_model::ScopeName::RULE),
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

/// The backend is built on the system clock and composed with a manual
/// clock that is an hour ahead: a deadline that has passed on the manual
/// clock, but not on the system clock, is refused without a spawn (lead
/// ruling S40, RBP-F046). On the system clock the missing binary would be
/// spawned and fail to start instead.
#[test]
fn the_composed_backend_measures_deadlines_on_the_composition_clock() {
    let root = repo("");
    let manual = Arc::new(ManualClock::default());
    manual.advance(Duration::from_secs(3600));
    let backend = |locks| BdCli::new("definitely-not-a-real-bd-binary-s40", locks);
    let clock: Arc<dyn Clock> = Arc::clone(&manual) as Arc<dyn Clock>;
    let composed = compose_with(
        root.path(),
        backend,
        Arc::new(ProjectLocks::new()),
        clock,
        None,
    )
    .unwrap();
    let passed = Deadline::at(manual.now_mono());
    assert!(
        !passed.expired(&SystemClock),
        "the deadline is still ahead on the system clock"
    );

    let project = ProjectRef::local(Some(root.path().to_string_lossy().into_owned()));
    let err = bd_cache::testing::backend(&composed.cache)
        .fingerprint(&project, passed)
        .unwrap_err();

    assert!(
        matches!(
            &err,
            BeadsCacheError::Unavailable { reason, timed_out: true }
                if reason.contains("had passed before it started")
        ),
        "{err:?}"
    );
}

/// Set (to `1`) in the child process of a case that runs in one.
#[cfg(unix)]
const CHILD_MARK: &str = "BD_CACHE_SYNC_COMPOSE_S40_CHILD";

/// The repository root of the S44 relative-directory case, set in its child.
#[cfg(unix)]
const S44_ROOT: &str = "BD_CACHE_SYNC_COMPOSE_S44_ROOT";

/// How long a child process may run before it is killed.
#[cfg(unix)]
const CHILD_LIMIT: Duration = Duration::from_secs(120);

/// What the composed backend's child saw: the cache file's name and the
/// `$BEADS_DIR` its `bd` child was given, `<unset>` when it had none. The
/// backend runs `/bin/sh` in the repository (S40).
#[cfg(unix)]
fn seen_by_the_child(root: &Path, beads_dir_env: Option<&std::ffi::OsStr>) -> (String, String) {
    use sc_beads::backend::CliBackend;

    // Built on the system clock; compose_with rebinds it to the given one (S40).
    let backend = |locks| BdCli::new("/bin/sh", locks);
    let clock: Arc<dyn Clock> = Arc::new(SystemClock);
    let composed = compose_with(
        root,
        backend,
        Arc::new(ProjectLocks::new()),
        clock,
        beads_dir_env,
    )
    .unwrap();
    let project = ProjectRef::local(Some(root.to_string_lossy().into_owned()));
    let out = bd_cache::testing::backend(&composed.cache)
        .run_raw(&project, &["-c", "printf %s \"${BEADS_DIR-<unset>}\""])
        .unwrap();
    assert!(out.success, "{out:?}");
    (composed.name, out.stdout)
}

/// The `$BEADS_DIR` the composed backend's child sees ([`seen_by_the_child`]).
#[cfg(unix)]
fn beads_dir_seen_by_the_child(root: &Path, beads_dir_env: Option<&std::ffi::OsStr>) -> String {
    seen_by_the_child(root, beads_dir_env).1
}

/// Reads a pipe to the end on its own thread, so a full pipe never blocks
/// the child (FTQ-031, as `bd-cache-store`'s `tests/common::run_bounded`).
#[cfg(unix)]
fn drain(pipe: Option<impl std::io::Read + Send + 'static>) -> std::sync::mpsc::Receiver<Vec<u8>> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(mut p) = pipe {
            // A read error ends the output early; what was read is reported.
            let _ = std::io::Read::read_to_end(&mut p, &mut buf);
        }
        let _ = tx.send(buf);
    });
    rx
}

/// Runs the test `name` of this binary in a child process configured by
/// `configure`, with [`CHILD_MARK`] set; asserts it ran that one case and
/// passed. Its stdout and stderr are drained while it runs; it is killed
/// after [`CHILD_LIMIT`]; either failure prints both (FTQ-031), or, for a
/// stream whose reader did not finish within 10 s after the exit, a
/// placeholder that says so (QA10-003).
#[cfg(unix)]
fn run_case_in_child(name: &str, configure: impl FnOnce(&mut std::process::Command)) {
    use std::process::{Command, Stdio};
    use std::time::Instant;

    let mut cmd = Command::new(std::env::current_exe().unwrap());
    cmd.args([name, "--exact", "--test-threads=1"])
        .env(CHILD_MARK, "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure(&mut cmd);
    let mut child = cmd.spawn().unwrap();
    let stdout = drain(child.stdout.take());
    let stderr = drain(child.stderr.take());
    let limit = Instant::now() + CHILD_LIMIT;
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break Some(status);
        }
        if Instant::now() >= limit {
            // Already exited (a race with the last poll) is not an error.
            let _ = child.kill();
            let _ = child.wait();
            break None;
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    // After an exit the pipes close at once; after a kill, a grandchild could
    // hold them, so the wait for the readers is bounded too.
    // A reader that does not finish in time is named in its stream's place,
    // not shown as empty output (QA10-003).
    let grace = Duration::from_secs(10);
    let read = |rx: &std::sync::mpsc::Receiver<Vec<u8>>, stream: &str| match rx.recv_timeout(grace)
    {
        Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
        Err(_) => format!(
            "<{stream} reader did not finish within {}s>",
            grace.as_secs()
        ),
    };
    let (out, err) = (read(&stdout, "stdout"), read(&stderr, "stderr"));
    let Some(status) = status else {
        panic!(
            "the child test did not finish within {CHILD_LIMIT:?}\nstdout:\n{out}\nstderr:\n{err}"
        );
    };
    assert!(status.success(), "{status}\nstdout:\n{out}\nstderr:\n{err}");
    // The child ran the case, not an empty filter.
    assert!(out.contains("1 passed"), "stdout:\n{out}\nstderr:\n{err}");
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
        r#"{"dolt_database": "dev", "project_id": "p-1"}"#,
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

/// An explicit empty `$BEADS_DIR` is unset for the store and for the `bd`
/// child alike (lead ruling S44, QA9-003): the child does not get `""`.
#[cfg(unix)]
#[test]
fn an_empty_explicit_beads_dir_is_unset_for_the_bd_child() {
    let root = repo("");

    let seen = beads_dir_seen_by_the_child(root.path(), Some(std::ffi::OsStr::new("")));

    assert_eq!(seen, "<unset>");
}

/// A relative explicit `$BEADS_DIR`, with a working directory that is not
/// the repository: the store reads `metadata.json` under the working
/// directory, and the `bd` child, which runs in the repository, is given
/// that same directory as an absolute path (lead ruling S44, RSH-035). Runs
/// in a child process whose working directory is a temporary directory; only
/// there does `rel/metadata.json` exist, with its own database name.
#[cfg(unix)]
#[test]
fn a_relative_explicit_beads_dir_names_one_absolute_directory_for_the_store_and_the_bd_child() {
    const NAME: &str =
        "a_relative_explicit_beads_dir_names_one_absolute_directory_for_the_store_and_the_bd_child";
    if std::env::var_os(CHILD_MARK).is_some() {
        let root = std::path::PathBuf::from(std::env::var_os(S44_ROOT).unwrap());
        let cwd = std::env::current_dir().unwrap();
        assert_ne!(cwd, root, "the working directory is not the repository");
        assert!(!root.join("rel").exists(), "no rel/ under the repository");

        let (name, seen) = seen_by_the_child(&root, Some(std::ffi::OsStr::new("rel")));

        assert_eq!(name, "rel.bd-cache.sqlite3", "the store read cwd/rel");
        assert!(Path::new(&seen).is_absolute(), "{seen}");
        assert_eq!(Path::new(&seen), cwd.join("rel"));
        return;
    }
    let root = repo("");
    let cwd = tempfile::tempdir().unwrap();
    std::fs::create_dir(cwd.path().join("rel")).unwrap();
    std::fs::write(
        cwd.path().join("rel").join("metadata.json"),
        r#"{"dolt_database": "rel", "project_id": "p-1"}"#,
    )
    .unwrap();
    run_case_in_child(NAME, |cmd| {
        cmd.current_dir(cwd.path()).env(S44_ROOT, root.path());
    });
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
    run_case_in_child(NAME, |cmd| {
        cmd.env("BEADS_DIR", ELSEWHERE);
    });
}
