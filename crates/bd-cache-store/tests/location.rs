//! Locating the cache file and loading `.beads/bd-cache.toml` (REQ-BDC-0101, 0102).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a failed unwrap or index is a test failure"
)]

mod common;

use std::ffi::OsStr;
use std::path::PathBuf;

use bd_cache_model::RepoConfig;
use bd_cache_store::{
    BeadsDirOverride, CacheLocation, ConfigLoadError, DoltDatabase, ProjectId, SqliteStore,
    load_repo_config,
};
use common::{CHILD_TIMEOUT, TempDir, run_bounded, this_test_binary, workspace};

#[test]
fn test_cache_file_is_named_for_the_dolt_database() {
    let tmp = TempDir::new("named");
    workspace(tmp.path(), "testdb", "p-1");
    let loc = CacheLocation::resolve_with(tmp.path(), &RepoConfig::default(), None).unwrap();
    assert_eq!(
        loc.cache_path,
        tmp.path().join(".bd-cache").join("testdb.bd-cache.sqlite3")
    );
    assert_eq!(loc.identity.dolt_database.as_str(), "testdb");
    assert_eq!(loc.identity.project_id.as_str(), "p-1");
    let store = SqliteStore::open_at(&loc.cache_path, &loc.identity).unwrap();
    assert!(store.path().exists());
}

#[test]
fn configured_cache_path_is_taken_from_the_repository_root() {
    let tmp = TempDir::new("configured");
    workspace(tmp.path(), "testdb", "p-1");
    let config = RepoConfig {
        cache_path: Some(PathBuf::from(".cache-dir/testdb.bd-cache.sqlite3")),
        ..RepoConfig::default()
    };
    let loc = CacheLocation::resolve_with(tmp.path(), &config, None).unwrap();
    assert_eq!(
        loc.cache_path,
        tmp.path()
            .join(".cache-dir")
            .join("testdb.bd-cache.sqlite3")
    );
}

#[test]
fn test_beads_dir_env_names_the_database() {
    let tmp = TempDir::new("beads-dir");
    let elsewhere = tmp.path().join("elsewhere");
    workspace(&elsewhere, "otherdb", "p-9");
    let beads = elsewhere.join(".beads");
    let dir = BeadsDirOverride::new(Some(beads.as_os_str())).unwrap();
    let loc =
        CacheLocation::resolve_with(tmp.path(), &RepoConfig::default(), dir.as_ref()).unwrap();
    assert_eq!(
        loc.cache_path.file_name(),
        Some(OsStr::new("otherdb.bd-cache.sqlite3"))
    );
    assert_eq!(loc.identity.project_id.as_str(), "p-9");
    // An empty BEADS_DIR is unset, as in the v3 Python writer.
    workspace(tmp.path(), "testdb", "p-1");
    let empty = BeadsDirOverride::new(Some(OsStr::new(""))).unwrap();
    assert_eq!(empty, None);
    let loc =
        CacheLocation::resolve_with(tmp.path(), &RepoConfig::default(), empty.as_ref()).unwrap();
    assert_eq!(loc.identity.dolt_database.as_str(), "testdb");

    // `resolve` reads the real environment: run it in a child with BEADS_DIR set.
    let out = run_bounded(
        this_test_binary("beads_dir_env_child")
            .env("BEADS_DIR", &beads)
            .env("BDC_TEST_ROOT", tmp.path()),
        CHILD_TIMEOUT,
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "{stdout}");
    assert!(
        stdout.contains("cache_path=") && stdout.contains("otherdb.bd-cache.sqlite3"),
        "{stdout}"
    );
}

#[test]
#[ignore = "child process of test_beads_dir_env_names_the_database"]
fn beads_dir_env_child() {
    let Some(root) = std::env::var_os("BDC_TEST_ROOT") else {
        return;
    };
    let loc = CacheLocation::resolve(root.as_ref(), &RepoConfig::default()).unwrap();
    println!("cache_path={}", loc.cache_path.display());
}

#[test]
fn test_missing_metadata_is_typed() {
    let tmp = TempDir::new("missing");
    let none = CacheLocation::resolve_with(tmp.path(), &RepoConfig::default(), None);
    assert_eq!(none.unwrap_err().code(), "BD_CACHE.NO_DATABASE");
    // `None`: no `$BEADS_DIR`, not the ambient one (lead ruling S36).
    let open = SqliteStore::open_with(tmp.path(), &RepoConfig::default(), None);
    assert_eq!(open.unwrap_err().code(), "BD_CACHE.NO_DATABASE");
    std::fs::create_dir_all(tmp.path().join(".beads")).unwrap();
    for bad in [
        "not json",
        r#"{"project_id": "p-1"}"#,
        r#"{"dolt_database": "testdb"}"#,
        r#"{"dolt_database": "a/b", "project_id": "p-1"}"#,
        r#"{"dolt_database": "", "project_id": "p-1"}"#,
        r#"{"dolt_database": "testdb", "project_id": 7}"#,
        r#"{"dolt_database": "testdb", "project_id": " "}"#,
    ] {
        std::fs::write(tmp.path().join(".beads").join("metadata.json"), bad).unwrap();
        let err =
            CacheLocation::resolve_with(tmp.path(), &RepoConfig::default(), None).unwrap_err();
        assert_eq!(err.code(), "BD_CACHE.NO_DATABASE", "{bad}");
        assert!(err.detail().contains("metadata.json"), "{}", err.detail());
    }
}

