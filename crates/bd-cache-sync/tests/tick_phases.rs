//! Which phases a tick syncs, and which configurations it refuses
//! (REQ-BDC-0701, 0702; ported from `test_bd_cache_cron.py`).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a failed unwrap or index is a test failure"
)]

mod common;

use bd_cache_model::TickConfig;
use bd_cache_sync::{TickOutcome, schedule_spec, stop_reason};
use common::{World, id, scope};
use sc_beads::testing::{FakeCall, FakeOp};

/// The labels of the phase scopes the tick listed.
fn synced_labels(w: &World) -> Vec<String> {
    w.bd.calls()
        .into_iter()
        .filter_map(|c| match c {
            FakeCall::ListScope { label, .. } => Some(label),
            _ => None,
        })
        .collect()
}

#[test]
fn test_tick_syncs_only_phases_with_an_open_root() {
    let w = World::new();
    // x: root open. y: root closed in bd.
    w.phase("x", Some("lead-x"));
    w.phase("y", Some("lead-y"));
    w.close_in_bd("ry");
    w.add("bx", Some("x"), |_| {});
    w.add("by", Some("y"), |_| {});
    let mut cache = w.cache();

    let reply = w.tick(&mut cache);

    assert_eq!(reply.outcome, TickOutcome::PassThrough, "{reply:?}");
    assert_eq!(reply.active, [scope("x").phase_name().clone()]);
    assert_eq!(synced_labels(&w), ["phase-x"], "only x is synced");
    assert!(cache.get(&id("bx"), None).is_ok());
    assert_eq!(reply.synced.len(), 1);
    // Both uncached roots were read in one `bd list --id`.
    let by_ids: Vec<_> =
        w.bd.calls()
            .into_iter()
            .filter(|c| c.op() == FakeOp::ListByIds)
            .collect();
    assert_eq!(by_ids.len(), 1, "{by_ids:?}");
    assert_eq!(by_ids[0], FakeCall::ListByIds(vec![id("rx"), id("ry")]));
}

#[test]
fn a_root_the_cache_holds_is_read_from_the_cache_and_a_closing_root_ends_the_phase() {
    let w = World::new();
    w.phase("x", Some("lead-x"));
    let mut cache = w.cache();
    // First tick: the root is not cached yet, so it is read from bd.
    let first = w.tick(&mut cache);
    assert_eq!(first.active.len(), 1);
    assert_eq!(
        w.bd.calls()
            .iter()
            .filter(|c| c.op() == FakeOp::ListByIds)
            .count(),
        1
    );

    // Second tick: the sync stored the root in the phase; no root read at all.
    w.bd.clear_calls();
    w.next_tick();
    let second = w.tick(&mut cache);
    assert_eq!(second.active.len(), 1);
    assert_eq!(
        w.bd.calls()
            .iter()
            .filter(|c| c.op() == FakeOp::ListByIds)
            .count(),
        0,
        "{:?}",
        w.bd.calls()
    );

    // The root closes in bd: the tick that syncs it still sees the old row,
    // the next one sees it closed and the phase is no longer synced.
    w.close_in_bd("rx");
    w.next_tick();
    let third = w.tick(&mut cache);
    assert_eq!(third.active.len(), 1);
    w.bd.clear_calls();
    w.next_tick();
    let fourth = w.tick(&mut cache);
    assert!(fourth.active.is_empty(), "{fourth:?}");
    assert!(synced_labels(&w).is_empty(), "a closed phase is not synced");
}

#[test]
fn a_root_that_bd_does_not_know_is_no_open_root() {
    let w = World::new();
    w.phase_file("z", "nowhere-1", Some("lead-z"));
    let mut cache = w.cache();
    let reply = w.tick(&mut cache);
    assert!(reply.active.is_empty());
    assert!(reply.unreadable_roots.is_empty());
}

#[test]
fn test_custom_phases_limits_sync_and_stop() {
    let w = World::new();
    w.phase("x", Some("lead-x"));
    w.phase("y", Some("lead-y"));
    let mut cache = w.cache();

    let mut only_y = w.config();
    only_y.phases = Some(vec!["y".into()]);
    let reply = w.tick_with(&mut cache, &only_y);
    assert_eq!(reply.active, [scope("y").phase_name().clone()]);
    assert_eq!(synced_labels(&w), ["phase-y"], "x is not synced");

    // The stop rule honours the same list: x is open, but the list names only z,
    // whose root is closed, so nothing is active and nothing is open.
    w.phase("z", Some("lead-z"));
    w.close_in_bd("rz");
    let mut only_z = w.config();
    only_z.phases = Some(vec!["phase-z".into()]);
    let clock = &*w.clock;
    let end = w.after(bd_cache_sync::TICK_BUDGET);
    assert!(
        stop_reason(&mut cache, &only_z, clock, end)
            .unwrap()
            .is_some(),
        "z is closed and the list excludes the open x and y"
    );
    assert_eq!(
        stop_reason(&mut cache, &w.config(), clock, end).unwrap(),
        None,
        "without the list x and y are open"
    );
}

