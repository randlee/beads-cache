//! The shared file format: the exact v3 DDL plus the additive extensions
//! (REQ-BDC-0804 amended, ADR-STORE-0001).
//!
//! The store file stays readable and writable by any v3 reader. That is a
//! property of the format, so it is tested on the format: no other
//! implementation runs here.
//!
//! - `compat_format/v3_base.sql` is the v3 DDL, byte for byte.
//! - `compat_format/schema.golden` is `sqlite_master` after the store opened a
//!   fresh file: the v3 objects with the extension columns appended, then the
//!   extension tables and indexes.
//!
//! A change to either file is a format change and needs an ADR. Run with
//! `BDC_BLESS=1` to rewrite `schema.golden` from the actual dump when the
//! change is intended.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a failed unwrap or index is a test failure"
)]

mod common;

use std::fmt::Write as _;
use std::path::Path;

use bd_cache_model::CacheStore;
use bd_cache_store::SqliteStore;
use common::{TempDir, identity};
use rusqlite::Connection;

/// The v3 DDL as a v3 writer runs it.
const V3_BASE: &str = include_str!("compat_format/v3_base.sql");
/// `sqlite_master` after the store opened a fresh file, as [`dump`] renders it.
const GOLDEN: &str = include_str!("compat_format/schema.golden");

/// A checkout may convert line endings; the SQL text is stored verbatim, so
/// compare with LF.
fn lf(text: &str) -> String {
    text.replace("\r\n", "\n")
}

/// One `sqlite_master` entry: `(type, name, sql)`.
type Object = (String, String, String);