#[test]
fn identity_values_are_validated() {
    for bad in [
        "",
        ".",
        "..",
        "a/b",
        "a\\b",
        "a\nb",
        "a:b",
        "a*b",
        "a?b",
        "a\"b",
        "a<b",
        "a>b",
        "a|b",
        "db.",
        "db ",
        "CON",
        "con",
        "Nul",
        "aux.db",
        "COM1",
        "lpt9",
        "PRN",
        "CONIN$",
        "conout$.db",
    ] {
        assert!(DoltDatabase::new(bad).is_err(), "{bad:?}");
    }
    for good in ["dev", "testdb", "a.b", "console", "com10", "lpt", "x_1"] {
        assert!(DoltDatabase::new(good).is_ok(), "{good:?}");
    }
    assert!(ProjectId::new("").is_err());
    assert!(ProjectId::new("p-1").is_ok());
}

#[test]
fn missing_repo_config_is_the_default() {
    let tmp = TempDir::new("cfg-missing");
    assert_eq!(load_repo_config(tmp.path()).unwrap(), RepoConfig::default());
}

#[test]
fn repo_config_is_parsed_and_unknown_keys_refused() {
    let tmp = TempDir::new("cfg");
    std::fs::create_dir_all(tmp.path().join(".beads")).unwrap();
    let file = tmp.path().join(".beads").join("bd-cache.toml");
    std::fs::write(
        &file,
        "cache_path = \".cache-dir/x.sqlite3\"\nscopes = [\"bk\", \"phase-bj\"]\nmax_attempts = 3\n\
         [tick]\nteam = \"dev-team\"\n",
    )
    .unwrap();
    let cfg = load_repo_config(tmp.path()).unwrap();
    assert_eq!(cfg.scopes, ["bk", "phase-bj"]);
    assert_eq!(cfg.max_attempts, Some(3));
    assert_eq!(cfg.cache_path, Some(PathBuf::from(".cache-dir/x.sqlite3")));
    assert_eq!(cfg.tick.and_then(|t| t.team).as_deref(), Some("dev-team"));

    std::fs::write(&file, "surprise = 1\n").unwrap();
    let err = load_repo_config(tmp.path()).unwrap_err();
    assert!(matches!(err, ConfigLoadError::Parse { .. }), "{err:?}");
    let shown = err.to_string();
    assert!(
        shown.contains("bd-cache.toml") && shown.contains("fix:") && !shown.contains("surprise"),
        "Display names the file and the fix, not the cause: {shown}"
    );
    // One fix text per way of failing: the message states the one `fix()` returns (RBP-F040).
    assert_eq!(err.code(), "store_config_invalid");
    assert!(shown.ends_with(&format!("fix: {}", err.fix())), "{shown}");
    let cause = std::error::Error::source(&err)
        .map(ToString::to_string)
        .unwrap_or_default();
    assert!(
        cause.contains("surprise"),
        "the cause is carried once, as the source: {cause}"
    );
    assert!(
        std::error::Error::source(&err).is_some(),
        "keeps the parse error"
    );

    std::fs::write(&file, "scopes = [\"bad name\"]\n").unwrap();
    let err = load_repo_config(tmp.path()).unwrap_err();
    assert!(
        matches!(&err, ConfigLoadError::Scope { source, .. } if source.value == "bad name"),
        "{err:?}"
    );
    assert_eq!(err.code(), "store_config_bad_scope");
    assert!(
        err.fix().contains(bd_cache_model::ScopeName::RULE),
        "{}",
        err.fix()
    );
    assert!(
        err.to_string().ends_with(&format!("fix: {}", err.fix())),
        "{err}"
    );
}

#[test]
fn unreadable_repo_config_is_an_error_not_the_default() {
    let tmp = TempDir::new("cfg-dir");
    // A directory where the file should be: present but unreadable as text.
    std::fs::create_dir_all(tmp.path().join(".beads").join("bd-cache.toml")).unwrap();
    let err = load_repo_config(tmp.path()).unwrap_err();
    assert!(matches!(err, ConfigLoadError::Read { .. }), "{err:?}");
    assert_eq!(err.code(), "store_config_unreadable");
    assert!(
        err.to_string().ends_with(&format!("fix: {}", err.fix())),
        "{err}"
    );
}

