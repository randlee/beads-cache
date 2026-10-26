//! The cost of an in-process cached `get` on the real `SqliteStore`
//! (NFR-BDC-0001: under 1 ms).
//!
//! The gate is deterministic: the number of store calls and items a cached
//! `get` touches is the same at 10, 100 and 1000 beads, so its cost does not
//! grow with the scope. The wall-clock median is printed for information
//! (`--ignored --nocapture`); it does not gate, because a shared CI runner can
//! stall any one call longer than the whole budget. `benches/get.rs` measures
//! the same with criterion. Release-profile figures are in the sprint doc.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a failed unwrap or index is a test failure"
)]

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use bd_cache::Cache;
use bd_cache_model::{CacheConfig, ScopeId};
use bd_cache_store::{DatabaseIdentity, DoltDatabase, FlockDrainLock, ProjectId, SqliteStore};
use common::counting::Counting;
use sc_beads::testing::{FakeBackend, ManualClock, bead};
use sc_beads::{Clock, DepType, ProjectRef};

/// How many calls the informational median is taken over.
const CALLS: usize = 1000;

type PerfCache = Cache<Counting<SqliteStore>, FlockDrainLock, FakeBackend>;

/// A cache over a fresh `SqliteStore` holding `beads` synced beads in scope x;
/// every tenth bead depends on its neighbour, so the ready base has blocked rows.
fn synced_cache(beads: usize) -> (tempfile::TempDir, PerfCache, Arc<AtomicU64>) {
    let dir = tempfile::tempdir().unwrap();
    let identity = DatabaseIdentity {
        dolt_database: DoltDatabase::new("perf").unwrap(),
        project_id: ProjectId::new("p-perf").unwrap(),
    };
    let store = SqliteStore::open_at(&dir.path().join("perf.sqlite3"), &identity).unwrap();
    let lock = store.drain_lock();
    let (store, touched) = Counting::new(store);
    let clock = Arc::new(ManualClock::default());
    let bd = FakeBackend::new(Arc::clone(&clock));
    let x = ScopeId::new("x").unwrap();
    for i in 0..beads {
        let mut b = bead(&format!("b{i}"));
        b.labels = vec!["phase-x".into()];
        bd.insert(b);
    }
    for i in (0..beads.saturating_sub(1)).step_by(10) {
        bd.add_edge(&format!("b{i}"), &format!("b{}", i + 1), DepType::Blocks);
    }
    let mut cache = Cache::new(
        CacheConfig::new(
            ProjectRef::local(None),
            vec![bd_cache_model::Scope::legacy(x.clone())],
        ),
        store,
        lock,
        bd,
        Arc::clone(&clock) as Arc<dyn Clock>,
    )
    .unwrap();
    let report = cache
        .sync(
            &x,
            sc_beads::Deadline::after(&*clock, Duration::from_secs(20)),
        )
        .unwrap();
    assert_eq!(usize::try_from(report.stored).unwrap(), beads);
    (dir, cache, touched)
}

/// What one cached `get` of a blocked bead (`b0` is blocked by `b1`) touches.
fn touched_by_one_get(beads: usize) -> u64 {
    let (_dir, mut cache, touched) = synced_cache(beads);
    let id = sc_beads::testing::id("b0");
    cache.get(&id, None).unwrap(); // warm: nothing to fill, nothing cached lazily
    touched.store(0, Ordering::SeqCst);
    let reply = cache.get(&id, None).unwrap();
    assert_eq!(
        reply.row.blocked,
        Some(vec![sc_beads::testing::id("b1")]),
        "the blocked state comes from the stored base"
    );
    touched.load(Ordering::SeqCst)
}

/// NFR-BDC-0001, deterministic: a cached `get` touches the same number of
/// store calls and items whatever the size of its scope.
#[test]
fn a_cached_get_touches_the_same_rows_at_10_100_and_1000_beads() {
    let counts: Vec<u64> = [10, 100, 1000].map(touched_by_one_get).to_vec();
    assert_eq!(counts[0], counts[1], "{counts:?}");
    assert_eq!(counts[1], counts[2], "{counts:?}");
    assert!(
        counts[0] < 20,
        "a cached get is a handful of reads: {counts:?}"
    );
}

