//! `write`: validate, dedupe, view or bounded miss fill, precondition, journal
//! (REQ-BDC-0002, 0004, 0302, 0903; REQ-CORE-0101).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a failed unwrap or index is a test failure"
)]

mod common;

use std::time::Duration;

use bd_cache::testing::{backend_check, current_view, miss_fill, store, store_mut};
use bd_cache_model::{
    CacheError, CacheStore, DrainBudget, FingerprintOutcome, GuardKey, IdemKey, JournalState,
    ScopeId, Unmet, WriteReply,
};
use common::{TestCache, World, id, x};
use sc_beads::testing::{FakeCall, FakeOp, Injection, actor};
use sc_beads::{Assignee, Clock, DepType, Edge, Guards};

fn write(
    c: &mut TestCache,
    bead: &str,
    op: &str,
    args: &str,
    pre: Guards,
    key: &str,
) -> Result<WriteReply, CacheError> {
    c.write(
        &id(bead),
        op,
        args,
        pre,
        IdemKey::new(key).unwrap(),
        actor("san"),
    )
}

fn status_is(s: &str) -> Guards {
    Guards {
        status: Some(s.into()),
        assignee: None,
    }
}

fn roomy(w: &World) -> DrainBudget {
    DrainBudget {
        start_gate: w.after(Duration::from_secs(20)),
        hard: w.after(Duration::from_secs(23)),
    }
}

fn unmet_of(e: &CacheError) -> Vec<(GuardKey, Unmet)> {
    match e {
        CacheError::Precondition { unmet, .. } => {
            unmet.iter().map(|(k, u)| (*k, u.clone())).collect()
        }
        other => panic!("expected PRECONDITION, got {other}"),
    }
}

fn unmet(expected: &str, actual: &str) -> Unmet {
    Unmet {
        expected: expected.into(),
        actual: actual.into(),
    }
}

#[test]
fn test_write_is_visible_to_reads_but_cache_row_is_untouched() {
    let w = World::new();
    let mut c = w.cache();
    let at = w.clock.now_utc();
    let r = write(
        &mut c,
        "b1",
        "close",
        r#"{"reason": "PASS"}"#,
        Guards::default(),
        "k1",
    )
    .unwrap();
    assert!(!r.duplicate);
    assert_eq!(
        w.bd.calls(),
        vec![FakeCall::ListByIds(vec![id("b1")])],
        "the only bd call is the miss fill; no write reaches bd"
    );
    let (view, _) = current_view(&c, &id("b1")).unwrap().unwrap();
    assert_eq!(view.status.as_deref(), Some("closed"));
    assert_eq!(view.close_reason.as_deref(), Some("PASS"));
    let (row, _) = store(&c).get_row(&id("b1")).unwrap().unwrap();
    assert_eq!(
        row.status.as_deref(),
        Some("in_progress"),
        "the cached row is untouched"
    );
    assert_eq!(w.bd_row("b1").status.as_deref(), Some("in_progress"));
    let e = store(&c).entry_by_seq(r.seq).unwrap().unwrap();
    assert_eq!(e.state, JournalState::Pending);
    assert_eq!(e.attempts, 0);
    assert_eq!(e.bead, id("b1"));
    assert_eq!(e.scope, Some(x()));
    assert_eq!(e.actor, actor("san"));
    assert_eq!(e.created_at, at);

    w.bd.clear_calls();
    write(
        &mut c,
        "b1",
        "update",
        r#"{"append_notes": "more"}"#,
        Guards::default(),
        "k2",
    )
    .unwrap();
    assert!(w.bd.calls().is_empty(), "a cached bead needs no bd call");
    let (view, _) = current_view(&c, &id("b1")).unwrap().unwrap();
    assert_eq!(view.notes.as_deref(), Some("more"));
    assert_eq!(view.status.as_deref(), Some("closed"));
}

#[test]
fn test_precondition_checked_at_write_time() {
    let w = World::new();
    let mut c = w.cache();
    let e = write(&mut c, "b1", "close", "{}", status_is("open"), "k1").unwrap_err();
    assert_eq!(e.code(), "BD_CACHE.PRECONDITION");
    assert_eq!(e.reason(), Some("precondition_failed"));
    assert_eq!(
        unmet_of(&e),
        vec![(GuardKey::Status, unmet("open", "in_progress"))]
    );
    let unassigned = Guards {
        status: None,
        assignee: Some(Assignee::Unassigned),
    };
    let e = write(&mut c, "b1", "close", "{}", unassigned, "k2").unwrap_err();
    assert_eq!(
        unmet_of(&e),
        vec![(GuardKey::Assignee, unmet("", "dev"))],
        "\"\" means unassigned"
    );
    assert!(w.store.journal().is_empty(), "nothing journaled");

    let met = Guards {
        status: Some("in_progress".into()),
        assignee: Some(Assignee::Named("dev".into())),
    };
    let ok = write(&mut c, "b1", "close", "{}", met, "k3").unwrap();
    assert_eq!(
        store(&c)
            .entry_by_seq(ok.seq)
            .unwrap()
            .unwrap()
            .precondition
            .status
            .as_deref(),
        Some("in_progress"),
        "the precondition is journaled for the drain"
    );

    // Checked against the view: the journaled close already shows.
    let e = write(
        &mut c,
        "b1",
        "update",
        r#"{"append_notes": "x"}"#,
        status_is("in_progress"),
        "k4",
    )
    .unwrap_err();
    assert_eq!(
        unmet_of(&e),
        vec![(GuardKey::Status, unmet("in_progress", "closed"))]
    );
    assert_eq!(w.store.journal().len(), 1);
}

