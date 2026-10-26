//! Informational latency of an in-process cached `get` on the real
//! `SqliteStore` (NFR-BDC-0001), by scope size.
//!
//! `cargo bench -p bd-cache-sync`. The assertion is `tests/perf.rs`; these
//! figures are for tracking, not gating.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "bench code: a failed setup is a failed bench"
)]

use std::hint::black_box;
use std::sync::Arc;
use std::time::Duration;

use bd_cache::Cache;
use bd_cache_model::{CacheConfig, ScopeId};
use bd_cache_store::{DatabaseIdentity, DoltDatabase, FlockDrainLock, ProjectId, SqliteStore};
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use sc_beads::testing::{FakeBackend, ManualClock, bead};
use sc_beads::{BeadId, Clock, Deadline, DepType, ProjectRef};

/// A cache over a fresh store holding `beads` synced beads in scope x, every
/// tenth blocked by its neighbour.
fn synced(
    beads: usize,
) -> (
    tempfile::TempDir,
    Cache<SqliteStore, FlockDrainLock, FakeBackend>,
) {
    let dir = tempfile::tempdir().expect("temp dir");
    let identity = DatabaseIdentity {
        dolt_database: DoltDatabase::new("bench").expect("database"),
        project_id: ProjectId::new("p-bench").expect("project"),
    };
    let store = SqliteStore::open_at(&dir.path().join("bench.sqlite3"), &identity).expect("store");
    let lock = store.drain_lock();
    let clock = Arc::new(ManualClock::default());
    let bd = FakeBackend::new(Arc::clone(&clock));
    let x = ScopeId::new("x").expect("scope");
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
    .expect("cache");
    cache
        .sync(&x, Deadline::after(&*clock, Duration::from_secs(20)))
        .expect("sync");
    (dir, cache)
}

fn cached_get(c: &mut Criterion) {
    let mut group = c.benchmark_group("cached_get");
    for beads in [10_usize, 100, 300, 1000] {
        let (_dir, mut cache) = synced(beads);
        let ids: Vec<BeadId> = (0..beads)
            .map(|i| BeadId::new(format!("b{i}")).expect("bead id"))
            .collect();
        let mut next = 0_usize;
        group.bench_with_input(BenchmarkId::from_parameter(beads), &beads, |b, _| {
            b.iter(|| {
                next = (next + 1) % ids.len();
                black_box(cache.get(black_box(&ids[next]), None)).expect("get")
            });
        });
    }
    group.finish();
}

criterion_group!(benches, cached_get);
criterion_main!(benches);
