//! `SqliteStore` cases beyond the contract suite (REQ-BDC-0109, 0604, 0901, 0108).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a failed unwrap or index is a test failure"
)]

mod common;

use bd_cache_model::{CacheStore, DrainLock, IdemKey, NewEntry, Op, ScopeId, normalize};
use bd_cache_store::{FlockDrainLock, SqliteStore};
use common::{TempDir, identity};
use rusqlite::Connection;
use sc_beads::testing::{actor, edge, id};
use sc_beads::{Bead, DepType, Guards, Timestamp};

fn x() -> ScopeId {
    ScopeId::phase("x").unwrap()
}

fn y() -> ScopeId {
    ScopeId::phase("y").unwrap()
}

fn ts(s: &str) -> Timestamp {
    Timestamp::parse(s).unwrap()
}

fn row(name: &str, labels: &[&str]) -> Bead {
    let mut b = sc_beads::testing::bead(name);
    b.labels = labels.iter().map(|l| (*l).to_owned()).collect();
    b
}

fn entry(key: &str, bead: &str, scope: Option<ScopeId>) -> NewEntry {
    NewEntry {
        idem_key: IdemKey::new(key).unwrap(),
        scope,
        bead: id(bead),
        op: Op::Close { reason: None },
        precondition: Guards::default(),
        actor: actor("san"),
    }
}

struct Open {
    _tmp: TempDir,
    path: std::path::PathBuf,
}

fn fresh(tag: &str) -> (Open, SqliteStore) {
    let tmp = TempDir::new(tag);
    let path = tmp.path().join("testdb.bd-cache.sqlite3");
    let store = SqliteStore::open_at(&path, &identity()).unwrap();
    (Open { _tmp: tmp, path }, store)
}

#[test]
fn test_dependents_use_the_edge_index() {
    let (open, mut store) = fresh("edges");
    let mut s1 = row("s1", &["phase-x"]);
    s1.dependencies = vec![edge("s1", "b1", DepType::Blocks)];
    let mut s2 = row("s2", &["phase-x"]);
    s2.dependencies = vec![edge("s2", "b1", DepType::Related)];
    store
        .upsert_rows(Some(&x()), &[row("b1", &["phase-x"]), s1, s2], false)
        .unwrap();
    assert_eq!(
        store.dependents_of(&id("b1")).unwrap(),
        [
            edge("s1", "b1", DepType::Blocks),
            edge("s2", "b1", DepType::Related)
        ]
    );
    assert!(store.dependents_of(&id("s1")).unwrap().is_empty());
    drop(store);
    // The lookup is answered from the target index, not a table scan.
    let plan: Vec<String> = {
        let c = Connection::open(&open.path).unwrap();
        let mut s = c
            .prepare("EXPLAIN QUERY PLAN SELECT issue_id, depends_on_id, type FROM edges WHERE depends_on_id = 'b1'")
            .unwrap();
        s.query_map([], |r| r.get::<_, String>(3))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    };
    assert!(plan.iter().any(|p| p.contains("edges_target")), "{plan:?}");
}

#[test]
fn test_escalations_are_unannounced_until_marked() {
    let (_open, mut store) = fresh("announce");
    let seq = store
        .insert_entry(entry("e1", "b1", Some(x())), &ts("2026-10-23T10:00:00Z"))
        .unwrap();
    store
        .escalate(
            seq,
            "precondition no longer holds",
            &ts("2026-10-23T10:00:01Z"),
        )
        .unwrap();
    let un: Vec<_> = store
        .unannounced_escalations()
        .unwrap()
        .into_iter()
        .map(|e| e.seq)
        .collect();
    assert_eq!(un, [seq]);
    assert!(store.open_work().unwrap().any());
    store
        .mark_announced(seq, &ts("2026-10-23T10:00:02Z"))
        .unwrap();
    assert!(store.unannounced_escalations().unwrap().is_empty());
    assert!(!store.open_work().unwrap().any());
    let e = store.entry_by_seq(seq).unwrap().unwrap();
    assert_eq!(
        e.announced_at.as_ref().map(Timestamp::as_str),
        Some("2026-10-23T10:00:02Z")
    );
    assert_eq!(
        store.status().unwrap().escalated[0].seq,
        seq,
        "listed until resolved"
    );
}

