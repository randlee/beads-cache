//! `join_idle` (PE-RSH-004): it waits for a blocking step still inside the
//! cache, up to a deadline. Causal: the step holds the cache's mutex until
//! the test releases it through a channel; no sleep decides the outcome.

#![allow(clippy::unwrap_used, clippy::expect_used, reason = "test code")]

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
    assert!(!join_idle(&cache, Instant::now() + Duration::from_millis(20)), "the step is inside");
    release_tx.send(()).unwrap();
    holder.join().unwrap();
    assert!(join_idle(&cache, Instant::now()), "free once the step returned");
}