#[test]
fn test_malformed_custom_phases_is_refused() {
    for bad in ["bad name", "", "a/b", "phase-"] {
        let w = World::new();
        w.phase("x", Some("lead-x"));
        let mut cache = w.cache();
        let mut cfg = w.config();
        cfg.phases = Some(vec!["x".into(), bad.into()]);

        let reply = w.tick_with(&mut cache, &cfg);

        assert_eq!(reply.outcome, TickOutcome::Refused, "{bad:?}: {reply:?}");
        assert!(reply.refused[0].contains("phases entry"), "{reply:?}");
        assert!(reply.drain.is_none(), "nothing ran");
        assert!(w.bd.calls().is_empty(), "no bd call for {bad:?}");
        assert!(w.sender.sent().is_empty());
    }
}

#[test]
fn a_configured_phase_without_a_file_and_a_broken_file_are_refused_but_the_rest_runs() {
    let w = World::new();
    w.phase("x", Some("lead-x"));
    std::fs::write(w.dir.path().join("phase-broken.toml"), "root = 7\n").unwrap();
    std::fs::write(w.dir.path().join("phase-noroot.toml"), "lead = \"l\"\n").unwrap();
    // Other files in the directory are none of the tick's business.
    std::fs::write(w.dir.path().join("notes.toml"), "x = 1\n").unwrap();
    let mut cache = w.cache();

    let reply = w.tick(&mut cache);

    assert_eq!(reply.outcome, TickOutcome::Refused);
    assert_eq!(reply.refused.len(), 2, "{:?}", reply.refused);
    assert!(
        reply
            .refused
            .iter()
            .any(|r| r.contains("phase-broken.toml"))
    );
    assert!(
        reply
            .refused
            .iter()
            .any(|r| r.contains("phase-noroot.toml"))
    );
    assert_eq!(reply.active.len(), 1, "x still ran");

    let mut cfg = w.config();
    cfg.phases = Some(vec!["x".into(), "ghost".into()]);
    let reply = w.tick_with(&mut cache, &cfg);
    assert!(
        reply.refused.iter().any(|r| r.contains("ghost")),
        "{:?}",
        reply.refused
    );
}

#[test]
fn an_unreadable_phases_directory_is_refused_and_the_journal_still_drains() {
    let w = World::new();
    w.phase("x", Some("lead-x"));
    let mut cache = w.cache();
    w.tick(&mut cache);
    w.add("b1", Some("x"), |_| {});
    cache
        .sync(&scope("x"), w.after(std::time::Duration::from_secs(20)))
        .unwrap();
    let seq = w.journal_close(&mut cache, "b1", "k1");

    let mut cfg = w.config();
    cfg.phases_dir = Some(w.dir.path().join("missing"));
    let reply = w.tick_with(&mut cache, &cfg);

    assert_eq!(reply.outcome, TickOutcome::Refused);
    assert!(reply.refused[0].contains("cannot read the phases directory"));
    assert_eq!(
        reply.drain.unwrap().applied,
        [seq],
        "the drain does not wait on phases"
    );
    // And an unreadable directory never ends the schedule.
    let end = w.after(bd_cache_sync::TICK_BUDGET);
    assert_eq!(stop_reason(&mut cache, &cfg, &*w.clock, end).unwrap(), None);
}

#[test]
fn test_missing_team_is_refused() {
    let w = World::new();
    w.phase("x", Some("lead-x"));
    let mut cache = w.cache();
    for blank in [None, Some(String::new()), Some("  ".to_owned())] {
        let mut cfg = w.config();
        cfg.team = blank.clone();
        let reply = w.tick_with(&mut cache, &cfg);
        assert_eq!(reply.outcome, TickOutcome::Refused, "{blank:?}");
        assert!(reply.refused[0].contains("team is missing"), "{reply:?}");
        assert!(reply.drain.is_none() && reply.synced.is_empty());
        assert!(w.bd.calls().is_empty(), "a refused tick makes no bd call");
        assert!(w.sender.attempts().is_empty());
        assert!(
            schedule_spec(&cfg, "r").is_err(),
            "no schedule is started either"
        );
    }
    // The fallback lead is required too (REQ-BDC-0701).
    let mut cfg = w.config();
    cfg.fallback_lead = None;
    let reply = w.tick_with(&mut cache, &cfg);
    assert_eq!(reply.outcome, TickOutcome::Refused);
    assert!(reply.refused[0].contains("fallback_lead"));
    assert!(schedule_spec(&cfg, "r").is_err());
    // A name that would be read as an option or split in two is refused.
    for bad in ["-x", "a b", "a@b"] {
        let mut cfg = w.config();
        cfg.team = Some(bad.into());
        assert_eq!(
            w.tick_with(&mut cache, &cfg).outcome,
            TickOutcome::Refused,
            "{bad}"
        );
    }
    assert!(w.bd.calls().is_empty());
    // With the table complete the same world ticks.
    assert_eq!(w.tick(&mut cache).outcome, TickOutcome::PassThrough);
    assert!(schedule_spec(&TickConfig::default(), "r").is_err());
}
