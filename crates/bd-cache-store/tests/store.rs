//! `SqliteStore` cases beyond the contract suite (REQ-BDC-0109, 0604, 0901, 0108).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a failed unwrap or index is a test failure"
)]

mod common;

use bd_cache_model::{CacheStore, DrainLock, IdemKey, NewEntry, Op, Outage, ScopeId, normalize};
use bd_cache_store::{FlockDrainLock, READ_DATING_THRESHOLD, SqliteStore};
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
        changed: Some(true),
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

/// The store reads the outage in one snapshot, stepped deterministically: a
/// fixed number of handshakes, each a committed set and clear by a second
/// (raw) connection followed by a reader snapshot, so the work does not depend
/// on machine speed.
///
/// Strict alternation alone could never interleave, so each handshake also
/// holds the writer inside an open `BEGIN IMMEDIATE` while the reader reads:
/// the reader sees the old record whole, then after the commit the new record
/// whole, never a mix. The free-running variant is
/// `the_outage_is_read_whole_under_a_free_running_soak` (`--ignored`).
#[test]
fn the_outage_is_read_whole_while_another_connection_toggles_it() {
    use std::time::Duration;

    const HANDSHAKES: u32 = 200;
    const SINCE: &str = "2026-10-23T10:00:00Z";
    const SET: &str = "INSERT OR REPLACE INTO meta VALUES \
                       ('outage_since', '2026-10-23T10:00:00Z'), ('outage_error', 'x');";
    const CLEAR: &str = "DELETE FROM meta WHERE key IN ('outage_since', 'outage_error');";

    let (open, store) = fresh("outage-stepped");
    let outage = Outage {
        since: ts(SINCE),
        last_error: "x".to_owned(),
    };
    let raw = Connection::open(&open.path).unwrap();
    raw.busy_timeout(Duration::from_secs(5)).unwrap();
    // Durability does not matter to this race; this removes disk-speed dependence.
    raw.pragma_update(None, "synchronous", "OFF").unwrap();

    for step in 0..HANDSHAKES {
        // Toggle commit, then a reader snapshot.
        raw.execute_batch(&format!("BEGIN IMMEDIATE; {SET} COMMIT;"))
            .unwrap();
        assert_eq!(store.outage().unwrap(), Some(outage.clone()), "step {step}");
        raw.execute_batch(&format!("BEGIN IMMEDIATE; {CLEAR} COMMIT;"))
            .unwrap();
        assert_eq!(store.outage().unwrap(), None, "step {step}");

        // Set held open: the reader still sees the old (absent) record whole.
        raw.execute_batch(&format!("BEGIN IMMEDIATE; {SET}"))
            .unwrap();
        assert_eq!(store.outage().unwrap(), None, "step {step}: set open");
        raw.execute_batch("COMMIT;").unwrap();
        assert_eq!(store.outage().unwrap(), Some(outage.clone()), "step {step}");

        // Clear held open: the reader still sees the old record whole.
        raw.execute_batch(&format!("BEGIN IMMEDIATE; {CLEAR}"))
            .unwrap();
        assert_eq!(
            store.outage().unwrap(),
            Some(outage.clone()),
            "step {step}: clear open"
        );
        raw.execute_batch("COMMIT;").unwrap();
        assert_eq!(store.outage().unwrap(), None, "step {step}");
    }
}

/// The store writes the outage whole, stepped: the store's own `set_outage`
/// runs on a writer thread, one command per handshake, while a raw reader
/// reads continuously until that command finishes. A set or clear split over
/// two transactions would be caught between them. The work is a fixed
/// 200 set/clear pairs; the only wait is the per-command deadline.
#[test]
fn the_store_writes_the_outage_in_one_transaction_per_change() {
    use std::sync::mpsc::{TryRecvError, channel};
    use std::time::{Duration, Instant};

    const HANDSHAKES: u32 = 200;
    const STAGE_TIMEOUT: Duration = Duration::from_secs(60);

    let (open, mut store) = fresh("outage-writes");
    let outage = Outage {
        since: ts("2026-10-23T10:00:00Z"),
        last_error: "x".to_owned(),
    };
    let reader = Connection::open(&open.path).unwrap();
    reader.busy_timeout(Duration::from_secs(5)).unwrap();
    let (cmd_tx, cmd_rx) = channel::<Option<Outage>>();
    let (done_tx, done_rx) = channel::<Result<(), String>>();
    let writer = std::thread::spawn(move || {
        while let Ok(cmd) = cmd_rx.recv() {
            let r = store.set_outage(cmd.as_ref()).map_err(|e| e.to_string());
            if done_tx.send(r).is_err() {
                break;
            }
        }
    });

    // One snapshot of both keys, as the store's own read takes it.
    let snapshot = |step: u32| -> bool {
        let (since, error): (Option<String>, Option<String>) = reader
            .query_row(
                "SELECT (SELECT value FROM meta WHERE key = 'outage_since'), \
                 (SELECT value FROM meta WHERE key = 'outage_error')",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(
            since.is_some(),
            error.is_some(),
            "step {step}: half-written outage: since={since:?} error={error:?}"
        );
        since.is_some()
    };
    let run = |cmd: Option<Outage>, step: u32| {
        let expect = cmd.is_some();
        cmd_tx.send(cmd).unwrap();
        let deadline = Instant::now() + STAGE_TIMEOUT;
        loop {
            snapshot(step);
            match done_rx.try_recv() {
                Ok(r) => {
                    r.unwrap_or_else(|e| panic!("step {step}: the writer failed: {e}"));
                    assert_eq!(snapshot(step), expect, "step {step}: after the command");
                    return;
                }
                Err(TryRecvError::Empty) => assert!(
                    Instant::now() < deadline,
                    "step {step}: the writer did not finish within {STAGE_TIMEOUT:?}"
                ),
                Err(TryRecvError::Disconnected) => panic!("step {step}: the writer thread died"),
            }
        }
    };

    for step in 0..HANDSHAKES {
        run(Some(outage.clone()), step);
        run(None, step);
    }
    drop(cmd_tx);
    writer.join().expect("the writer thread panicked");
}

/// The store's own `outage()` is read whole while a raw writer on another
/// thread free-runs set/clear toggles. The exit is count-based, not
/// wall-clock: the reader runs until it has made `READS` reads and the writer
/// has completed `MIN_TOGGLES` toggles. The hang guard only fails a true hang.
/// Every read must be `Ok` (a split read surfaces as a corrupt-store error).
#[test]
fn the_store_reads_the_outage_whole_while_a_raw_writer_free_runs() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
    use std::time::{Duration, Instant};

    const READS: u32 = 2_000;
    const MIN_TOGGLES: u32 = 200;
    const HANG_GUARD: Duration = Duration::from_secs(120);

    let (open, store) = fresh("outage-reader");
    let path = open.path.clone();
    let stop = Arc::new(AtomicBool::new(false));
    let toggles = Arc::new(AtomicU32::new(0));
    let writer = {
        let (stop, toggles) = (Arc::clone(&stop), Arc::clone(&toggles));
        std::thread::spawn(move || -> rusqlite::Result<()> {
            let c = Connection::open(&path)?;
            c.busy_timeout(Duration::from_secs(5))?;
            c.pragma_update(None, "synchronous", "OFF")?;
            while !stop.load(Ordering::Relaxed) {
                c.execute_batch(
                    "BEGIN IMMEDIATE; INSERT OR REPLACE INTO meta VALUES \
                     ('outage_since', '2026-10-23T10:00:00Z'), ('outage_error', 'x'); COMMIT;",
                )?;
                c.execute_batch(
                    "BEGIN IMMEDIATE; DELETE FROM meta WHERE key IN ('outage_since', 'outage_error'); COMMIT;",
                )?;
                toggles.fetch_add(1, Ordering::Relaxed);
            }
            Ok(())
        })
    };

    let started = Instant::now();
    let mut reads = 0_u32;
    let mut failure = None;
    while reads < READS || toggles.load(Ordering::Relaxed) < MIN_TOGGLES {
        if started.elapsed() > HANG_GUARD {
            failure = Some(format!("hang guard after {reads} reads"));
            break;
        }
        if let Err(e) = store.outage() {
            failure = Some(format!("read {reads}: {e}"));
            break;
        }
        reads += 1;
    }
    stop.store(true, Ordering::Relaxed);
    let done = toggles.load(Ordering::Relaxed);
    let written = writer.join().expect("the writer thread panicked");
    assert!(written.is_ok(), "the writer failed: {written:?}");
    assert!(failure.is_none(), "{failure:?} ({done} toggles)");
}

