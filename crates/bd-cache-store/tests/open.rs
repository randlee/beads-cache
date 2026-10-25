//! Opening the file: settings, schema version, identity, one connection per process
//! (REQ-BDC-0103, 0104, 0107; NFR-BDC-0007).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a failed unwrap or index is a test failure"
)]

mod common;

use bd_cache_store::{BUSY_TIMEOUT, SCHEMA_VERSION, SqliteStore};
use common::{TempDir, identity, identity_of};
use rusqlite::Connection;

fn file(tmp: &TempDir) -> std::path::PathBuf {
    tmp.path().join(".bd-cache").join("testdb.bd-cache.sqlite3")
}

/// An inspection connection, opened only while no store is open on the file.
fn raw(path: &std::path::Path) -> Connection {
    Connection::open(path).unwrap()
}

#[test]
fn test_newer_cache_schema_is_refused() {
    let tmp = TempDir::new("schema");
    let path = file(&tmp);
    drop(SqliteStore::open_at(&path, &identity()).unwrap());
    raw(&path).pragma_update(None, "user_version", 99).unwrap();
    let err = SqliteStore::open_at(&path, &identity()).unwrap_err();
    assert_eq!(err.code(), "BD_CACHE.SCHEMA_VERSION");
    assert!(err.detail().contains("schema 99"), "{}", err.detail());
    let version: i32 = raw(&path)
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(version, 99, "a refused open changes nothing");
}

#[test]
fn version_zero_is_initialized_to_three_and_three_is_kept() {
    let tmp = TempDir::new("v0");
    let path = file(&tmp);
    drop(SqliteStore::open_at(&path, &identity()).unwrap());
    let version: i32 = raw(&path)
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(version, SCHEMA_VERSION);
    assert_eq!(SCHEMA_VERSION, 3);
    drop(SqliteStore::open_at(&path, &identity()).unwrap());
    let version: i32 = raw(&path)
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(version, 3);
}

#[test]
fn test_file_from_another_project_is_refused() {
    let tmp = TempDir::new("wrong-db");
    let path = file(&tmp);
    drop(SqliteStore::open_at(&path, &identity()).unwrap());
    let meta: Vec<(String, String)> = {
        let c = raw(&path);
        let mut s = c
            .prepare("SELECT key, value FROM meta WHERE key IN ('dolt_database', 'project_id') ORDER BY key")
            .unwrap();
        s.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    };
    assert_eq!(
        meta,
        [
            ("dolt_database".to_owned(), "testdb".to_owned()),
            ("project_id".to_owned(), "p-1".to_owned())
        ]
    );
    let err = SqliteStore::open_at(&path, &identity_of("testdb", "p-2")).unwrap_err();
    assert_eq!(err.code(), "BD_CACHE.WRONG_DATABASE");
    assert!(err.detail().contains("p-1") && err.detail().contains("p-2"));
    assert!(
        SqliteStore::open_at(&path, &identity()).is_ok(),
        "the right project still opens"
    );
}

#[test]
fn wal_mode_and_busy_timeout() {
    let tmp = TempDir::new("wal");
    let path = file(&tmp);
    drop(SqliteStore::open_at(&path, &identity()).unwrap());
    let mode: String = raw(&path)
        .pragma_query_value(None, "journal_mode", |r| r.get(0))
        .unwrap();
    assert_eq!(mode, "wal", "WAL is persistent in the file");
    assert_eq!(BUSY_TIMEOUT.as_secs(), 5);
}

#[test]
fn one_connection_per_process() {
    let tmp = TempDir::new("one-conn");
    let path = file(&tmp);
    let first = SqliteStore::open_at(&path, &identity()).unwrap();
    // Also through another spelling of the same path.
    let other = tmp
        .path()
        .join(".bd-cache")
        .join(".")
        .join("testdb.bd-cache.sqlite3");
    for p in [&path, &other] {
        let err = SqliteStore::open_at(p, &identity()).unwrap_err();
        assert!(err.detail().contains("NFR-BDC-0007"), "{}", err.detail());
    }
    drop(first);
    assert!(
        SqliteStore::open_at(&path, &identity()).is_ok(),
        "released on drop"
    );
}