/// NFR-BDC-0001 with a long journal (S13): thousands of applied entries older
/// than the base, dated as this store applies them (`applied_epoch` set), do
/// not change what a cached `get` reads. The matching
/// assertion that the journal check is an index lookup is
/// `the_entries_since_check_uses_indexes_and_ignores_a_long_history`
/// (bd-cache-store).
#[test]
fn a_cached_get_reads_the_same_with_thousands_of_old_applied_entries() {
    let (dir, mut cache, touched) = synced_cache(100);
    let id = sc_beads::testing::id("b0");
    cache.get(&id, None).unwrap();
    touched.store(0, Ordering::SeqCst);
    cache.get(&id, None).unwrap();
    let empty = touched.load(Ordering::SeqCst);
    {
        let mut c = rusqlite::Connection::open(dir.path().join("perf.sqlite3")).unwrap();
        let tx = c.transaction().unwrap();
        {
            let mut ins = tx
                .prepare(
                    "INSERT INTO journal(idem_key, phase, bead, op, args_json, precondition_json, \
                     actor, state, attempts, created_at, updated_at, applied_at, applied_epoch) \
                     VALUES (?1, 'x', 'b1', 'close', '{}', '{}', 'san', 'applied', 0, \
                     '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z', \
                     1767225600)",
                )
                .unwrap();
            for i in 0..5_000 {
                ins.execute([format!("old-{i}")]).unwrap();
            }
        }
        tx.commit().unwrap();
    }
    touched.store(0, Ordering::SeqCst);
    let reply = cache.get(&id, None).unwrap();
    assert_eq!(reply.row.blocked, Some(vec![sc_beads::testing::id("b1")]));
    assert_eq!(touched.load(Ordering::SeqCst), empty, "history is not read");
}

/// The SQLite virtual-machine steps of one journal check on `c`.
fn check_steps(c: &rusqlite::Connection, at: i64) -> i32 {
    let mut stmt = c.prepare(bd_cache_store::HAS_ENTRIES_SINCE_SQL).unwrap();
    let _: bool = stmt.query_row([at], |r| r.get(0)).unwrap();
    stmt.get_status(rusqlite::StatementStatus::VmStep)
}

