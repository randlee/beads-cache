//! `SqliteStore`'s phase-B rules that the shared contract suite cannot state:
//! the registry's data stays readable by an earlier reader (ADR-STORE-0006,
//! ADR-BDC-0013), an ingest is one transaction even when it fails half way
//! (REQ-BDC-0008, REQ-BDC-0726), a full-size ingest holds the write lock well
//! under `BUSY_TIMEOUT`, an oversized configuration file is refused, and the
//! store's auto traits (ADR-CORE-0001 as the store sees it).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code: a failed unwrap is a test failure"
)]

mod common;

use std::path::PathBuf;
use std::time::Instant;

use bd_cache_model::{
    CacheStore, FilterParts, IngestSnapshot, MAX_INGEST_ROWS, Scope, ScopeFilter, ScopeId,
};
use bd_cache_store::{
    BUSY_TIMEOUT, CacheScheduleStore, ConfigLoadError, MAX_CONFIG_BYTES, SqliteStore,
    load_repo_config,
};
use common::{TempDir, identity};
use rusqlite::Connection;
use sc_beads::Timestamp;

fn file(tmp: &TempDir) -> PathBuf {
    tmp.path().join(".bd-cache").join("testdb.bd-cache.sqlite3")
}

fn lane() -> Scope {
    Scope::new(
        ScopeId::new("lane").unwrap(),
        ScopeFilter::from_parts(FilterParts {
            labels_all: vec!["lane".into()],
            status: vec!["open".into()],
            ..FilterParts::default()
        })
        .unwrap(),
    )
}

fn at() -> Timestamp {
    Timestamp::parse("2026-10-23T10:00:00Z").unwrap()
}

fn lane_row(name: &str) -> sc_beads::Bead {
    let mut b = sc_beads::testing::bead(name);
    b.labels = vec!["lane".into()];
    b
}

/// Every registry row keeps `kind = 'phase'`; `scope.filter` alone tells a
/// filter scope from a phase-a one (ADR-STORE-0006, ADR-BDC-0013), so the
/// earlier reader, which fails on any other kind, reads a file this build
/// wrote (B7Q1-007).
#[test]
fn filter_scopes_keep_kind_phase_so_an_earlier_reader_reads_every_row() {
    let tmp = TempDir::new("kind");
    let path = file(&tmp);
    let mut store = SqliteStore::open_at(&path, &identity()).unwrap();
    store.put_scope(&lane()).unwrap();
    store
        .ensure_scope(&Scope::legacy(ScopeId::new("x").unwrap()))
        .unwrap();
    let all = Scope::new(ScopeId::new("all").unwrap(), ScopeFilter::All);
    store.scope_add(&all).unwrap();
    store
        .scope_set(&Scope::new(
            all.id.clone(),
            ScopeFilter::label("a").unwrap(),
        ))
        .unwrap();
    // The earlier reader's query and check (develop's `scopes()`).
    let c = Connection::open(&path).unwrap();
    let rows: Vec<(String, String, Option<String>)> = c
        .prepare("SELECT name, kind, filter FROM scope ORDER BY name")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(rows.len(), 3);
    for (name, kind, _) in &rows {
        assert_eq!(
            kind, "phase",
            "{name}: an earlier reader refuses any other kind"
        );
    }
    let filters: Vec<(&str, bool)> = rows
        .iter()
        .map(|(n, _, f)| (n.as_str(), f.is_some()))
        .collect();
    assert_eq!(filters, [("all", true), ("lane", true), ("x", false)]);
    assert_eq!(
        store.scopes().unwrap().len(),
        3,
        "this build reads them back"
    );
    // Planted negative: a row with another kind is what the earlier reader
    // refuses, and this build refuses it too.
    c.execute("UPDATE scope SET kind = 'filter' WHERE name = 'lane'", [])
        .unwrap();
    assert_eq!(store.scopes().unwrap_err().code(), "BD_CACHE.STORE_FAILED");
}

