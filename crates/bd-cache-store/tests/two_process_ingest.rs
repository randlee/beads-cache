//! A second process ingests while a first holds a handle (AC6; REQ-BDC-0726,
//! ADR-STORE-0002's concurrency rules, NFR-BDC-0007).
//!
//! The parent opens the cache file and keeps the handle, with no transaction
//! open; it runs this test binary again as the child (`child_ingests`, ignored
//! unless asked for by name), which opens the same file in its own process and
//! ingests a scope. The parent then reads the result through its handle.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code: a failed unwrap is a test failure"
)]

mod common;

use std::path::PathBuf;
use std::process::Command;

use bd_cache_model::{CacheStore, FilterParts, IngestSnapshot, Scope, ScopeFilter, ScopeId};
use bd_cache_store::SqliteStore;
use common::{TempDir, identity};
use sc_beads::{Clock, SystemClock};

/// Where the child finds the cache file.
const CHILD_FILE: &str = "BD_CACHE_STORE_CHILD_FILE";

fn lane() -> Scope {
    Scope::new(
        ScopeId::new("lane").unwrap(),
        ScopeFilter::from_parts(FilterParts {
            labels_all: vec!["lane".into()],
            ..FilterParts::default()
        })
        .unwrap(),
    )
}

#[test]
fn a_second_process_ingests_while_a_first_holds_a_handle() {
    let tmp = TempDir::new("two-process-ingest");
    let path: PathBuf = tmp.path().join(".bd-cache").join("testdb.bd-cache.sqlite3");
    let mut first = SqliteStore::open_at(&path, &identity()).unwrap();
    first.ensure_scope(&lane()).unwrap();
    // The first handle stays open, with no transaction open, while the child runs.
    let out = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "child_ingests", "--ignored", "--nocapture"])
        .env(CHILD_FILE, &path)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "child failed: {}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let ids: Vec<String> = first
        .rows_in_scope(&lane().id)
        .unwrap()
        .into_iter()
        .map(|(b, _)| b.id.to_string())
        .collect();
    assert_eq!(
        ids,
        ["c1", "c2"],
        "the first handle reads what the second ingested"
    );
    assert_eq!(
        first.scope_revision(&lane().id).unwrap(),
        Some("from-child".to_owned())
    );
    assert!(first.sync_state(&lane().id).unwrap().last_ok_at.is_some());
}

/// The child: run only by the parent test, through its environment.
#[test]
#[ignore = "run by a_second_process_ingests_while_a_first_holds_a_handle as its child"]
fn child_ingests() {
    let Some(path) = std::env::var_os(CHILD_FILE) else {
        return;
    };
    let mut second = SqliteStore::open_at(&PathBuf::from(path), &identity()).unwrap();
    let rows = ["c1", "c2"]
        .iter()
        .map(|n| {
            let mut b = sc_beads::testing::bead(n);
            b.labels = vec!["lane".into()];
            b
        })
        .collect();
    let snapshot = IngestSnapshot::new(&lane().id, rows, vec![]).unwrap();
    let at = SystemClock.now_utc();
    second
        .ingest_scope(&lane().id, &snapshot, &[], &at, Some("from-child"))
        .unwrap();
}