/// PE-RSH-006: a zero `call_timeout_s`, `miss_fill_timeout_ms` or `max_attempts`
/// is refused at load as a parse error with its own message, and 1 is accepted.
#[test]
fn zero_timeouts_and_attempts_are_refused_at_load_each_with_its_own_message() {
    let tmp = TempDir::new("cfg-zero");
    std::fs::create_dir_all(tmp.path().join(".beads")).unwrap();
    let file = tmp.path().join(".beads").join("bd-cache.toml");
    let mut causes = Vec::new();
    for key in ["call_timeout_s", "miss_fill_timeout_ms", "max_attempts"] {
        std::fs::write(&file, format!("{key} = 0\n")).unwrap();
        let err = load_repo_config(tmp.path()).unwrap_err();
        assert!(
            matches!(err, ConfigLoadError::Parse { .. }),
            "{key}: {err:?}"
        );
        assert_eq!(err.code(), "store_config_invalid", "{key}");
        let cause = std::error::Error::source(&err)
            .map(ToString::to_string)
            .unwrap_or_default();
        assert!(
            cause.contains(&format!("{key} = 0 is out of range: it must be at least 1")),
            "{key}: {cause}"
        );
        causes.push(cause);

        std::fs::write(&file, format!("{key} = 1\n")).unwrap();
        assert!(load_repo_config(tmp.path()).is_ok(), "{key} = 1 is valid");
    }
    causes.dedup();
    assert_eq!(causes.len(), 3, "one message per key: {causes:?}");
}

/// PE-RBP-F014: a `metadata.json` that is not JSON is `NO_DATABASE` with the parse
/// error kept as the source and its text in the detail.
#[test]
fn metadata_json_that_does_not_parse_keeps_its_cause() {
    let tmp = TempDir::new("meta-junk");
    std::fs::create_dir_all(tmp.path().join(".beads")).unwrap();
    std::fs::write(tmp.path().join(".beads").join("metadata.json"), "{not json").unwrap();
    let err = CacheLocation::resolve_with(tmp.path(), &RepoConfig::default(), None).unwrap_err();
    assert_eq!(err.code(), "BD_CACHE.NO_DATABASE");
    let source = std::error::Error::source(&err).expect("the parse error is kept");
    assert!(source.is::<serde_json::Error>(), "{source:?}");
    assert!(
        err.detail().contains("not JSON") && err.detail().contains(&source.to_string()),
        "{}",
        err.detail()
    );
    assert!(
        err.fix().contains("bd init"),
        "a bad file keeps the init advice"
    );
}

/// PE-RBP-F014: a `metadata.json` this process may not read says so, in the detail
/// and in the fix, instead of the `bd init` advice; a missing one keeps that advice.
#[cfg(unix)]
#[test]
fn metadata_json_that_cannot_be_read_says_permission_denied_not_bd_init() {
    use std::os::unix::fs::PermissionsExt;
    let tmp = TempDir::new("meta-denied");
    workspace(tmp.path(), "testdb", "p-1");
    let meta = tmp.path().join(".beads").join("metadata.json");
    std::fs::set_permissions(&meta, std::fs::Permissions::from_mode(0o000)).unwrap();
    let readable = std::fs::read(&meta).is_ok();
    let result = CacheLocation::resolve_with(tmp.path(), &RepoConfig::default(), None);
    std::fs::set_permissions(&meta, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert!(
        !readable,
        "this test needs an unprivileged user: mode 000 did not stop the read"
    );
    let err = result.unwrap_err();
    assert_eq!(err.code(), "BD_CACHE.NO_DATABASE");
    assert!(
        err.detail().starts_with("permission denied reading"),
        "{}",
        err.detail()
    );
    let source = std::error::Error::source(&err).expect("the I/O error is kept");
    assert_eq!(
        source
            .downcast_ref::<std::io::Error>()
            .map(std::io::Error::kind),
        Some(std::io::ErrorKind::PermissionDenied)
    );
    assert!(!err.fix().contains("bd init"), "{}", err.fix());
    assert!(err.fix().contains("permissions"), "{}", err.fix());

    std::fs::remove_file(&meta).unwrap();
    let missing =
        CacheLocation::resolve_with(tmp.path(), &RepoConfig::default(), None).unwrap_err();
    assert_eq!(missing.code(), "BD_CACHE.NO_DATABASE");
    assert!(missing.fix().contains("bd init"), "{}", missing.fix());
}