/// Opt-in stress (`cargo test -- --ignored`): the outage is read whole while a
/// second connection free-runs [`TOGGLES`] set/clear toggles. Speed dependent,
/// so it has its own generous deadline and is not in the default suite.
#[test]
#[ignore = "free-running 2,000-toggle stress; opt in with --ignored"]
fn the_outage_is_read_whole_under_a_free_running_soak() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
    use std::time::{Duration, Instant};

    const TOGGLES: u32 = 2_000;
    const STAGE_TIMEOUT: Duration = Duration::from_secs(600);

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
            c.pragma_update(None, "synchronous", "OFF")?;
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

/// The per-bead index of a ready base (ADR-STORE-0004) lives and dies with the
/// base: a replace swaps it, a prune and a schema rebuild delete it.
#[test]
fn the_ready_base_index_follows_the_base_through_replace_prune_and_rebuild() {
    use bd_cache_model::FingerprintOutcome;
    use sc_beads::{BlockedRow, Fingerprint, Sha256Hex};

    let (open, mut store) = fresh("base-index");
    let indexed = |scope: &str| -> i64 {
        Connection::open(&open.path)
            .unwrap()
            .query_row(
                "SELECT COUNT(*) FROM ready_base_blocked WHERE scope = ?1",
                [scope],
                |r| r.get(0),
            )
            .unwrap()
    };
    let blocked = |bead: &str, by: &str| BlockedRow {
        id: id(bead),
        blocked_by: vec![id(by)],
    };
    store.ensure_scope(&x()).unwrap();
    store.ensure_scope(&y()).unwrap();
    store
        .replace_ready_base(
            &x(),
            &[blocked("a", "b"), blocked("c", "b")],
            &[],
            &ts("2026-10-23T10:00:00Z"),
        )
        .unwrap();
    store
        .replace_ready_base(&y(), &[blocked("a", "z")], &[], &ts("2026-10-23T10:00:00Z"))
        .unwrap();
    assert_eq!((indexed("x"), indexed("y")), (2, 1));
    store
        .replace_ready_base(&x(), &[blocked("a", "b")], &[], &ts("2026-10-23T10:00:05Z"))
        .unwrap();
    assert_eq!(indexed("x"), 1, "a replace swaps the whole index");

    store.prune_scope(&y()).unwrap();
    assert_eq!(
        (indexed("x"), indexed("y")),
        (1, 0),
        "a prune takes its own scope's index only"
    );

    let sha = |c: char| Sha256Hex::new(c.to_string().repeat(64)).unwrap();
    let fp = |c| Fingerprint {
        bd_version: "1.3.1".into(),
        schema_sha256: sha(c),
        schema_version: "1".into(),
    };
    assert_eq!(
        store.record_fingerprint(&fp('a')).unwrap(),
        FingerprintOutcome::Recorded
    );
    assert_eq!(
        store.record_fingerprint(&fp('b')).unwrap(),
        FingerprintOutcome::Rebuilt
    );
    assert_eq!(indexed("x"), 0, "a rebuild drops every base and its index");
}

