//! The format golden (ADR-STORE-0006, ADR-STORE-0001, ADR-STORE-0002; REQ-BDC-0804 as
//! amended, REQ-STORE-0003): the cache file's exact DDL is the BD-CACHE v3 DDL plus
//! every phase-B extension, at `user_version` 3.
//!
//! `fixtures/format-golden.txt` is the whole schema a fresh open writes: one line per
//! object of `sqlite_master` (`type name: sql`, sorted by type and name), then the
//! `meta` row of the schedule table's version and `user_version`. It is this sprint's
//! (b-7) evidence that the file stays readable and writable by any v3 reader; sprint
//! b-9 only verifies it. `fixtures/v3-schema.sql` is the v3 DDL as the v3 Python
//! writer runs it, byte for byte.
//!
//! To regenerate after an approved extension (a later ADR):
//! `BD_CACHE_STORE_REGENERATE=1 cargo test -p bd-cache-store --test format_golden`.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code: a failed unwrap is a test failure"
)]

mod common;

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use bd_cache_model::CacheStore;
use bd_cache_store::{SC_CRON_SCHEDULE_VERSION_KEY, SCHEMA_VERSION, SqliteStore};
use common::{TempDir, identity};
use rusqlite::Connection;

const GOLDEN: &str = "tests/fixtures/format-golden.txt";
const V3_SCHEMA: &str = include_str!("fixtures/v3-schema.sql");

fn golden_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(GOLDEN)
}

/// The schema of the file at `path`, as the golden writes it.
fn dump(path: &Path) -> String {
    let c = Connection::open(path).unwrap();
    let mut out = String::new();
    let mut s = c
        .prepare(
            "SELECT type, name, sql FROM sqlite_master \
             WHERE name NOT LIKE 'sqlite_%' AND sql IS NOT NULL ORDER BY type, name",
        )
        .unwrap();
    let rows = s
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })
        .unwrap();
    for r in rows {
        let (kind, name, sql) = r.unwrap();
        // One line per object: the stored text, its whitespace runs folded.
        let sql = sql.split_whitespace().collect::<Vec<_>>().join(" ");
        writeln!(out, "{kind} {name}: {sql}").unwrap();
    }
    let version: Option<String> = c
        .query_row(
            "SELECT value FROM meta WHERE key = ?1",
            [SC_CRON_SCHEDULE_VERSION_KEY],
            |r| r.get(0),
        )
        .ok();
    writeln!(
        out,
        "meta {SC_CRON_SCHEDULE_VERSION_KEY}: {}",
        version.as_deref().unwrap_or("absent")
    )
    .unwrap();
    let user_version: i32 = c
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    writeln!(out, "pragma user_version: {user_version}").unwrap();
    out
}

fn file(tmp: &TempDir) -> PathBuf {
    tmp.path().join(".bd-cache").join("testdb.bd-cache.sqlite3")
}

/// A fresh open writes exactly the golden: the v3 DDL plus every extension of
/// ADR-STORE-0006 (planted negative: drop `scope.filter` from the extension
/// list, and this fails).
#[test]
fn a_fresh_file_matches_the_format_golden() {
    let tmp = TempDir::new("golden");
    let path = file(&tmp);
    drop(SqliteStore::open_at(&path, &identity()).unwrap());
    let got = dump(&path);
    if std::env::var_os("BD_CACHE_STORE_REGENERATE").is_some() {
        std::fs::write(golden_path(), &got).unwrap();
    }
    let want = std::fs::read_to_string(golden_path()).unwrap();
    assert_eq!(
        got, want,
        "the cache file's DDL differs from {GOLDEN}; an extension needs an ADR (ADR-STORE-0006)"
    );
}

