//! The marker writes after a send wait for the store's write lock at most the
//! time left before `T` (lead ruling S33, RSH-029): the tick passes that time
//! to `mark_announced_within` and `set_sync_alert_within`, and the store caps
//! it at its own busy timeout (`bd-cache-store`'s tests hold a real lock).
//!
//! Deterministic: the world's `ManualClock`, a tick that ends 3 s from now, a
//! sender that spends 1 s of that clock, and a store that records each wait.
//! The one marker write after the send must be given exactly the 2 s left.
//!
//! A bounded write that fails (lead ruling S35, QA7-003, RSH-031): a marker
//! write after a delivered send fails the tick, and the next tick sends the
//! message again and records its marker (at-least-once); an alert clear that
//! fails is logged and does not fail the tick.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a failed unwrap or index is a test failure"
)]

mod common;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use bd_cache::Cache;
use bd_cache_model::testing::{FakeDrainLock, FakeStore};
use bd_cache_model::{CacheConfig, CacheStore};
use bd_cache_sync::{TICK_BUDGET, TickReply, tick};
use common::blocking::Controlled;
use common::counting::Counting;
use common::{SlowSender, World, scope};
use sc_beads::testing::{FakeOp, Injection};
use sc_beads::{Clock, ProjectRef};

type WaitCache = Cache<Counting<FakeStore>, FakeDrainLock, Controlled>;

/// When the tick ends.
const LEFT: Duration = Duration::from_secs(3);
/// What the one message costs on the world's clock.
const SEND: Duration = Duration::from_secs(1);
/// The time left before `T` once the message is sent: `LEFT - SEND`.
const AFTER_SEND: Duration = Duration::from_secs(2);
const _: () = assert!(LEFT.as_millis() - SEND.as_millis() == AFTER_SEND.as_millis());

/// A cache over the world's parts whose store records the waits.
fn recording_cache(w: &World) -> (WaitCache, Arc<Mutex<Vec<Duration>>>) {
    let (cache, waits, _) = armable_cache(w);
    (cache, waits)
}

/// As [`recording_cache`], with the switch that fails the next bounded write.
fn armable_cache(w: &World) -> (WaitCache, Arc<Mutex<Vec<Duration>>>, Arc<AtomicBool>) {
    let (store, _) = Counting::new(w.store.clone());
    let waits = store.waits();
    let fail_next = store.fail_next_bounded_write();
    let cache = Cache::new(
        CacheConfig::new(ProjectRef::local(None), vec![]),
        store,
        w.lock.clone(),
        w.switch.clone(),
        Arc::clone(&w.clock) as Arc<dyn Clock>,
    )
    .unwrap();
    (cache, waits, fail_next)
}

/// One tick ending [`LEFT`] from now, whose message costs [`SEND`].
fn short_tick(w: &World, cache: &mut WaitCache) -> TickReply {
    let sender = SlowSender::new(&w.clock, SEND);
    let reply = tick(cache, &w.config(), &sender, w.after(LEFT)).unwrap();
    assert_eq!(sender.sent().len(), 1, "one message: {reply:?}");
    reply
}

fn waits(of: &Arc<Mutex<Vec<Duration>>>) -> Vec<Duration> {
    of.lock().unwrap().clone()
}

#[test]
fn an_escalation_is_marked_with_the_time_left_before_t() {
    let w = World::new();
    w.phase("x", Some("lead-x"));
    w.add("b1", Some("x"), |_| {});
    let (mut cache, recorded) = recording_cache(&w);
    cache
        .sync(&scope("x"), w.after(Duration::from_secs(20)))
        .unwrap();
    cache
        .write(
            &common::id("b1"),
            "close",
            "{}",
            sc_beads::Guards::default(),
            bd_cache_model::IdemKey::new("k1").unwrap(),
            sc_beads::testing::actor("san"),
        )
        .unwrap();
    w.bd.remove("b1");
    // A full tick escalates the entry; its message is not delivered.
    w.sender.fail(true);
    tick(&mut cache, &w.config(), &*w.sender, w.after(TICK_BUDGET)).unwrap();
    assert_eq!(cache.unannounced_escalations().unwrap().len(), 1);
    assert!(waits(&recorded).is_empty(), "no marker written yet");

    w.next_tick();
    short_tick(&w, &mut cache);

    assert_eq!(waits(&recorded), [AFTER_SEND], "the time left before T");
    assert!(cache.unannounced_escalations().unwrap().is_empty());
}

#[test]
fn a_sync_failure_alert_is_recorded_with_the_time_left_before_t() {
    let w = World::new();
    w.phase("x", Some("lead-x"));
    let (mut cache, recorded) = recording_cache(&w);
    w.bd.inject(FakeOp::ListScope, Injection::Failed(1));

    let reply = short_tick(&w, &mut cache);

    assert_eq!(waits(&recorded), [AFTER_SEND], "{reply:?}");
    assert_eq!(
        w.store.sync_alert(&scope("x")).unwrap().as_deref(),
        Some("BD_CACHE.BD_LIST_FAILED")
    );
}