#[test]
fn test_sync_alert_round_trip() {
    let (_open, mut store) = fresh("alert");
    assert_eq!(store.sync_alert(&x()).unwrap(), None);
    store
        .set_sync_alert(&x(), Some("BD_CACHE.SYNC_FAILED: boom"))
        .unwrap();
    assert_eq!(
        store.sync_alert(&x()).unwrap().as_deref(),
        Some("BD_CACHE.SYNC_FAILED: boom")
    );
    assert_eq!(store.sync_alert(&y()).unwrap(), None);
    assert_eq!(
        store
            .status()
            .unwrap()
            .meta
            .get("sync_alert:x")
            .map(String::as_str),
        Some("BD_CACHE.SYNC_FAILED: boom"),
        "stored as the v3 meta key"
    );
    store.set_sync_alert(&x(), None).unwrap();
    assert_eq!(store.sync_alert(&x()).unwrap(), None);
}

#[test]
fn test_pending_work_counts_as_open() {
    let (_open, mut store) = fresh("open-work");
    assert!(!store.open_work().unwrap().any());
    store
        .insert_entry(entry("o1", "b2", Some(x())), &ts("2026-10-23T10:00:00Z"))
        .unwrap();
    let work = store.open_work().unwrap();
    assert_eq!((work.pending, work.escalated_unannounced), (1, 0));
    assert!(work.any());
}

#[test]
fn pruning_a_scope_deletes_only_beads_no_other_scope_holds() {
    let (_open, mut store) = fresh("prune");
    let both = row("both", &["phase-x", "phase-y"]);
    let mut only = row("only-x", &["phase-x"]);
    only.dependencies = vec![edge("only-x", "both", DepType::Blocks)];
    store
        .upsert_rows(Some(&x()), &[only, both.clone()], false)
        .unwrap();
    store.upsert_rows(Some(&y()), &[both], false).unwrap();
    let report = store.prune_scope(&x()).unwrap();
    assert_eq!((report.beads_deleted, report.out_of_scope_swept), (1, 0));
    assert!(store.get_row(&id("only-x")).unwrap().is_none());
    assert!(
        store.dependents_of(&id("both")).unwrap().is_empty(),
        "its edges went with it"
    );
    let ys: Vec<_> = store
        .rows_in_scope(&y())
        .unwrap()
        .into_iter()
        .map(|(b, _)| b.id)
        .collect();
    assert_eq!(ys, [id("both")]);
    assert!(store.rows_in_scope(&x()).unwrap().is_empty());
}

