//! The drain lock across processes (REQ-BDC-0401).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a failed unwrap or index is a test failure"
)]

mod common;

use std::io::Read as _;

use bd_cache_model::DrainLock;
use bd_cache_store::FlockDrainLock;
use common::{CHILD_TIMEOUT, Holder, TempDir, this_test_binary};

const READY: &str = "BDC-LOCK-HELD";

#[test]
fn two_processes_contend_for_the_drain_lock() {
    let tmp = TempDir::new("lock");
    let cache = tmp.path().join("testdb.bd-cache.sqlite3");
    let lock = FlockDrainLock::for_cache_file(&cache);
    assert_eq!(lock.path(), tmp.path().join("testdb.bd-cache.drain.lock"));

    let (child, line) = Holder::spawn(
        this_test_binary("drain_lock_child").env("BDC_TEST_CACHE", &cache),
        READY,
        CHILD_TIMEOUT,
    );
    assert!(line.contains("acquired=true"), "{line}");
    assert!(
        lock.try_acquire().unwrap().is_none(),
        "only one process acquires the lock"
    );
    child.release(CHILD_TIMEOUT);
    let guard = lock.try_acquire().unwrap();
    assert!(
        guard.is_some(),
        "the child's guard was dropped: the lock is free"
    );

    // The other way round: while this process holds it, the child cannot take it.
    let (child, line) = Holder::spawn(
        this_test_binary("drain_lock_child").env("BDC_TEST_CACHE", &cache),
        READY,
        CHILD_TIMEOUT,
    );
    assert!(line.contains("acquired=false"), "{line}");
    child.release(CHILD_TIMEOUT);
    drop(guard);
    assert!(
        lock.try_acquire().unwrap().is_some(),
        "dropping the guard releases it"
    );
}

/// Child of the test above: tries the lock, reports, holds it until stdin closes.
#[test]
#[ignore = "child process of two_processes_contend_for_the_drain_lock"]
fn drain_lock_child() {
    let Some(cache) = std::env::var_os("BDC_TEST_CACHE") else {
        return;
    };
    let lock = FlockDrainLock::for_cache_file(cache.as_ref());
    let guard = lock.try_acquire().unwrap();
    println!("{READY} acquired={}", guard.is_some());
    // The parent closes stdin to release us (or dies, which closes it too).
    let mut sink = Vec::new();
    let _ = std::io::stdin().read_to_end(&mut sink);
    drop(guard);
}