#[test]
fn an_outage_alert_is_recorded_with_the_time_left_before_t() {
    let w = World::new();
    w.phase("x", Some("lead-x"));
    let (mut cache, recorded) = recording_cache(&w);
    cache
        .sync(&scope("x"), w.after(Duration::from_secs(20)))
        .unwrap();
    w.switch.set_down(true);

    let reply = short_tick(&w, &mut cache);

    assert_eq!(waits(&recorded), [AFTER_SEND], "{reply:?}");
    assert_eq!(
        w.store.sync_alert(&scope("x")).unwrap().as_deref(),
        Some(bd_cache_sync::OUTAGE_CAUSE)
    );
}

/// Journals a close of `b1` in phase `x` that bd cannot apply, and runs one
/// full tick whose message is not delivered: the entry is escalated and
/// unannounced, and the sender has recorded nothing.
fn an_unannounced_escalation(w: &World, cache: &mut WaitCache) {
    w.phase("x", Some("lead-x"));
    w.add("b1", Some("x"), |_| {});
    cache
        .sync(&scope("x"), w.after(Duration::from_secs(20)))
        .unwrap();
    cache
        .write(
            &common::id("b1"),
            "close",
            "{}",
            sc_beads::Guards::default(),
            bd_cache_model::IdemKey::new("k1").unwrap(),
            sc_beads::testing::actor("san"),
        )
        .unwrap();
    w.bd.remove("b1");
    w.sender.fail(true);
    tick(cache, &w.config(), &*w.sender, w.after(TICK_BUDGET)).unwrap();
    assert_eq!(cache.unannounced_escalations().unwrap().len(), 1);
    w.sender.fail(false);
    w.sender.clear();
}

/// The marker write after a delivered send fails: the tick fails with the
/// store's error, the message went out once and stays unmarked; the next tick
/// sends it again and marks it (S35, QA7-003, RSH-031).
#[test]
fn a_failed_marker_write_fails_the_tick_and_the_next_tick_sends_again() {
    let w = World::new();
    let (mut cache, _, fail_next) = armable_cache(&w);
    an_unannounced_escalation(&w, &mut cache);

    w.next_tick();
    fail_next.store(true, Ordering::SeqCst);
    let err = tick(&mut cache, &w.config(), &*w.sender, w.after(TICK_BUDGET)).unwrap_err();

    assert_eq!(err.code(), "BD_CACHE.STORE_FAILED", "{err:?}");
    assert_eq!(w.sender.sent_to("lead-x").len(), 1, "{:?}", w.sender.sent());
    assert_eq!(
        cache.unannounced_escalations().unwrap().len(),
        1,
        "unmarked"
    );

    w.next_tick();
    let reply = tick(&mut cache, &w.config(), &*w.sender, w.after(TICK_BUDGET)).unwrap();

    assert_eq!(w.sender.sent_to("lead-x").len(), 2, "sent again: {reply:?}");
    assert!(
        cache.unannounced_escalations().unwrap().is_empty(),
        "marked"
    );
}

/// The alert clear after a passing sync fails: the tick still succeeds and
/// the alert stays set; the next tick clears it (S35, RSH-030).
#[test]
fn a_failed_alert_clear_is_logged_and_does_not_fail_the_tick() {
    let w = World::new();
    w.phase("x", Some("lead-x"));
    let (mut cache, _, fail_next) = armable_cache(&w);
    w.bd.inject(FakeOp::ListScope, Injection::Failed(1));
    tick(&mut cache, &w.config(), &*w.sender, w.after(TICK_BUDGET)).unwrap();
    assert!(
        w.store.sync_alert(&scope("x")).unwrap().is_some(),
        "alerted"
    );

    w.next_tick();
    fail_next.store(true, Ordering::SeqCst);
    let reply = tick(&mut cache, &w.config(), &*w.sender, w.after(TICK_BUDGET)).unwrap();

    assert_eq!(reply.synced.len(), 1, "{reply:?}");
    assert!(
        w.store.sync_alert(&scope("x")).unwrap().is_some(),
        "the clear failed, the alert stays"
    );

    w.next_tick();
    tick(&mut cache, &w.config(), &*w.sender, w.after(TICK_BUDGET)).unwrap();

    assert_eq!(w.store.sync_alert(&scope("x")).unwrap(), None, "cleared");
}

/// The alert clear after a passing sync, which sends nothing, is given the
/// time left before `T` too (S35, RSH-030): here all of [`LEFT`]. Both clears
/// (this one and the end of an outage) go through one bounded write.
#[test]
fn an_alert_clear_after_a_passing_sync_waits_the_time_left_before_t() {
    let w = World::new();
    w.phase("x", Some("lead-x"));
    let (mut cache, recorded) = recording_cache(&w);
    w.bd.inject(FakeOp::ListScope, Injection::Failed(1));
    tick(&mut cache, &w.config(), &*w.sender, w.after(TICK_BUDGET)).unwrap();
    assert!(
        w.store.sync_alert(&scope("x")).unwrap().is_some(),
        "alerted"
    );
    recorded.lock().unwrap().clear();

    w.next_tick();
    tick(&mut cache, &w.config(), &*w.sender, w.after(LEFT)).unwrap();

    assert_eq!(waits(&recorded), [LEFT], "the clear's wait");
    assert_eq!(w.store.sync_alert(&scope("x")).unwrap(), None);
}