#[test]
fn stored_rows_are_normalized_and_written_for_python() {
    let (open, mut store) = fresh("normalize");
    let mut b = row("b1", &["phase-x", "other"]);
    b.dependency_count = Some(3);
    b.dependent_count = Some(1);
    b.comment_count = Some(2);
    b.dependencies = vec![
        edge("b1", "z", DepType::Related),
        edge("b1", "a", DepType::Blocks),
    ];
    store
        .upsert_rows(Some(&x()), std::slice::from_ref(&b), false)
        .unwrap();
    drop(store);
    let (phase, status, row_json): (Option<String>, Option<String>, String) =
        Connection::open(&open.path)
            .unwrap()
            .query_row(
                "SELECT phase, status, row_json FROM beads WHERE id = 'b1'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
    assert_eq!(phase.as_deref(), Some("x"), "the v3 phase column");
    assert_eq!(status.as_deref(), Some("open"));
    let v: serde_json::Value = serde_json::from_str(&row_json).unwrap();
    assert!(v.get("dependency_count").is_none() && v.get("dependent_count").is_none());
    assert_eq!(v["comment_count"], 2);
    assert_eq!(
        v["dependencies"],
        serde_json::json!([
            {"issue_id": "b1", "depends_on_id": "a", "type": "blocks"},
            {"issue_id": "b1", "depends_on_id": "z", "type": "related"}
        ])
    );
    assert_eq!(
        row_json,
        bd_cache_model::python_json(&serde_json::to_value(normalize(&b)).unwrap())
    );
}

/// Rows `bd_cache.py` replaced (no `membership_known`) belong to their v3
/// `phase`; leftover memberships of beads deleted elsewhere are dropped.
#[test]
fn rows_another_implementation_wrote_belong_to_their_phase() {
    let (open, mut store) = fresh("foreign");
    store.ensure_scope(&x()).unwrap();
    store
        .upsert_rows(
            Some(&x()),
            &[row("kept", &["phase-x"]), row("gone", &["phase-x"])],
            false,
        )
        .unwrap();
    store
        .upsert_rows(Some(&y()), &[row("moved", &["phase-y"])], false)
        .unwrap();
    drop(store);
    {
        // What bd_cache.py does: INSERT OR REPLACE a row (extension columns NULL), DELETE a bead.
        let c = Connection::open(&open.path).unwrap();
        let json: String = c
            .query_row("SELECT row_json FROM beads WHERE id = 'moved'", [], |r| {
                r.get(0)
            })
            .unwrap();
        c.execute(
            "INSERT OR REPLACE INTO beads(id, phase, status, updated_at, row_json, synced_at, stale) \
             VALUES ('moved', 'x', 'open', '2026-10-23T09:00:01Z', ?1, '2026-10-23T10:00:00Z', 0)",
            [json],
        )
        .unwrap();
        c.execute("DELETE FROM beads WHERE id = 'gone'", [])
            .unwrap();
    }
    let mut store = SqliteStore::open_at(&open.path, &identity()).unwrap();
    let ids = |s: &SqliteStore, scope: &ScopeId| -> Vec<String> {
        s.rows_in_scope(scope)
            .unwrap()
            .into_iter()
            .map(|(b, _)| b.id.into_string())
            .collect()
    };
    assert_eq!(ids(&store, &x()), ["kept", "moved"]);
    assert!(ids(&store, &y()).is_empty());
    // A bead deleted elsewhere and stored again unscoped is a new, out-of-scope row.
    store
        .upsert_rows(None, &[row("gone", &["phase-x"])], false)
        .unwrap();
    assert_eq!(ids(&store, &x()), ["kept", "moved"]);
    let x_status = store
        .status()
        .unwrap()
        .scopes
        .into_iter()
        .find(|s| s.sync.scope == x())
        .map(|s| s.beads);
    assert_eq!(x_status, Some(2));
}

#[test]
fn a_python_row_written_while_open_is_read_by_its_phase() {
    let (open, mut store) = fresh("live");
    store
        .upsert_rows(Some(&y()), &[row("b1", &["phase-y"])], false)
        .unwrap();
    let json: String = Connection::open(&open.path)
        .unwrap()
        .query_row("SELECT row_json FROM beads WHERE id = 'b1'", [], |r| {
            r.get(0)
        })
        .unwrap();
    // A second connection, as a concurrent bd_cache.py process would hold.
    Connection::open(&open.path)
        .unwrap()
        .execute(
            "INSERT OR REPLACE INTO beads(id, phase, status, updated_at, row_json, synced_at, stale) \
             VALUES ('b1', 'x', 'open', '2026-10-23T09:00:01Z', ?1, '2026-10-23T10:00:00Z', 0)",
            [json],
        )
        .unwrap();
    assert_eq!(store.rows_in_scope(&x()).unwrap().len(), 1);
    assert!(store.rows_in_scope(&y()).unwrap().is_empty());
    // An unscoped store keeps that membership.
    store
        .upsert_rows(None, &[row("b1", &["phase-y"])], true)
        .unwrap();
    assert_eq!(store.rows_in_scope(&x()).unwrap().len(), 1);
    assert!(store.rows_in_scope(&y()).unwrap().is_empty());
}

#[test]
fn undecodable_stored_values_are_errors_not_skipped() {
    let (open, mut store) = fresh("corrupt");
    store
        .upsert_rows(Some(&x()), &[row("b1", &["phase-x"])], false)
        .unwrap();
    store
        .insert_entry(entry("k", "b1", Some(x())), &ts("2026-10-23T10:00:00Z"))
        .unwrap();
    let c = Connection::open(&open.path).unwrap();
    c.execute(
        "UPDATE beads SET row_json = '{\"surprise\": 1}' WHERE id = 'b1'",
        [],
    )
    .unwrap();
    c.execute("UPDATE journal SET created_at = 'yesterday'", [])
        .unwrap();
    let err = store.get_row(&id("b1")).unwrap_err();
    assert!(err.detail().contains("b1"), "{}", err.detail());
    let err = store.pending_entries().unwrap_err();
    assert!(err.detail().contains("created_at"), "{}", err.detail());
}

/// In this binary because it spawns no processes: a child forked by another
/// test thread holds a copy of every open descriptor until it execs, which
/// would keep a just-dropped lock held for that instant.
#[test]
fn a_guard_dropped_on_another_thread_releases_the_lock() {
    let tmp = TempDir::new("lock-thread");
    let lock = FlockDrainLock::for_cache_file(&tmp.path().join("t.bd-cache.sqlite3"));
    let guard = lock.try_acquire().unwrap().unwrap();
    std::thread::spawn(move || drop(guard)).join().unwrap();
    assert!(lock.try_acquire().unwrap().is_some());
}

#[test]
fn a_read_back_row_of_another_bead_is_refused_and_changes_nothing() {
    let (_open, mut store) = fresh("readback-id");
    store
        .upsert_rows(Some(&x()), &[row("b1", &["phase-x"])], false)
        .unwrap();
    let seq = store
        .insert_entry(entry("k", "b1", Some(x())), &ts("2026-10-23T10:00:00Z"))
        .unwrap();
    let err = store
        .apply_with_readback(
            seq,
            Some(&row("b2", &["phase-x"])),
            None,
            &ts("2026-10-23T10:00:01Z"),
        )
        .unwrap_err();
    assert_eq!(err.code(), "BD_CACHE.BD_CONTRACT", "{err}");
    assert!(
        store.get_row(&id("b2")).unwrap().is_none(),
        "nothing stored"
    );
    let e = store.entry_by_seq(seq).unwrap().unwrap();
    assert_eq!(
        e.state,
        bd_cache_model::JournalState::Pending,
        "still pending"
    );
}

/// The outage is read in one snapshot: a writer setting and clearing it (one
/// transaction each, as both implementations do) never makes the read see
/// one key without the other.
///
/// The reader starts once the writer has completed a full set/clear cycle,
/// then reads until the writer has toggled [`TOGGLES`] times; every stage has
/// a hard deadline that fails naming it. The writer reports its own errors.
#[test]
fn the_outage_is_read_whole_while_another_connection_toggles_it() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
    use std::time::{Duration, Instant};

    const TOGGLES: u32 = 2_000;
    const STAGE_TIMEOUT: Duration = Duration::from_secs(60);

    let (open, store) = fresh("outage-race");
    let path = open.path.clone();
    let stop = Arc::new(AtomicBool::new(false));
    let toggles = Arc::new(AtomicU32::new(0));
    let (ready_tx, ready_rx) = std::sync::mpsc::channel();
    let writer = {
        let (stop, toggles) = (Arc::clone(&stop), Arc::clone(&toggles));
        std::thread::spawn(move || -> rusqlite::Result<()> {
            let c = Connection::open(&path)?;
            c.busy_timeout(Duration::from_secs(5))?;
            while !stop.load(Ordering::Relaxed) {
                c.execute_batch(
                    "BEGIN IMMEDIATE; INSERT OR REPLACE INTO meta VALUES \
                     ('outage_since', '2026-10-23T10:00:00Z'), ('outage_error', 'x'); COMMIT;",
                )?;
                c.execute_batch(
                    "BEGIN IMMEDIATE; DELETE FROM meta WHERE key IN ('outage_since', 'outage_error'); COMMIT;",
                )?;
                if toggles.fetch_add(1, Ordering::Relaxed) == 0 {
                    let _ = ready_tx.send(());
                }
            }
            Ok(())
        })
    };
    let finish = |stop: &AtomicBool| {
        stop.store(true, Ordering::Relaxed);
        writer.join().expect("the writer thread panicked")
    };

    if ready_rx.recv_timeout(STAGE_TIMEOUT).is_err() {
        let result = finish(&stop);
        panic!(
            "stage 1: the writer did not complete a set/clear cycle within {STAGE_TIMEOUT:?} ({result:?})"
        );
    }
    let deadline = Instant::now() + STAGE_TIMEOUT;
    // One read before the loop, so `reads > 0` holds however fast the writer is.
    let mut failure = store.outage().err();
    let mut reads = 1_u32;
    while failure.is_none() && toggles.load(Ordering::Relaxed) < TOGGLES {
        if Instant::now() >= deadline {
            let done = toggles.load(Ordering::Relaxed);
            let result = finish(&stop);
            panic!(
                "stage 2: the writer reached {done} of {TOGGLES} toggles within {STAGE_TIMEOUT:?} ({result:?})"
            );
        }
        if let Err(e) = store.outage() {
            failure = Some(e);
            break;
        }
        reads += 1;
    }
    let done = toggles.load(Ordering::Relaxed);
    if let Err(e) = finish(&stop) {
        panic!("the writer failed after {done} toggles: {e}");
    }
    assert!(
        failure.is_none(),
        "after {reads} reads, {done} toggles: {failure:?}"
    );
    assert!(reads > 0, "the reader ran");
}

