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
    let x = ScopeId::phase("x").unwrap();
    for i in 0..beads {
        let mut b = bead(&format!("b{i}"));
        b.labels = vec!["phase-x".into()];
        bd.insert(b);
    }
    for i in (0..beads.saturating_sub(1)).step_by(10) {
        bd.add_edge(&format!("b{i}"), &format!("b{}", i + 1), DepType::Blocks);
    }
    let mut cache = Cache::new(
        CacheConfig::new(ProjectRef::local(None), vec![x.clone()]),
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