#[test]
fn test_duplicate_idem_key_is_one_entry_and_a_conflicting_reuse_is_refused() {
    let w = World::new();
    let mut c = w.cache();
    let args = r#"{"assignee": "lead"}"#;
    let first = write(&mut c, "b1", "update", args, Guards::default(), "k1").unwrap();
    assert!(!first.duplicate);
    let again = write(&mut c, "b1", "update", args, Guards::default(), "k1").unwrap();
    assert_eq!(
        again,
        WriteReply {
            seq: first.seq,
            duplicate: true
        }
    );
    let respelled = write(
        &mut c,
        "b1",
        "update",
        r#"{ "assignee" : "lead" }"#,
        Guards::default(),
        "k1",
    )
    .unwrap();
    assert!(
        respelled.duplicate,
        "equal args in another spelling are one write"
    );

    for (bead, op, args) in [
        ("b1", "update", r#"{"assignee": "other"}"#),
        ("b2", "update", r#"{"assignee": "lead"}"#),
        ("b1", "close", "{}"),
    ] {
        let e = write(&mut c, bead, op, args, Guards::default(), "k1").unwrap_err();
        assert_eq!(e.code(), "BD_CACHE.IDEM_CONFLICT", "{bead} {op} {args}");
        match e {
            CacheError::IdemConflict { seq, .. } => assert_eq!(seq, first.seq),
            other => panic!("{other}"),
        }
    }
    assert_eq!(w.store.journal().len(), 1, "one entry for the key");

    // A key is deduped in any state, not only while pending.
    let at = w.clock.now_utc();
    store_mut(&mut c)
        .escalate(first.seq, "test escalation", &at)
        .unwrap();
    let late = write(&mut c, "b1", "update", args, Guards::default(), "k1").unwrap();
    assert!(late.duplicate);
    assert_eq!(late.seq, first.seq);
}

#[test]
fn test_retry_of_a_journaled_precondition_write_is_a_duplicate_not_a_precondition_error() {
    let w = World::new();
    let mut c = w.cache();
    let pre = || status_is("in_progress");
    let first = write(&mut c, "b1", "close", "{}", pre(), "k1").unwrap();
    // The view is now closed, so checking the precondition first would refuse the retry.
    let retry = write(&mut c, "b1", "close", "{}", pre(), "k1").unwrap();
    assert_eq!(retry.seq, first.seq);
    assert!(retry.duplicate);

    let report = c.drain(&roomy(&w)).unwrap();
    assert_eq!(report.applied, vec![first.seq]);
    let retry = write(&mut c, "b1", "close", "{}", pre(), "k1").unwrap();
    assert!(retry.duplicate, "applied entries dedupe too");
    assert_eq!(retry.seq, first.seq);
    assert_eq!(w.store.journal().len(), 1);
    assert_eq!(w.bd.writes().len(), 1);
}

#[test]
fn test_dep_add_overlays_then_lands_as_a_real_edge() {
    let w = World::new();
    let mut c = w.cache();
    let edge = Edge::new(id("b2"), id("b1"), DepType::Blocks);
    let r = write(
        &mut c,
        "b2",
        "dep_add",
        r#"{"depends_on": "b1", "type": "blocks"}"#,
        Guards::default(),
        "k1",
    )
    .unwrap();
    let (view, _) = current_view(&c, &id("b2")).unwrap().unwrap();
    assert!(
        view.dependencies.contains(&edge),
        "the overlay shows the edge"
    );
    let (row, _) = store(&c).get_row(&id("b2")).unwrap().unwrap();
    assert!(
        !row.dependencies.contains(&edge),
        "the cached row has none yet"
    );
    assert!(!w.bd_row("b2").dependencies.contains(&edge));

    let report = c.drain(&roomy(&w)).unwrap();
    assert_eq!(report.applied, vec![r.seq]);
    assert!(
        w.bd_row("b2").dependencies.contains(&edge),
        "bd holds the edge"
    );
    let (row, _) = store(&c).get_row(&id("b2")).unwrap().unwrap();
    assert!(row.dependencies.contains(&edge), "the read-back stored it");
    assert!(store(&c).dependents_of(&id("b1")).unwrap().contains(&edge));
    assert_eq!(
        store(&c).entry_by_seq(r.seq).unwrap().unwrap().state,
        JournalState::Applied
    );
}

#[test]
fn test_write_during_an_outage_journals_cached_beads_and_refuses_uncached_ones_in_time() {
    let w = World::new();
    let mut c = w.cache();
    miss_fill(&mut c, &id("b1"), None).unwrap();
    w.bd.clear_calls();

    // bd too slow to answer: the uncached bead's fill stops at miss_fill_timeout.
    w.bd.set_cost_for(FakeOp::ListByIds, Duration::from_secs(10));
    let before = w.clock.now_mono();
    let e = write(&mut c, "b2", "close", "{}", Guards::default(), "k1").unwrap_err();
    assert_eq!(e.code(), "BD_CACHE.BD_UNAVAILABLE");
    assert_eq!(
        w.clock.now_mono() - before,
        w.config.miss_fill_timeout,
        "refused within the miss-fill timeout"
    );

    // bd refusing connections: BD_UNAVAILABLE and the outage is recorded.
    w.bd.set_cost_for(FakeOp::ListByIds, Duration::ZERO);
    w.bd.inject(
        FakeOp::ListByIds,
        Injection::Unavailable { timed_out: false },
    );
    let e = write(&mut c, "b2", "close", "{}", Guards::default(), "k2").unwrap_err();
    assert_eq!(e.code(), "BD_CACHE.BD_UNAVAILABLE");
    assert!(store(&c).outage().unwrap().is_some());

    // The cached bead is journaled with no bd call.
    w.bd.clear_calls();
    let ok = write(&mut c, "b1", "close", "{}", Guards::default(), "k3").unwrap();
    assert!(w.bd.calls().is_empty());
    let journal = w.store.journal();
    assert_eq!(journal.len(), 1);
    assert_eq!(journal[0].seq, ok.seq);
    assert_eq!(journal[0].bead, id("b1"));
}

#[test]
fn invalid_writes_are_refused_before_any_bd_call() {
    let w = World::new();
    let mut c = w.cache();
    for (op, args, code) in [
        ("delete", "{}", "BD_CACHE.BAD_OP"),
        ("close", "[]", "BD_CACHE.BAD_ARGS"),
        ("update", "{}", "BD_CACHE.BAD_ARGS"),
        (
            "dep_add",
            r#"{"depends_on": "b1", "type": "loves"}"#,
            "BD_CACHE.BAD_ARGS",
        ),
        (
            "update",
            r#"{"set_metadata": {"k": "5"}}"#,
            "BD_CACHE.BAD_ARGS",
        ),
    ] {
        let e = write(&mut c, "b2", op, args, Guards::default(), "k1").unwrap_err();
        assert_eq!(e.code(), code, "{op} {args}");
    }
    assert!(
        w.bd.calls().is_empty(),
        "validation runs before the miss fill"
    );
    assert!(w.store.journal().is_empty());
}

#[test]
fn a_write_to_a_bead_unknown_to_bd_is_not_found() {
    let w = World::new();
    let mut c = w.cache();
    let e = write(&mut c, "nope", "close", "{}", Guards::default(), "k1").unwrap_err();
    assert_eq!(e.code(), "BD_CACHE.NOT_FOUND");
    assert!(w.store.journal().is_empty());
    assert_eq!(w.bd.calls(), vec![FakeCall::ListByIds(vec![id("nope")])]);
}

#[test]
fn the_entry_scope_is_the_views_first_phase_label() {
    let w = World::new();
    w.add("o1", |b| b.labels = vec!["misc".into()]);
    w.add("y1", |b| b.labels = vec!["misc".into(), "phase-y".into()]);
    let mut c = w.cache();
    let scope = |c: &mut TestCache, bead: &str, key: &str| {
        let r = write(c, bead, "close", "{}", Guards::default(), key).unwrap();
        store(c).entry_by_seq(r.seq).unwrap().unwrap().scope
    };
    assert_eq!(scope(&mut c, "b1", "k1"), Some(x()));
    assert_eq!(scope(&mut c, "o1", "k2"), None, "an out-of-scope bead");
    assert_eq!(
        scope(&mut c, "y1", "k3"),
        Some(ScopeId::phase("y").unwrap())
    );
}

#[test]
fn a_write_after_a_rebuild_checks_the_filled_row_with_pending_entries_replayed() {
    let w = World::new();
    let mut c = w.cache();
    write(&mut c, "b1", "close", "{}", Guards::default(), "k1").unwrap();
    let hard = w.after(Duration::from_secs(5));
    assert_eq!(
        backend_check(&mut c, hard).unwrap().0,
        FingerprintOutcome::Recorded
    );
    w.bd.set_schema("{\"changed\": true}");
    assert_eq!(
        backend_check(&mut c, hard).unwrap().0,
        FingerprintOutcome::Rebuilt
    );
    assert!(
        store(&c).get_row(&id("b1")).unwrap().is_none(),
        "rows dropped"
    );
    // The refilled row is in_progress in bd; the journaled close still holds in the view.
    write(
        &mut c,
        "b1",
        "update",
        r#"{"append_notes": "after"}"#,
        status_is("closed"),
        "k2",
    )
    .unwrap();
    assert_eq!(w.store.journal().len(), 2);
}
