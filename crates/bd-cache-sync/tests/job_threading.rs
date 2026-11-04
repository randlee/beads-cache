//! `join_idle` (PE-RSH-004): it waits for a blocking step still inside the
//! cache, up to a deadline. Causal: the step holds the cache's mutex until
//! the test releases it through a channel; no sleep decides the outcome.

mod common;

use std::sync::mpsc;
use std::time::{Duration, Instant};

use bd_cache_sync::join_idle;
use common::World;

#[test]
fn join_idle_is_false_while_a_step_holds_the_cache_and_true_once_it_returns() {
    let world = World::new();
    let cache = world.shared(vec![]);
    let (held_tx, held_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel::<()>();
    let holder = {
        let cache = std::sync::Arc::clone(&cache);
        std::thread::spawn(move || {
            let guard = cache.lock().unwrap();
            held_tx.send(()).unwrap();
            release_rx.recv().unwrap();
            drop(guard);
        })
    };
    held_rx.recv().unwrap();
    assert!(
        !join_idle(&cache, Instant::now() + Duration::from_millis(20)),
        "the step is inside"
    );
    release_tx.send(()).unwrap();
    holder.join().unwrap();
    assert!(
        join_idle(&cache, Instant::now()),
        "free once the step returned"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn inherited_deadline_includes_the_blocking_cache_claim_and_starts_no_bd_after_expiry() {
    let world = World::new();
    let cache = world.shared(vec![]);
    let (held, holding) = mpsc::channel();
    let (release, released) = mpsc::channel::<()>();
    let holder = {
        let cache = std::sync::Arc::clone(&cache);
        std::thread::spawn(move || {
            let until = Instant::now() + Duration::from_secs(5);
            let guard = loop {
                if let Ok(guard) = cache.try_lock() {
                    break guard;
                }
                assert!(Instant::now() < until);
                std::thread::yield_now();
            };
            held.send(()).unwrap();
            let _ = released.recv_timeout(Duration::from_secs(5));
            drop(guard);
        })
    };
    holding.recv_timeout(Duration::from_secs(5)).unwrap();
    let before = world.bd.calls().len();
    let payload = bd_cache_sync::BuiltinPayload::default();
    let budget = bd_cache_sync::Budget::new(&*world.clock, &payload.budget);
    let start = Instant::now();
    let result = bd_cache_sync::run_builtin_until(
        &cache,
        &payload,
        &world.ctx(),
        bd_cache_sync::new_run_id(9).unwrap(),
        sc_beads::Deadline::at(start + Duration::from_millis(50)),
        budget,
    )
    .await;
    let elapsed = start.elapsed();
    drop(release);
    common::join_thread(holder).unwrap();
    assert!(result.unwrap_err().detail().contains("deadline expired"));
    assert!(elapsed < Duration::from_millis(150));
    assert_eq!(
        world.bd.calls().len(),
        before,
        "no detached preparation starts a late bd call"
    );
    let expired = bd_cache_sync::run_builtin_until(
        &cache,
        &payload,
        &world.ctx(),
        bd_cache_sync::new_run_id(10).unwrap(),
        sc_beads::Deadline::at(Instant::now().checked_sub(Duration::from_secs(1)).unwrap()),
        budget,
    )
    .await;
    assert!(expired.is_err());
    assert_eq!(world.bd.calls().len(), before);
}
