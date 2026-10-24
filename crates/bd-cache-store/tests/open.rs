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
            "applied_at"
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
    for table in ["scope", "scope_member", "ready_base"] {
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
    for index in ["beads_phase", "edges_target", "journal_pending"] {
        assert!(indexes.iter().any(|i| i == index), "{index} in {indexes:?}");
    }
}