/// The same with entries `bd_cache.py` applies while the cache is open (no
/// `applied_at`, no epoch, QA4-002): a cached `get` still makes the same
/// store calls, and its journal check dates them, so the SQL work of the
/// next check is index probes again, not one per entry. Measured in SQLite
/// VM steps, never wall-clock.
#[test]
fn a_cached_get_dates_entries_python_applied_while_open() {
    let (dir, mut cache, touched) = synced_cache(100);
    let id = sc_beads::testing::id("b0");
    cache.get(&id, None).unwrap();
    touched.store(0, Ordering::SeqCst);
    cache.get(&id, None).unwrap();
    let empty = touched.load(Ordering::SeqCst);
    let path = dir.path().join("perf.sqlite3");
    let c = rusqlite::Connection::open(&path).unwrap();
    let base_at: i64 = c
        .query_row(
            "SELECT unixepoch(fetched_at) FROM ready_base WHERE scope = 'x'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let idle = check_steps(&c, base_at);
    {
        let mut c = rusqlite::Connection::open(&path).unwrap();
        let tx = c.transaction().unwrap();
        {
            let mut ins = tx
                .prepare(
                    "INSERT INTO journal(idem_key, phase, bead, op, args_json, precondition_json, \
                     actor, state, attempts, created_at, updated_at) \
                     VALUES (?1, 'x', 'b1', 'close', '{}', '{}', 'san', 'applied', 0, \
                     '1999-01-01T00:00:00Z', '1999-01-01T00:00:00Z')",
                )
                .unwrap();
            for i in 0..5_000 {
                ins.execute([format!("py-{i}")]).unwrap();
            }
        }
        tx.commit().unwrap();
    }
    let walking = check_steps(&c, base_at);
    assert!(
        walking > 5_000,
        "control: undated entries are walked ({walking})"
    );
    touched.store(0, Ordering::SeqCst);
    let reply = cache.get(&id, None).unwrap();
    assert_eq!(reply.row.blocked, Some(vec![sc_beads::testing::id("b1")]));
    assert_eq!(touched.load(Ordering::SeqCst), empty, "history is not read");
    let after = check_steps(&c, base_at);
    assert!(
        after <= idle + 10,
        "the get dated them: {after} steps, {idle} before they landed"
    );
}

/// With a write in flight the whole base is adjusted (a blocker's close can
/// unblock any bead), so the cost grows with the scope until the next sync:
/// the fast path is only for a base no entry can have changed.
#[test]
fn a_pending_entry_makes_get_adjust_the_whole_base_and_still_agree() {
    let (_dir, mut cache, touched) = synced_cache(100);
    let blocker = sc_beads::testing::id("b1");
    cache
        .write(
            &blocker,
            "close",
            "{}",
            sc_beads::Guards::default(),
            bd_cache_model::IdemKey::new("k").unwrap(),
            sc_beads::testing::actor("san"),
        )
        .unwrap();
    touched.store(0, Ordering::SeqCst);
    let reply = cache.get(&sc_beads::testing::id("b0"), None).unwrap();
    assert_eq!(
        reply.row.blocked,
        Some(vec![]),
        "the pending close of b1 unblocks b0"
    );
    assert!(touched.load(Ordering::SeqCst) > 100);
}

/// An entry applied after the base was fetched is part of the answer too: the
/// stored base still lists the blocker, the journal has closed it.
#[test]
fn an_entry_applied_since_the_fetch_makes_get_adjust_the_whole_base() {
    let (_dir, mut cache, _touched) = synced_cache(20);
    cache
        .write(
            &sc_beads::testing::id("b1"),
            "close",
            "{}",
            sc_beads::Guards::default(),
            bd_cache_model::IdemKey::new("k").unwrap(),
            sc_beads::testing::actor("san"),
        )
        .unwrap();
    let budget = bd_cache_model::DrainBudget::before_tick_end(sc_beads::Deadline::after(
        &*cache.clock(),
        Duration::from_secs(25),
    ));
    let report = cache.drain(&budget).unwrap();
    assert_eq!(report.applied.len(), 1, "{report:?}");
    assert!(
        cache.open_work().unwrap().pending == 0,
        "nothing is pending now"
    );
    let reply = cache.get(&sc_beads::testing::id("b0"), None).unwrap();
    assert_eq!(
        reply.row.blocked,
        Some(vec![]),
        "the applied close of b1 unblocks b0, though the stored base still lists it"
    );
}

/// `Cache::cached`, which the tick uses to read a phase's root, reads the same
/// at any scope size and never calls bd.
#[test]
fn a_cached_read_of_one_bead_touches_the_same_rows_at_10_and_1000_beads() {
    let touched_by = |beads: usize| {
        let (_dir, cache, touched) = synced_cache(beads);
        let id = sc_beads::testing::id("b0");
        cache.cached(&id).unwrap();
        touched.store(0, Ordering::SeqCst);
        let reply = cache.cached(&id).unwrap().unwrap();
        assert!(reply.scope_last_ok_at.is_some());
        assert!(
            cache
                .cached(&sc_beads::testing::id("nobody"))
                .unwrap()
                .is_none()
        );
        touched.load(Ordering::SeqCst)
    };
    assert_eq!(touched_by(10), touched_by(1000));
}

fn median(mut samples: Vec<Duration>) -> Duration {
    samples.sort();
    samples[samples.len() / 2]
}

/// Prints the median wall-clock `get` per scope size; informational, never gates.
#[test]
#[ignore = "informational: prints the median cached get per scope size (use --release)"]
fn print_median_cached_get_by_scope_size() {
    for beads in [10, 100, 300, 1000] {
        let (_dir, mut cache, _touched) = synced_cache(beads);
        let ids: Vec<_> = (0..CALLS)
            .map(|i| sc_beads::testing::id(&format!("b{}", i % beads)))
            .collect();
        for id in ids.iter().take(50) {
            cache.get(id, None).unwrap();
        }
        let samples: Vec<Duration> = ids
            .iter()
            .map(|id| {
                let start = Instant::now();
                cache.get(id, None).unwrap();
                start.elapsed()
            })
            .collect();
        eprintln!("{beads} beads: median cached get {:?}", median(samples));
    }
}