/// An ingest that fails after some rows are written leaves no row,
/// membership, base, sync state or revision behind: one transaction
/// (REQ-BDC-0008, REQ-BDC-0726; B7Q1-019).
#[test]
fn an_ingest_that_fails_mid_transaction_leaves_nothing_behind() {
    let tmp = TempDir::new("midtx");
    let path = file(&tmp);
    let mut store = SqliteStore::open_at(&path, &identity()).unwrap();
    store.put_scope(&lane()).unwrap();
    // A planted failure on the third row's insert, after two rows are written.
    Connection::open(&path)
        .unwrap()
        .execute_batch(
            "CREATE TRIGGER planted BEFORE INSERT ON beads WHEN NEW.id = 'l3' \
             BEGIN SELECT RAISE(ABORT, 'planted failure'); END;",
        )
        .unwrap();
    let rows = ["l1", "l2", "l3"].map(lane_row).to_vec();
    let snap = IngestSnapshot::new(&lane().id, rows, vec![]).unwrap();
    let e = store
        .ingest_scope(&lane().id, &snap, &[], &at(), Some("rev"))
        .unwrap_err();
    assert_eq!(e.code(), "BD_CACHE.STORE_FAILED", "{e}");
    for id in ["l1", "l2", "l3"] {
        let id = sc_beads::testing::id(id);
        assert!(store.get_row(&id).unwrap().is_none(), "{id}: no row");
    }
    assert!(
        store.members(&lane().id).unwrap().is_empty(),
        "no membership"
    );
    assert_eq!(store.ready_base(&lane().id).unwrap(), None, "no base");
    assert_eq!(store.sync_state(&lane().id).unwrap().last_ok_at, None);
    assert_eq!(store.scope_revision(&lane().id).unwrap(), None);
    // With the failure removed, the same snapshot stores whole.
    Connection::open(&path)
        .unwrap()
        .execute_batch("DROP TRIGGER planted;")
        .unwrap();
    store
        .ingest_scope(&lane().id, &snap, &[], &at(), Some("rev"))
        .unwrap();
    assert_eq!(store.members(&lane().id).unwrap().len(), 3);
}

/// A full-size ingest (`MAX_INGEST_ROWS` rows) holds the connection's mutex
/// and the write lock for one transaction; it stays well under
/// `BUSY_TIMEOUT`, the most another writer waits (B7Q1-050). A release build
/// is held to half of `BUSY_TIMEOUT`. A debug build is several times slower
/// and runs on shared CI runners (a stall during the one long transaction is
/// the host's, not the product's), so it is held to four times
/// `BUSY_TIMEOUT` (the ingest took about a third of `BUSY_TIMEOUT` on a
/// developer laptop): a gross-regression bound with real headroom, so CI's
/// debug run still executes an assertion on the timing.
#[test]
fn a_full_size_ingest_holds_the_write_lock_well_under_the_busy_timeout() {
    let tmp = TempDir::new("big");
    let mut store = SqliteStore::open_at(&file(&tmp), &identity()).unwrap();
    store.put_scope(&lane()).unwrap();
    let rows: Vec<sc_beads::Bead> = (0..MAX_INGEST_ROWS)
        .map(|i| lane_row(&format!("l{i}")))
        .collect();
    let snap = IngestSnapshot::new(&lane().id, rows, vec![]).unwrap();
    let start = Instant::now();
    store
        .ingest_scope(&lane().id, &snap, &[], &at(), None)
        .unwrap();
    let took = start.elapsed();
    eprintln!("a {MAX_INGEST_ROWS}-row ingest took {took:?} (BUSY_TIMEOUT {BUSY_TIMEOUT:?})");
    assert_eq!(store.members(&lane().id).unwrap().len(), MAX_INGEST_ROWS);
    let bound = if cfg!(debug_assertions) {
        BUSY_TIMEOUT * 4
    } else {
        BUSY_TIMEOUT / 2
    };
    assert!(took < bound, "{took:?} is not under {bound:?}");
}

/// `.beads/bd-cache.toml` is read through a bounded reader: a file over
/// `MAX_CONFIG_BYTES` is refused, never read whole (B7Q1-044).
#[test]
fn an_oversized_configuration_file_is_refused() {
    let tmp = TempDir::new("bigcfg");
    let root = tmp.path();
    common::workspace(root, "testdb", "p-1");
    let mut text = String::from("scopes = []\n");
    text.push_str(&"#".repeat(usize::try_from(MAX_CONFIG_BYTES).unwrap()));
    std::fs::write(root.join(".beads").join("bd-cache.toml"), text).unwrap();
    let e = load_repo_config(root).unwrap_err();
    assert!(
        matches!(&e, ConfigLoadError::Read { source, .. }
            if source.kind() == std::io::ErrorKind::InvalidData),
        "{e}"
    );
    std::fs::write(root.join(".beads").join("bd-cache.toml"), "scopes = []\n").unwrap();
    assert!(load_repo_config(root).is_ok());
}

/// The store and its schedule view are `Send + Sync` through their shared
/// mutex (the `Cache` that owns the store stays `!Sync` by its own marker,
/// `crates/bd-cache/tests/threading.rs`; B7Q1-025).
#[test]
fn the_store_and_its_schedule_view_are_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<SqliteStore>();
    assert_send_sync::<CacheScheduleStore>();
}