/// Seeds `n` applied journal rows the way `bd_cache.py` leaves them: state
/// `applied`, no `applied_at`, no epoch, and `updated_at` as it writes it.
fn seed_python_applied(path: &std::path::Path, n: usize, updated_at: &str, tag: &str) {
    // `created_at` is always readable: only the applied time is under test.
    let mut c = Connection::open(path).unwrap();
    let tx = c.transaction().unwrap();
    {
        let mut ins = tx
            .prepare(
                "INSERT INTO journal(idem_key, phase, bead, op, args_json, precondition_json, \
                 actor, state, attempts, created_at, updated_at) \
                 VALUES (?1, 'x', 'b1', 'close', '{}', '{}', 'san', 'applied', 0, \
                 '2026-10-24T07:00:00Z', ?2)",
            )
            .unwrap();
        for i in 0..n {
            ins.execute([format!("{tag}-{i}"), updated_at.to_owned()])
                .unwrap();
        }
    }
    tx.commit().unwrap();
}

/// `has_entries_since` answers from indexes, not from the journal's history
/// (QA2-001): thousands of applied entries older than the base do not make it
/// scan, and the query plan says so.
#[test]
fn the_entries_since_check_uses_indexes_and_ignores_a_long_history() {
    let (open, mut store) = fresh("entries-since");
    {
        let mut c = Connection::open(&open.path).unwrap();
        let tx = c.transaction().unwrap();
        {
            // Entries this store applied: with their epoch.
            let mut ins = tx
                .prepare(
                    "INSERT INTO journal(idem_key, phase, bead, op, args_json, precondition_json, \
                     actor, state, attempts, created_at, updated_at, applied_at, applied_epoch) \
                     VALUES (?1, 'x', 'b1', 'close', '{}', '{}', 'san', 'applied', 0, \
                     '2026-10-01T00:00:00Z', '2026-10-01T00:00:00Z', '2026-10-01T00:00:00Z', 1790812800)",
                )
                .unwrap();
            for i in 0..5_000 {
                ins.execute([format!("old-{i}")]).unwrap();
            }
        }
        tx.commit().unwrap();
    }
    assert!(
        !store
            .has_entries_since(&ts("2026-10-23T10:00:00Z"))
            .unwrap(),
        "old applied entries are not 'since'"
    );
    assert!(
        store
            .has_entries_since(&ts("2026-10-01T00:00:00Z"))
            .unwrap()
    );

    let c = Connection::open(&open.path).unwrap();
    let plan: Vec<String> = c
        .prepare(&format!(
            "EXPLAIN QUERY PLAN {}",
            bd_cache_store::HAS_ENTRIES_SINCE_SQL
        ))
        .unwrap()
        .query_map([1_792_749_600_i64], |r| r.get::<_, String>(3))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    // The third lookup walks `journal_applied_unknown`, which holds only the
    // entries applied without an epoch; every journal access is through an index.
    assert!(
        plan.iter()
            .filter(|d| d.contains("journal"))
            .all(|d| d.contains("USING")),
        "{plan:?}"
    );
    let plan = plan.join(" | ");
    for index in [
        "journal_pending",
        "journal_applied_epoch",
        "journal_applied_unknown",
    ] {
        assert!(plan.contains(index), "{index} not in {plan}");
    }
    // And a pending entry still counts, whatever the time.
    store.ensure_scope(&x()).unwrap();
    store
        .insert_entry(entry("fresh", "b9", Some(x())), &ts("2026-10-23T10:00:00Z"))
        .unwrap();
    assert!(
        store
            .has_entries_since(&ts("2030-01-01T00:00:00Z"))
            .unwrap()
    );
}