/// The golden holds every extension ADR-STORE-0006 lists, and the v3 DDL.
#[test]
fn the_golden_names_every_extension_and_the_v3_tables() {
    let want = std::fs::read_to_string(golden_path()).unwrap();
    let table = |name: &str| {
        want.lines()
            .find(|l| l.starts_with(&format!("table {name}: ")))
            .unwrap_or_else(|| panic!("no table {name} in the golden"))
            .to_owned()
    };
    for (t, column) in [
        ("scope", "revision TEXT"),
        ("scope", "filter TEXT"),
        ("sync_state", "failing_since TEXT"),
        ("sync_state", "notified_since TEXT"),
        ("journal", "first_offered_at TEXT"),
    ] {
        assert!(table(t).contains(column), "{t}.{column}");
    }
    assert!(table("tick_lease").contains("id INTEGER PRIMARY KEY CHECK (id = 1)"));
    assert!(table("hook_state").contains("consecutive_failures INTEGER NOT NULL DEFAULT 0"));
    assert!(table("sc_cron_schedule").contains("enabled INTEGER NOT NULL DEFAULT 1"));
    assert!(want.contains(&format!("meta {SC_CRON_SCHEDULE_VERSION_KEY}: 2\n")));
    assert!(want.contains(&format!("pragma user_version: {SCHEMA_VERSION}\n")));
    for v3 in ["meta", "beads", "edges", "journal", "sync_state"] {
        table(v3);
    }
}

/// Opening a v3-only file adds every extension, and `user_version` stays 3.
#[test]
fn a_v3_only_file_gains_the_extensions_and_stays_at_version_3() {
    let tmp = TempDir::new("golden-v3");
    let path = file(&tmp);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    {
        let c = Connection::open(&path).unwrap();
        c.execute_batch(V3_SCHEMA).unwrap();
        c.pragma_update(None, "user_version", 3).unwrap();
    }
    drop(SqliteStore::open_at(&path, &identity()).unwrap());
    let want = std::fs::read_to_string(golden_path()).unwrap();
    assert_eq!(dump(&path), want, "a v3 file opened is the golden");
}

/// A reader that knows only v3 still reads and writes a file with every
/// extension: its v3 statements, column-named, ignore the additions.
#[test]
fn a_v3_reader_still_reads_and_writes_an_extended_file() {
    let tmp = TempDir::new("golden-reader");
    let path = file(&tmp);
    drop(SqliteStore::open_at(&path, &identity()).unwrap());
    let c = Connection::open(&path).unwrap();
    // The v3 writer's own `CREATE ... IF NOT EXISTS` is a no-op on the file.
    c.execute_batch(V3_SCHEMA).unwrap();
    c.execute(
        "INSERT OR REPLACE INTO beads(id, phase, status, updated_at, row_json, synced_at, stale) \
         VALUES ('b1', 'x', 'open', '2026-10-23T10:00:00Z', '{}', '2026-10-23T10:00:00Z', 0)",
        [],
    )
    .unwrap();
    c.execute(
        "INSERT INTO journal(idem_key, phase, bead, op, args_json, precondition_json, actor, \
         state, created_at, updated_at) VALUES ('k1', 'x', 'b1', 'close', '{}', '{}', 'dev', \
         'pending', '2026-10-23T10:00:00Z', '2026-10-23T10:00:00Z')",
        [],
    )
    .unwrap();
    c.execute(
        "INSERT OR REPLACE INTO sync_state(phase, last_started_at, last_ok_at, last_error) \
         VALUES ('x', '2026-10-23T10:00:00Z', '2026-10-23T10:00:00Z', NULL)",
        [],
    )
    .unwrap();
    let n: i64 = c
        .query_row(
            "SELECT COUNT(*) FROM journal WHERE state = 'pending'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(n, 1);
    let version: i32 = c
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(version, SCHEMA_VERSION);
    drop(c);
    // And this store reads what that writer left.
    let store = SqliteStore::open_at(&path, &identity()).unwrap();
    assert_eq!(store.pending_entries().unwrap().len(), 1);
}