/// A stored `attempts` that no longer fits is refused by the store's own
/// validation, as the journal decoder refuses it: `STORE_FAILED`, no source.
#[test]
fn an_attempts_count_past_u32_is_store_failed_without_a_source() {
    let (open, mut store) = fresh("attempts");
    let seq = store
        .insert_entry(entry("k", "b1", None), &ts("2026-10-23T10:00:00Z"))
        .unwrap();
    Connection::open(&open.path)
        .unwrap()
        .execute("UPDATE journal SET attempts = 4294967295", [])
        .unwrap();
    let err = store
        .record_attempt(seq, "x", &ts("2026-10-23T10:00:01Z"))
        .unwrap_err();
    assert_eq!(err.code(), "BD_CACHE.STORE_FAILED", "{err}");
    assert!(
        err.detail()
            .contains(&format!("journal entry {seq}: attempts:")),
        "{}",
        err.detail()
    );
    assert!(
        std::error::Error::source(&err).is_none(),
        "no source: {err:?}"
    );
    let e = store.entry_by_seq(seq).unwrap().unwrap();
    assert_eq!(e.attempts, u32::MAX, "the refused attempt rolled back");
    Connection::open(&open.path)
        .unwrap()
        .execute("UPDATE journal SET attempts = 4294967296", [])
        .unwrap();
    let err = store.entry_by_seq(seq).unwrap_err();
    assert!(
        err.detail()
            .contains(&format!("journal entry {seq}: attempts:")),
        "{}",
        err.detail()
    );
    assert!(
        std::error::Error::source(&err).is_none(),
        "the decoder agrees: {err:?}"
    );
}