#[test]
fn v3_tables_are_unchanged_and_extensions_are_additive() {
    let tmp = TempDir::new("ddl");
    let path = file(&tmp);
    drop(SqliteStore::open_at(&path, &identity()).unwrap());
    let c = raw(&path);
    let columns = |table: &str| -> Vec<(String, String, bool)> {
        let mut s = c
            .prepare(&format!(
                "SELECT name, type, \"notnull\" FROM pragma_table_info('{table}') ORDER BY cid"
            ))
            .unwrap();
        s.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    };
    let names = |table: &str| -> Vec<String> { columns(table).into_iter().map(|c| c.0).collect() };
    // The v3 columns come first, in v3 order; every added column is nullable.
    assert_eq!(
        names("beads"),
        [
            "id",
            "phase",
            "status",
            "updated_at",
            "row_json",
            "synced_at",
            "stale",
            "membership_known",
            "revision"
        ]
    );
    assert_eq!(
        names("journal"),
        [
            "seq",
            "idem_key",
            "phase",
            "bead",
            "op",
            "args_json",
            "precondition_json",
            "actor",
            "state",
            "attempts",
            "last_error",
            "created_at",
            "updated_at",
            "announced_at",
            "applied_at",
            "applied_epoch"
        ]
    );
    assert_eq!(
        names("sync_state"),
        [
            "phase",
            "last_started_at",
            "last_ok_at",
            "last_error",
            "backend_kind",
            "endpoint",
            "replica_id",
            "journal_seq"
        ]
    );
    assert_eq!(names("meta"), ["key", "value"]);
    assert_eq!(names("edges"), ["issue_id", "depends_on_id", "type"]);
    for (table, from) in [("beads", 7), ("journal", 14), ("sync_state", 4)] {
        for (name, _, notnull) in columns(table).into_iter().skip(from) {
            assert!(!notnull, "{table}.{name} must be nullable");
        }
    }
    for table in ["scope", "scope_member", "ready_base", "ready_base_blocked"] {
        assert!(!names(table).is_empty(), "{table} exists");
    }
    let indexes: Vec<String> = {
        let mut s = c
            .prepare("SELECT name FROM sqlite_master WHERE type = 'index' AND sql IS NOT NULL ORDER BY name")
            .unwrap();
        s.query_map([], |r| r.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    };
    // Additive since sprint a-7 (ADR-STORE-0004): the ready base's index column
    // is nullable, so a writer that does not know it leaves it NULL.
    let ready_base: Vec<(String, bool)> = columns("ready_base")
        .into_iter()
        .map(|(n, _, notnull)| (n, notnull))
        .collect();
    assert!(
        ready_base.contains(&("indexed".to_owned(), false)),
        "ready_base.indexed is nullable: {ready_base:?}"
    );
    for index in [
        "beads_phase",
        "edges_target",
        "journal_pending",
        "journal_applied_epoch",
        "journal_applied_unknown",
    ] {
        assert!(indexes.iter().any(|i| i == index), "{index} in {indexes:?}");
    }
}

#[cfg(unix)]
#[test]
fn one_connection_per_process_through_a_symlink() {
    let tmp = TempDir::new("one-conn-link");
    let path = file(&tmp);
    let first = SqliteStore::open_at(&path, &identity()).unwrap();
    let link = tmp.path().join("link.sqlite3");
    std::os::unix::fs::symlink(&path, &link).unwrap();
    let err = SqliteStore::open_at(&link, &identity()).unwrap_err();
    assert!(err.detail().contains("NFR-BDC-0007"), "{}", err.detail());
    assert_eq!(err.code(), "BD_CACHE.STORE_IN_USE", "{err}");
    let dir_link = tmp.path().join("dir-link");
    std::os::unix::fs::symlink(tmp.path().join(".bd-cache"), &dir_link).unwrap();
    let err =
        SqliteStore::open_at(&dir_link.join("testdb.bd-cache.sqlite3"), &identity()).unwrap_err();
    assert!(err.detail().contains("NFR-BDC-0007"), "{}", err.detail());
    drop(first);
    assert!(
        SqliteStore::open_at(&link, &identity()).is_ok(),
        "released on drop"
    );
}

/// PE-RBP-F008: a second open in this process is `STORE_IN_USE`, whose fix says to
/// close or reuse the other handle, never `STORE_FAILED`'s "move the file aside".
#[test]
fn a_second_open_in_this_process_is_store_in_use_with_its_own_fix() {
    let tmp = TempDir::new("store-in-use");
    let path = file(&tmp);
    let first = SqliteStore::open_at(&path, &identity()).unwrap();
    let err = SqliteStore::open_at(&path, &identity()).unwrap_err();
    assert_eq!(err.code(), "BD_CACHE.STORE_IN_USE", "{err}");
    assert!(
        err.detail().contains(&path.display().to_string()),
        "names the file: {}",
        err.detail()
    );
    assert!(
        err.fix().contains("close that handle, or reuse it"),
        "{}",
        err.fix()
    );
    assert!(!err.fix().contains("move it aside"), "{}", err.fix());
    assert!(std::error::Error::source(&err).is_none());
    drop(first);
    assert!(
        SqliteStore::open_at(&path, &identity()).is_ok(),
        "the file is fine once the first handle is closed"
    );
}

#[test]
fn a_store_failure_is_store_failed_with_its_source() {
    let tmp = TempDir::new("store-failed");
    let blocker = tmp.path().join("not-a-dir");
    std::fs::write(&blocker, "x").unwrap();
    let err =
        SqliteStore::open_at(&blocker.join("testdb.bd-cache.sqlite3"), &identity()).unwrap_err();
    assert_eq!(err.code(), "BD_CACHE.STORE_FAILED", "{err}");
    assert!(
        err.detail().contains("create the cache directory"),
        "{}",
        err.detail()
    );
    let source = std::error::Error::source(&err).expect("the I/O error is kept as the source");
    assert!(source.is::<std::io::Error>(), "{source:?}");
    // A file that is not SQLite fails the same way, not as NO_DATABASE.
    let junk = tmp.path().join("junk.sqlite3");
    std::fs::write(&junk, "this is not a database file, just text ".repeat(64)).unwrap();
    let err = SqliteStore::open_at(&junk, &identity()).unwrap_err();
    assert_eq!(err.code(), "BD_CACHE.STORE_FAILED", "{err}");
    assert!(std::error::Error::source(&err).is_some());
}

#[test]
fn a_refused_file_keeps_its_journal_mode_version_and_data() {
    let tmp = TempDir::new("refused-untouched");
    let path = file(&tmp);
    drop(SqliteStore::open_at(&path, &identity()).unwrap());
    let snapshot = |c: &Connection| -> (String, i32, i64) {
        (
            c.pragma_query_value(None, "journal_mode", |r| r.get(0))
                .unwrap(),
            c.pragma_query_value(None, "user_version", |r| r.get(0))
                .unwrap(),
            c.query_row("SELECT COUNT(*) FROM sqlite_master", [], |r| r.get(0))
                .unwrap(),
        )
    };
    {
        let c = raw(&path);
        let _: String = c
            .pragma_update_and_check(None, "journal_mode", "DELETE", |r| r.get(0))
            .unwrap();
    }
    let before = snapshot(&raw(&path));
    assert_eq!(before.0, "delete");
    // Another project: refused, nothing changes, WAL is not set.
    let err = SqliteStore::open_at(&path, &identity_of("testdb", "p-2")).unwrap_err();
    assert_eq!(err.code(), "BD_CACHE.WRONG_DATABASE");
    assert_eq!(snapshot(&raw(&path)), before);
    // A newer schema: likewise.
    raw(&path).pragma_update(None, "user_version", 99).unwrap();
    let before = snapshot(&raw(&path));
    let err = SqliteStore::open_at(&path, &identity()).unwrap_err();
    assert_eq!(err.code(), "BD_CACHE.SCHEMA_VERSION");
    assert_eq!(snapshot(&raw(&path)), before);
    assert_eq!(before.0, "delete", "the journal mode is untouched");
}