/// Entries `bd_cache.py` applied carry no `applied_at` and no epoch; their
/// time is `updated_at`, written as `...Z` by it and readable as `+00:00` with
/// fractions too. Before and after they are dated (by the check or the next
/// open), an old one is not "since", a recent one is, and one with an unreadable time counts as since:
/// the base must never be served without a write Python applied (S18).
#[test]
fn entries_bd_cache_py_applied_count_by_their_updated_at_and_unreadable_ones_count_as_since() {
    use bd_cache_model::CacheStore as _;
    let (open, store) = fresh("python-applied");
    let at = ts("2026-10-24T09:00:00Z");
    seed_python_applied(&open.path, 3, "2026-10-24T08:00:00Z", "z-old");
    assert!(
        !store.has_entries_since(&at).unwrap(),
        "applied an hour before the base: not since"
    );
    assert!(store.applied_since(&at).unwrap().is_empty());

    seed_python_applied(&open.path, 1, "2026-10-24T09:00:01Z", "z-new");
    assert!(
        store.has_entries_since(&at).unwrap(),
        "applied after the base"
    );
    assert_eq!(store.applied_since(&at).unwrap().len(), 1);
    assert!(store.applied_since(&at).unwrap()[0].applied_at.is_none());

    // The other format seen in the file: offset and fractions.
    let (open2, store2) = fresh("python-applied-offset");
    seed_python_applied(&open2.path, 1, "2026-10-24T08:59:59.500+00:00", "off-old");
    assert!(!store2.has_entries_since(&at).unwrap());
    seed_python_applied(&open2.path, 1, "2026-10-24T09:00:00.250+00:00", "off-new");
    assert!(
        store2.has_entries_since(&at).unwrap(),
        "same second counts (inclusive)"
    );

    // A time nobody can read: conservative, forever.
    let (open3, store3) = fresh("python-applied-garbage");
    seed_python_applied(&open3.path, 1, "yesterday-ish", "bad");
    assert!(store3.has_entries_since(&at).unwrap());
    assert_eq!(store3.applied_since(&at).unwrap().len(), 1);
    drop((store, store2, store3));

    // Reopening dates the readable ones (no more scanning for them) and still
    // answers the same.
    let reopened = SqliteStore::open_at(&open.path, &identity()).unwrap();
    let undated: i64 = Connection::open(&open.path)
        .unwrap()
        .query_row(
            "SELECT COUNT(*) FROM journal WHERE state = 'applied' AND applied_epoch IS NULL",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(undated, 0, "the open dated them");
    assert!(
        !reopened
            .has_entries_since(&ts("2026-10-24T09:00:02Z"))
            .unwrap()
    );
    assert!(reopened.has_entries_since(&at).unwrap());
    let reopened3 = SqliteStore::open_at(&open3.path, &identity()).unwrap();
    assert!(
        reopened3.has_entries_since(&at).unwrap(),
        "still counted after the open"
    );
}

/// The shared file stays writable by a `bd_cache.py` on an SQLite older than
/// 3.38, which has no `unixepoch()` (Ubuntu 22.04 ships 3.37.2) (S18): no index
/// holds an expression, and with `unixepoch` replaced by a function that
/// fails, every kind of journal write still works.
#[test]
fn no_index_holds_a_function_and_journal_writes_work_without_unixepoch() {
    let (open, mut store) = fresh("no-functions");
    store.ensure_scope(&x()).unwrap();
    store
        .insert_entry(entry("k", "b1", Some(x())), &ts("2026-10-24T08:00:00Z"))
        .unwrap();
    drop(store);
    let c = Connection::open(&open.path).unwrap();
    let expression_columns: i64 = c
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master m, pragma_index_xinfo(m.name) x \
             WHERE m.type = 'index' AND x.cid = -2",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(expression_columns, 0, "an index holds an expression");
    let mentions: i64 = c
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND sql LIKE '%unixepoch%'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(mentions, 0);

    // The stand-in for an old SQLite: a `unixepoch` that fails when called.
    c.create_scalar_function(
        "unixepoch",
        -1,
        rusqlite::functions::FunctionFlags::SQLITE_UTF8,
        |_| -> rusqlite::Result<i64> {
            Err(rusqlite::Error::UserFunctionError(
                "no unixepoch here".into(),
            ))
        },
    )
    .unwrap();
    c.execute(
        "INSERT INTO journal(idem_key, phase, bead, op, args_json, precondition_json, actor, \
         state, attempts, created_at, updated_at) \
         VALUES ('py', 'x', 'b2', 'close', '{}', '{}', 'san', 'pending', 0, 't', 't')",
        [],
    )
    .unwrap();
    c.execute(
        "UPDATE journal SET state = 'applied', updated_at = 'u' WHERE idem_key = 'py'",
        [],
    )
    .unwrap();
    c.execute("DELETE FROM journal WHERE idem_key = 'py'", [])
        .unwrap();

    // Negative control: with the earlier build's expression index back, the
    // same old-SQLite writer can no longer mark an entry applied.
    Connection::open(&open.path)
        .unwrap()
        .execute(
            "CREATE INDEX journal_applied_at ON journal(unixepoch(applied_at)) WHERE state = 'applied'",
            [],
        )
        .unwrap();
    // The stand-in is declared deterministic here so the schema loads and
    // the failure is the index's own: the pending insert is outside the
    // partial index, the mark to `applied` must evaluate it.
    let old = Connection::open(&open.path).unwrap();
    old.create_scalar_function(
        "unixepoch",
        -1,
        rusqlite::functions::FunctionFlags::SQLITE_UTF8
            | rusqlite::functions::FunctionFlags::SQLITE_DETERMINISTIC,
        |_| -> rusqlite::Result<i64> {
            Err(rusqlite::Error::UserFunctionError(
                "no unixepoch here".into(),
            ))
        },
    )
    .unwrap();
    old.execute(
        "INSERT INTO journal(idem_key, phase, bead, op, args_json, precondition_json, actor, \
         state, attempts, created_at, updated_at) \
         VALUES ('py2', 'x', 'b2', 'close', '{}', '{}', 'san', 'pending', 0, 't', 't')",
        [],
    )
    .unwrap();
    let err = old
        .execute(
            "UPDATE journal SET state = 'applied', updated_at = 'u' WHERE idem_key = 'py2'",
            [],
        )
        .unwrap_err();
    assert!(err.to_string().contains("no unixepoch here"), "{err}");
}

/// `INDEXED BY` is an error when another writer dropped the index: the check
/// falls back to the unhinted query and stays correct (QA3-007), and keeps
/// to it without trying the hints again (QA4-005).
#[test]
fn the_entries_since_check_survives_a_dropped_index() {
    let (open, store) = fresh("dropped-index");
    seed_python_applied(&open.path, 1, "2026-10-24T09:00:01Z", "late");
    let at = ts("2026-10-24T09:00:00Z");
    assert!(store.has_entries_since(&at).unwrap());
    let c = Connection::open(&open.path).unwrap();
    c.execute_batch("DROP INDEX journal_applied_epoch; DROP INDEX journal_applied_unknown;")
        .unwrap();
    assert!(
        store.has_entries_since(&at).unwrap(),
        "still true without the indexes"
    );
    assert!(
        !store
            .has_entries_since(&ts("2026-10-24T09:00:02Z"))
            .unwrap()
    );
    assert_eq!(store.applied_since(&at).unwrap().len(), 1);

    // Once missing, the hints are not tried again (QA4-005): indexes of the
    // same names that no hinted query can use make `INDEXED BY` fail with
    // another error, and the store still answers, call after call.
    c.execute_batch(
        "CREATE INDEX journal_applied_epoch ON journal(applied_epoch) WHERE state = 'never'; \
         CREATE INDEX journal_applied_unknown ON journal(seq) WHERE state = 'never';",
    )
    .unwrap();
    let trap = c
        .prepare(bd_cache_store::HAS_ENTRIES_SINCE_SQL)
        .map(|_| ())
        .unwrap_err();
    assert!(
        !trap.to_string().contains("no such index"),
        "control: {trap}"
    );
    seed_python_applied(&open.path, 1, "2026-10-24T09:00:03Z", "later");
    for _ in 0..3 {
        assert!(store.has_entries_since(&at).unwrap());
        assert!(
            !store
                .has_entries_since(&ts("2026-10-24T09:00:04Z"))
                .unwrap()
        );
        assert_eq!(store.applied_since(&at).unwrap().len(), 2);
    }
    c.execute_batch("DROP INDEX journal_applied_epoch; DROP INDEX journal_applied_unknown;")
        .unwrap();
    drop(store);
    let again = SqliteStore::open_at(&open.path, &identity()).unwrap();
    let back: i64 = c
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE name IN ('journal_applied_epoch', 'journal_applied_unknown')",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(back, 2, "the open recreates them");
    assert!(again.has_entries_since(&at).unwrap());
}

/// The shared file used to carry an expression index on `unixepoch()`; a file
/// an earlier build made loses it when this store opens it.
#[test]
fn the_expression_index_of_an_earlier_build_is_dropped_on_open() {
    let (open, store) = fresh("drop-expression-index");
    drop(store);
    Connection::open(&open.path)
        .unwrap()
        .execute(
            "CREATE INDEX journal_applied_at ON journal(unixepoch(applied_at)) WHERE state = 'applied'",
            [],
        )
        .unwrap();
    let _store = SqliteStore::open_at(&open.path, &identity()).unwrap();
    let left: i64 = Connection::open(&open.path)
        .unwrap()
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE name = 'journal_applied_at'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(left, 0);
}

/// `bd_cache.py` (atm-core `05831df`) never creates, reads or writes `ready_base`,
/// `ready_base_blocked` or `scope_member`: its rebuild deletes `edges`, `beads`
/// and `sync_state`, and its prune deletes a phase's `edges`, `beads`,
/// `journal` and `sync_state` rows. Neither touches a stored base or its index
/// (ADR-STORE-0004), so after them `base_entry` still equals the whole base.
#[test]
fn a_python_style_rebuild_or_prune_leaves_the_base_and_its_index_in_agreement() {
    use bd_cache_model::BaseEntry;
    use sc_beads::BlockedRow;

    let (open, mut store) = fresh("python-rewrite");
    store.ensure_scope(&x()).unwrap();
    let blocked = [BlockedRow {
        id: id("a"),
        blocked_by: vec![id("b"), id("c")],
    }];
    store
        .replace_ready_base(&x(), &blocked, &[id("d")], &ts("2026-10-23T10:00:00Z"))
        .unwrap();
    let agree = |store: &SqliteStore| {
        let base = store.ready_base(&x()).unwrap().unwrap();
        for bead in ["a", "d", "nobody"] {
            assert_eq!(
                store.base_entry(&x(), &id(bead)).unwrap().unwrap(),
                BaseEntry::of(&base, &id(bead)),
                "{bead}"
            );
        }
    };
    agree(&store);
    let c = Connection::open(&open.path).unwrap();
    // bd_cache.py's rebuild, then its prune of phase x.
    c.execute_batch(
        "DELETE FROM edges; DELETE FROM beads; DELETE FROM sync_state; \
         DELETE FROM journal WHERE phase = 'x';",
    )
    .unwrap();
    agree(&store);
}

/// A base another writer replaces with `INSERT OR REPLACE` (the way every
/// `bd_cache.py` write is spelled, and what a future Python `ready_base` writer
/// would do), without the `indexed` column, resets `indexed` to NULL: the
/// index rows left behind are ignored and the base is read whole.
#[test]
fn a_base_replaced_by_another_writer_is_read_from_its_blob_not_from_a_stale_index() {
    use sc_beads::BlockedRow;

    let (open, mut store) = fresh("python-base");
    store.ensure_scope(&x()).unwrap();
    store
        .replace_ready_base(
            &x(),
            &[BlockedRow {
                id: id("a"),
                blocked_by: vec![id("b")],
            }],
            &[],
            &ts("2026-10-23T10:00:00Z"),
        )
        .unwrap();
    assert_eq!(
        store
            .base_entry(&x(), &id("a"))
            .unwrap()
            .unwrap()
            .blocked_by,
        [id("b")]
    );
    let c = Connection::open(&open.path).unwrap();
    c.execute(
        "INSERT OR REPLACE INTO ready_base(scope, blocked_json, ready_json, fetched_at, stale) \
         VALUES ('x', '[{\"id\": \"a\", \"blocked_by\": [\"z\"]}]', '[]', '2026-10-23T10:05:00Z', 0)",
        [],
    )
    .unwrap();
    let indexed: Option<i64> = c
        .query_row(
            "SELECT indexed FROM ready_base WHERE scope = 'x'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(indexed, None, "a replace without the column resets it");
    let entry = store.base_entry(&x(), &id("a")).unwrap().unwrap();
    assert_eq!(
        entry.blocked_by,
        [id("z")],
        "the blob wins over the stale index"
    );
    assert_eq!(entry.fetched_at.as_str(), "2026-10-23T10:05:00Z");
    // The next Rust replace indexes again and drops the stale rows.
    store
        .replace_ready_base(&x(), &[], &[], &ts("2026-10-23T10:10:00Z"))
        .unwrap();
    let left: i64 = c
        .query_row("SELECT COUNT(*) FROM ready_base_blocked", [], |r| r.get(0))
        .unwrap();
    assert_eq!(left, 0);
    assert!(
        store
            .base_entry(&x(), &id("a"))
            .unwrap()
            .unwrap()
            .blocked_by
            .is_empty()
    );
}

/// A file written before the index existed (no `ready_base_blocked` table, no
/// `indexed` column) opens, its base reads whole, and the next replace migrates it.
#[test]
fn a_file_from_before_the_ready_base_index_opens_and_migrates_on_the_next_replace() {
    use sc_beads::BlockedRow;

    let (open, store) = fresh("pre-index");
    drop(store);
    {
        let c = Connection::open(&open.path).unwrap();
        c.execute_batch(
            "DROP TABLE ready_base_blocked; ALTER TABLE ready_base DROP COLUMN indexed; \
             INSERT INTO scope(name) VALUES ('x'); \
             INSERT INTO ready_base(scope, blocked_json, ready_json, fetched_at, stale) \
             VALUES ('x', '[{\"id\": \"a\", \"blocked_by\": [\"b\", \"b\"]}]', '[\"d\"]', \
             '2026-10-23T10:00:00Z', 0);",
        )
        .unwrap();
    }
    let mut store = SqliteStore::open_at(&open.path, &identity()).unwrap();
    let old = store.base_entry(&x(), &id("a")).unwrap().unwrap();
    assert_eq!(old.blocked_by, [id("b")], "read whole, repeats merged");
    store
        .replace_ready_base(
            &x(),
            &[BlockedRow {
                id: id("a"),
                blocked_by: vec![id("c")],
            }],
            &[],
            &ts("2026-10-23T10:01:00Z"),
        )
        .unwrap();
    let c = Connection::open(&open.path).unwrap();
    let indexed: Option<i64> = c
        .query_row(
            "SELECT indexed FROM ready_base WHERE scope = 'x'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(indexed, Some(1));
    assert_eq!(
        store
            .base_entry(&x(), &id("a"))
            .unwrap()
            .unwrap()
            .blocked_by,
        [id("c")]
    );
}

/// The SQLite virtual-machine steps one run of `sql` takes on `c`: the SQL
/// work, independent of the machine's speed.
fn vm_steps(c: &Connection, sql: &str, args: impl rusqlite::Params) -> i32 {
    let mut stmt = c.prepare(sql).unwrap();
    let mut rows = stmt.query(args).unwrap();
    while rows.next().unwrap().is_some() {}
    drop(rows);
    stmt.get_status(rusqlite::StatementStatus::VmStep)
}

fn undated(path: &std::path::Path) -> i64 {
    Connection::open(path)
        .unwrap()
        .query_row(
            "SELECT COUNT(*) FROM journal WHERE state = 'applied' AND applied_epoch IS NULL",
            [],
            |r| r.get(0),
        )
        .unwrap()
}

/// [`READ_DATING_THRESHOLD`] as a count of rows to seed.
fn threshold() -> usize {
    usize::try_from(READ_DATING_THRESHOLD).unwrap()
}

/// `PRAGMA data_version` of `c`: it changes when another connection commits.
fn data_version(c: &Connection) -> i64 {
    c.query_row("PRAGMA data_version", [], |r| r.get(0))
        .unwrap()
}

/// Entries `bd_cache.py` applies while the store is open have no epoch; past
/// [`READ_DATING_THRESHOLD`] the next journal check dates them, so its SQL
/// work goes back to a few index probes instead of one `unixepoch` per
/// undated entry (QA4-001, S26). Measured in SQLite VM steps, never wall-clock.
#[test]
fn entries_python_applies_while_open_are_dated_by_the_next_check() {
    let (open, store) = fresh("dated-while-open");
    let at = ts("2026-10-24T09:00:00Z");
    assert!(!store.has_entries_since(&at).unwrap());
    let c = Connection::open(&open.path).unwrap();
    let empty = vm_steps(&c, bd_cache_store::HAS_ENTRIES_SINCE_SQL, [at.unix()]);

    seed_python_applied(&open.path, 20_000, "2026-10-24T08:00:00Z", "py");
    let walking = vm_steps(&c, bd_cache_store::HAS_ENTRIES_SINCE_SQL, [at.unix()]);
    assert!(
        walking > 20_000,
        "control: undated entries are walked one by one ({walking} steps)"
    );

    assert!(
        !store.has_entries_since(&at).unwrap(),
        "old entries are not since"
    );
    assert_eq!(undated(&open.path), 0, "the check dated them");
    let after = vm_steps(&c, bd_cache_store::HAS_ENTRIES_SINCE_SQL, [at.unix()]);
    assert!(
        after <= empty + 10,
        "dated entries cost index probes only: {after} steps, {empty} with none"
    );
    let since = vm_steps(&c, bd_cache_store::APPLIED_SINCE_SQL, [at.unix()]);
    assert!(since < 100, "applied_since reads no history: {since} steps");

    // A few recent ones, below the threshold, are walked, not dated (S26),
    // and still count, in both reads.
    seed_python_applied(&open.path, 2, "2026-10-24T09:00:01Z", "new");
    assert!(store.has_entries_since(&at).unwrap());
    seed_python_applied(&open.path, 1, "2026-10-24T09:00:02Z", "newer");
    assert_eq!(store.applied_since(&at).unwrap().len(), 3);
    assert_eq!(undated(&open.path), 3, "below the threshold: not dated");
}

/// Below [`READ_DATING_THRESHOLD`] the journal reads write nothing (S26): a
/// small burst `bd_cache.py` applied is walked, so a cached `get` has no WAL
/// commit on its path; one entry more and the next read dates them all.
#[test]
fn a_small_burst_is_walked_and_not_dated_until_past_the_threshold() {
    let (open, store) = fresh("dating-threshold");
    let at = ts("2026-10-24T09:00:00Z");
    seed_python_applied(&open.path, threshold() - 1, "2026-10-24T08:00:00Z", "old");
    seed_python_applied(&open.path, 1, "2026-10-24T09:00:01Z", "new");
    let c = Connection::open(&open.path).unwrap();
    let before = data_version(&c);
    for _ in 0..3 {
        assert!(store.has_entries_since(&at).unwrap());
        assert!(
            !store
                .has_entries_since(&ts("2026-10-24T09:00:02Z"))
                .unwrap()
        );
        assert_eq!(store.applied_since(&at).unwrap().len(), 1);
    }
    assert_eq!(data_version(&c), before, "the reads committed nothing");
    assert_eq!(undated(&open.path), i64::from(READ_DATING_THRESHOLD));

    seed_python_applied(&open.path, 1, "2026-10-24T08:00:00Z", "one-more");
    assert!(store.has_entries_since(&at).unwrap(), "the same answer");
    assert_eq!(undated(&open.path), 0, "past the threshold: dated");
    assert!(
        !store
            .has_entries_since(&ts("2026-10-24T09:00:02Z"))
            .unwrap()
    );
    assert_eq!(store.applied_since(&at).unwrap().len(), 1);
}

/// An entry with an unreadable time, applied after readable undated ones, does
/// not hide them from the count (QA5-002, S26): once more arrive past the
/// threshold, all the readable ones are dated and only the unreadable stays.
#[test]
fn an_unreadable_entry_does_not_hide_earlier_undated_ones_from_the_dating() {
    let (open, store) = fresh("dating-floor");
    let at = ts("2026-10-24T09:00:00Z");
    let half = threshold() / 2;
    seed_python_applied(&open.path, half, "2026-10-24T08:00:00Z", "early");
    assert!(!store.has_entries_since(&at).unwrap());
    seed_python_applied(&open.path, 1, "garbage", "bad");
    assert!(store.has_entries_since(&at).unwrap(), "unreadable counts");
    assert_eq!(undated(&open.path), i64::try_from(half).unwrap() + 1);
    seed_python_applied(&open.path, half + 1, "2026-10-24T08:00:00Z", "late");
    assert!(store.has_entries_since(&at).unwrap());
    assert_eq!(undated(&open.path), 1, "every readable one dated");
    assert_eq!(store.applied_since(&at).unwrap().len(), 1);
}

/// A writer holding the write lock does not make the check wait: past the
/// threshold the dating is tried with no busy timeout, skipped, and the
/// answer is the same, from the undated entries. (Had the reads waited, the
/// undated count would be 0: the lock is held until the rollback.)
#[test]
fn the_check_does_not_wait_for_the_write_lock_to_date_entries() {
    let (open, store) = fresh("dating-busy");
    let at = ts("2026-10-24T09:00:00Z");
    seed_python_applied(&open.path, threshold(), "2026-10-24T08:00:00Z", "old");
    seed_python_applied(&open.path, 1, "2026-10-24T09:00:01Z", "new");
    let mut c = Connection::open(&open.path).unwrap();
    let tx = c
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .unwrap();
    assert!(store.has_entries_since(&at).unwrap());
    assert!(
        !store
            .has_entries_since(&ts("2026-10-24T09:00:02Z"))
            .unwrap()
    );
    assert_eq!(store.applied_since(&at).unwrap().len(), 1);
    assert_eq!(
        undated(&open.path),
        i64::from(READ_DATING_THRESHOLD) + 1,
        "nothing dated while the lock is held"
    );
    tx.rollback().unwrap();
    assert!(store.has_entries_since(&at).unwrap());
    assert_eq!(undated(&open.path), 0, "dated once the lock is free");
}

/// A bounded marker write gives up on a write lock another connection holds
/// and writes nothing (lead ruling S33): the escalation stays unannounced and
/// the alert unset, so the next tick sends the message again. Causal: the
/// holder keeps the lock until both writes have returned, so their failure is
/// the held lock's (`database is locked`), not a race with its release. No
/// clock check (FTQ-029): that each write waits at most its wait, and that the
/// busy timeout is back after it, is the unit test's causal probe
/// (`a_bounded_marker_write_waits_at_most_its_wait_and_restores_the_timeout`);
/// the 500 ms hold this replaces is gone (FTQ-022).
#[test]
fn a_bounded_marker_write_gives_up_on_a_held_lock_within_its_wait() {
    let (open, mut store) = fresh("bounded-marker");
    let at = ts("2026-10-24T09:00:00Z");
    let seq = store.insert_entry(entry("k1", "b1", None), &at).unwrap();
    store.escalate(seq, "gone", &at).unwrap();
    let mut c = Connection::open(&open.path).unwrap();
    let tx = c
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .unwrap();

    let wait = std::time::Duration::from_millis(50);
    let marked = store.mark_announced_within(seq, &at, wait);
    let alerted = store.set_sync_alert_within(&x(), Some("cause"), wait);
    // Released only now, after both writes returned.
    tx.rollback().unwrap();

    for (what, result) in [("mark", marked), ("alert", alerted)] {
        let err = result.expect_err(what);
        assert_eq!(err.code(), "BD_CACHE.STORE_FAILED", "{what}: {err:?}");
        assert!(err.detail().contains("locked"), "{what}: {}", err.detail());
    }
    assert_eq!(
        store.unannounced_escalations().unwrap().len(),
        1,
        "not marked: sent again next tick"
    );
    assert_eq!(store.sync_alert(&x()).unwrap(), None);

    // With the lock free, the same bounded writes succeed (the next tick).
    store.mark_announced_within(seq, &at, wait).unwrap();
    store
        .set_sync_alert_within(&x(), Some("cause"), wait)
        .unwrap();
    assert!(store.unannounced_escalations().unwrap().is_empty());
    assert_eq!(store.sync_alert(&x()).unwrap().as_deref(), Some("cause"));
}

/// `applied_since` reads two partial-index ranges joined by `UNION ALL`;
/// an `OR` in one `WHERE` would let neither index serve it (QA4-003).
#[test]
fn applied_since_reads_both_partial_indexes_and_no_history() {
    let (open, mut store) = fresh("applied-since-plan");
    {
        let mut c = Connection::open(&open.path).unwrap();
        let tx = c.transaction().unwrap();
        {
            let mut ins = tx
                .prepare(
                    "INSERT INTO journal(idem_key, phase, bead, op, args_json, precondition_json, \
                     actor, state, attempts, created_at, updated_at, applied_at, applied_epoch) \
                     VALUES (?1, 'x', 'b1', 'close', '{}', '{}', 'san', 'applied', 0, \
                     '2026-10-01T00:00:00Z', '2026-10-01T00:00:00Z', '2026-10-01T00:00:00Z', 1790812800)",
                )
                .unwrap();
            for i in 0..5_000 {
                ins.execute([format!("old-{i}")]).unwrap();
            }
        }
        tx.commit().unwrap();
    }
    let c = Connection::open(&open.path).unwrap();
    let plan: Vec<String> = c
        .prepare(&format!(
            "EXPLAIN QUERY PLAN {}",
            bd_cache_store::APPLIED_SINCE_SQL
        ))
        .unwrap()
        .query_map([1_792_749_600_i64], |r| r.get::<_, String>(3))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert!(
        plan.iter()
            .filter(|d| d.contains("journal"))
            .all(|d| d.contains("USING")),
        "{plan:?}"
    );
    let joined = plan.join(" | ");
    for index in ["journal_applied_epoch", "journal_applied_unknown"] {
        assert!(joined.contains(index), "{index} not in {joined}");
    }
    let at = ts("2026-10-23T10:00:00Z");
    let steps = vm_steps(&c, bd_cache_store::APPLIED_SINCE_SQL, [at.unix()]);
    assert!(steps < 100, "5000 old entries are not read: {steps} steps");
    let unhinted = vm_steps(&c, bd_cache_store::APPLIED_SINCE_UNHINTED_SQL, [at.unix()]);
    assert!(
        unhinted > 5_000,
        "control: the unhinted form walks them ({unhinted})"
    );

    // Both halves answer, in seq order.
    store.ensure_scope(&x()).unwrap();
    let mine = store
        .insert_entry(entry("mine", "b2", Some(x())), &at)
        .unwrap();
    seed_python_applied(&open.path, 1, "garbage", "unreadable");
    store
        .apply_with_readback(mine, None, None, &ts("2026-10-24T00:00:00Z"))
        .unwrap();
    let got = store.applied_since(&at).unwrap();
    let keys: Vec<&str> = got.iter().map(|e| e.idem_key.as_str()).collect();
    assert_eq!(keys, ["mine", "unreadable-0"], "seq order, both halves");
    assert!(got[0].seq < got[1].seq);
}

/// A pending entry journaled before PE-RBP-F004 (an earlier build, or
/// `bd_cache.py`) may carry a metadata key a new write is refused for. It must
/// still decode, so the drain reaches it and escalates it, instead of every
/// pending read failing with `BAD_ARGS` (QA RCR-001 on #60).
#[test]
fn a_stored_entry_with_a_metadata_key_new_writes_refuse_still_decodes() {
    let (open, store) = fresh("legacy-meta-key");
    {
        let c = Connection::open(&open.path).unwrap();
        let mut ins = c
            .prepare(
                "INSERT INTO journal(idem_key, phase, bead, op, args_json, precondition_json, \
                 actor, state, attempts, created_at, updated_at) \
                 VALUES (?1, 'x', 'b1', 'update', ?2, '{}', 'san', 'pending', 0, \
                 '2026-10-24T07:00:00Z', '2026-10-24T07:00:00Z')",
            )
            .unwrap();
        ins.execute(["legacy-eq", r#"{"set_metadata": {"a=b": "x"}}"#])
            .unwrap();
        ins.execute(["legacy-empty", r#"{"set_metadata": {"": "x"}}"#])
            .unwrap();
    }
    let pending = store.pending_entries().unwrap();
    assert_eq!(pending.len(), 2, "both legacy entries decode");
    for e in &pending {
        let Op::Update(patch) = &e.op else {
            panic!("an update, got {:?}", e.op)
        };
        assert_eq!(patch.set_metadata.len(), 1);
    }
}

/// `last_applied_at` is two index reads that walk no history (RCR-001 of
/// #91): thousands of old applied entries are not read, the plan names the
/// indexes, and the answer does not depend on how many rows the journal has
/// or on the seqs they carry.
#[test]
fn last_applied_at_reads_the_applied_indexes_and_ignores_history_and_seqs() {
    let (open, mut store) = fresh("last-applied-at");
    {
        let mut c = Connection::open(&open.path).unwrap();
        let tx = c.transaction().unwrap();
        {
            let mut ins = tx
                .prepare(
                    "INSERT INTO journal(idem_key, phase, bead, op, args_json, precondition_json, \
                     actor, state, attempts, created_at, updated_at, applied_at, applied_epoch) \
                     VALUES (?1, 'x', 'b1', 'close', '{}', '{}', 'san', 'applied', 0, \
                     '2026-10-01T00:00:00Z', '2026-10-01T00:00:00Z', '2026-10-01T00:00:00Z', 1790812800)",
                )
                .unwrap();
            for i in 0..5_000 {
                ins.execute([format!("old-{i}")]).unwrap();
            }
        }
        tx.commit().unwrap();
    }
    let c = Connection::open(&open.path).unwrap();
    for sql in [
        bd_cache_store::NEWEST_APPLIED_EPOCH_SQL,
        bd_cache_store::NEWEST_UNDATED_APPLIED_SQL,
    ] {
        let plan: Vec<String> = c
            .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
            .unwrap()
            .query_map([], |r| r.get::<_, String>(3))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert!(
            plan.iter()
                .filter(|d| d.contains("journal"))
                .all(|d| d.contains("USING")),
            "{plan:?}"
        );
        let steps = vm_steps(&c, sql, []);
        assert!(steps < 100, "5000 old entries are not read: {steps} steps");
    }
    // Control: both unhinted forms answer the same on this file.
    for sql in [
        bd_cache_store::NEWEST_APPLIED_EPOCH_UNHINTED_SQL,
        bd_cache_store::NEWEST_UNDATED_APPLIED_UNHINTED_SQL,
    ] {
        assert!(vm_steps(&c, sql, []) > 0);
    }
    assert_eq!(
        store.last_applied_at().unwrap(),
        Some(ts("2026-10-01T00:00:00Z")),
        "the seeded history is the newest applied time"
    );

    // A newer one wins whatever its seq, and a python-applied entry (no
    // epoch) counts by its `updated_at`.
    store.ensure_scope(&x()).unwrap();
    let mine = store
        .insert_entry(entry("mine", "b2", Some(x())), &ts("2026-10-23T10:00:00Z"))
        .unwrap();
    store
        .apply_with_readback(mine, None, None, &ts("2026-10-24T00:00:00Z"))
        .unwrap();
    assert_eq!(
        store.last_applied_at().unwrap(),
        Some(ts("2026-10-24T00:00:00Z"))
    );
    seed_python_applied(&open.path, 1, "2026-10-25T00:00:00Z", "py-new");
    seed_python_applied(&open.path, 1, "garbage", "py-bad");
    assert_eq!(
        store.last_applied_at().unwrap(),
        Some(ts("2026-10-25T00:00:00Z")),
        "undated entries count by updated_at; an unreadable one is skipped"
    );
}