fn objects(conn: &Connection) -> Vec<Object> {
    let mut stmt = conn
        .prepare(
            "SELECT type, name, sql FROM sqlite_master \
             WHERE name NOT LIKE 'sqlite_%' AND sql IS NOT NULL \
             ORDER BY type DESC, name",
        )
        .unwrap();
    stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

/// The golden's rendering: `-- <type> <name>`, then the stored SQL and `;`.
fn dump(conn: &Connection) -> String {
    let mut out = String::new();
    for (kind, name, sql) in objects(conn) {
        writeln!(out, "-- {kind} {name}\n{sql};").unwrap();
    }
    out
}

fn user_version(conn: &Connection) -> i32 {
    conn.pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap()
}

fn raw(path: &Path) -> Connection {
    Connection::open(path).unwrap()
}

/// A file as a v3 writer leaves it: the v3 DDL, version 3 and the identity.
fn v3_file(path: &Path) {
    let conn = raw(path);
    conn.execute_batch(&lf(V3_BASE)).unwrap();
    conn.pragma_update(None, "user_version", 3).unwrap();
    conn.execute_batch(
        "INSERT INTO meta VALUES ('dolt_database', 'testdb'), ('project_id', 'p-1')",
    )
    .unwrap();
}

fn assert_golden(actual: &str) {
    if std::env::var_os("BDC_BLESS").is_some() {
        let golden =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/compat_format/schema.golden");
        std::fs::write(golden, actual).unwrap();
        return;
    }
    assert_eq!(
        actual,
        lf(GOLDEN),
        "the file schema changed; a format change needs an ADR (ADR-STORE-0001)"
    );
}

#[test]
fn a_fresh_store_file_has_the_golden_schema_and_user_version_3() {
    let tmp = TempDir::new("golden");
    let path = tmp.path().join("testdb.bd-cache.sqlite3");
    drop(SqliteStore::open_at(&path, &identity()).unwrap());

    let conn = raw(&path);
    assert_eq!(user_version(&conn), 3, "the version is never changed");
    assert_golden(&dump(&conn));
}

#[test]
fn the_extensions_only_add_to_the_v3_objects() {
    let tmp = TempDir::new("additive");
    let base = tmp.path().join("base.sqlite3");
    raw(&base).execute_batch(&lf(V3_BASE)).unwrap();
    let v3 = objects(&raw(&base));
    assert_eq!(v3.len(), 8, "five v3 tables and three v3 indexes: {v3:?}");

    let path = tmp.path().join("opened.sqlite3");
    drop(SqliteStore::open_at(&path, &identity()).unwrap());
    let opened = objects(&raw(&path));

    for (kind, name, sql) in &v3 {
        let (_, _, now) = opened
            .iter()
            .find(|(k, n, _)| (k, n) == (kind, name))
            .unwrap_or_else(|| panic!("the v3 {kind} {name} is gone"));
        if kind == "index" {
            assert_eq!(now, sql, "the v3 index {name} is unchanged");
        } else {
            // `ALTER TABLE ADD COLUMN` only appends `, <column> <type>` before the
            // closing parenthesis: every v3 column and constraint is where it was.
            let head = sql.strip_suffix(')').unwrap();
            assert!(
                now.starts_with(head) && now.ends_with(')'),
                "the v3 table {name} changed beyond added columns:\n{sql}\n{now}"
            );
            let added = &now[head.len()..now.len() - 1];
            assert!(
                added.is_empty() || added.starts_with(','),
                "{name}: not an appended column list: {added:?}"
            );
        }
    }
    assert!(
        opened.len() > v3.len(),
        "the extension tables and indexes are added"
    );
}

#[test]
fn a_v3_only_file_gains_the_extensions_and_keeps_its_rows() {
    let tmp = TempDir::new("v3-only");
    let path = tmp.path().join("testdb.bd-cache.sqlite3");
    v3_file(&path);
    {
        let conn = raw(&path);
        conn.execute_batch(
            "INSERT INTO journal (idem_key, phase, bead, op, args_json, precondition_json, \
                 actor, state, attempts, created_at, updated_at) \
             VALUES ('k-1', 'ct', 'ct-3', 'close', '{\"reason\": \"done\"}', '{}', \
                 'dev', 'pending', 0, '2026-10-23T10:00:00Z', '2026-10-23T10:00:00Z'); \
             INSERT INTO sync_state (phase, last_started_at, last_ok_at) \
             VALUES ('ct', '2026-10-23T10:00:00Z', '2026-10-23T10:00:00Z');",
        )
        .unwrap();
    }

    let store = SqliteStore::open_at(&path, &identity()).unwrap();
    let pending = store.pending_entries().unwrap();
    assert_eq!(pending.len(), 1, "the v3 writer's journal entry is read");
    assert_eq!(pending[0].idem_key.as_str(), "k-1");
    assert_eq!(pending[0].bead.as_str(), "ct-3");
    drop(store);

    let conn = raw(&path);
    assert_eq!(user_version(&conn), 3, "the version stays 3");
    assert_golden(&dump(&conn));
    let (state, attempts): (String, i64) = conn
        .query_row(
            "SELECT state, attempts FROM journal WHERE idem_key = 'k-1'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!((state.as_str(), attempts), ("pending", 0));
}

#[test]
fn a_v3_writer_still_writes_the_file_after_the_store_opened_it() {
    let tmp = TempDir::new("v3-writes");
    let path = tmp.path().join("testdb.bd-cache.sqlite3");
    drop(SqliteStore::open_at(&path, &identity()).unwrap());

    {
        // Statements naming only v3 columns, as a v3 writer runs them.
        let conn = raw(&path);
        conn.execute_batch(
            "INSERT INTO beads (id, phase, status, updated_at, row_json, synced_at) \
             VALUES ('w-1', 'ct', 'open', '2026-10-23T10:00:00Z', '{}', '2026-10-23T10:00:00Z'); \
             INSERT INTO edges VALUES ('w-1', 'w-0', 'blocks'); \
             INSERT INTO journal (idem_key, phase, bead, op, args_json, precondition_json, \
                 actor, state, created_at, updated_at) \
             VALUES ('k-w', 'ct', 'w-1', 'close', '{}', '{}', 'dev', 'pending', \
                 '2026-10-23T10:00:00Z', '2026-10-23T10:00:00Z'); \
             UPDATE journal SET state = 'applied', updated_at = '2026-10-23T10:01:00Z' \
             WHERE idem_key = 'k-w'; \
             INSERT OR REPLACE INTO sync_state (phase, last_ok_at) \
             VALUES ('ct', '2026-10-23T10:01:00Z'); \
             DELETE FROM beads WHERE id = 'w-1';",
        )
        .unwrap();
    }

    let store = SqliteStore::open_at(&path, &identity()).unwrap();
    assert!(
        store.pending_entries().unwrap().is_empty(),
        "the entry the v3 writer applied is not pending"
    );
    drop(store);
    let conn = raw(&path);
    assert_eq!(user_version(&conn), 3);
    assert_golden(&dump(&conn));
}
