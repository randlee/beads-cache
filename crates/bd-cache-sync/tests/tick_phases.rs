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
use bd_cache_sync::{RefusalCode, TickOutcome, schedule_spec, stop_reason};
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

/// A root bd does not know is no open root, so its phase is not synced; it is
/// unknown, not closed, and is reported with the unreadable roots (lead ruling
/// S23, ADR-SYNC-0004).
#[test]
fn a_root_that_bd_does_not_know_is_no_open_root_and_is_reported() {
    let w = World::new();
    w.phase_file("z", "nowhere-1", Some("lead-z"));
    let mut cache = w.cache();
    let reply = w.tick(&mut cache);
    assert!(reply.active.is_empty());
    assert!(
        synced_labels(&w).is_empty(),
        "no sync of an unknown root's phase"
    );
    assert_eq!(reply.unreadable_roots, [scope("z").phase_name().clone()]);
    // A root bd answered without has its own refusal (RBP-F039); the phase
    // stays unknown, so the schedule goes on (S23).
    let not_returned: Vec<_> = reply
        .refused
        .iter()
        .filter(|r| r.code == RefusalCode::PhaseRootNotReturned)
        .collect();
    assert_eq!(not_returned.len(), 1, "{reply:?}");
    assert_eq!(
        not_returned[0].fix,
        "check root in phase-<x>.toml names an existing bead"
    );
    assert!(
        not_returned[0].detail.contains("phase-z.toml")
            && not_returned[0].detail.contains("nowhere-1"),
        "{}",
        not_returned[0].detail
    );
    assert_eq!(reply.outcome, TickOutcome::Refused);
    let end = w.after(bd_cache_sync::TICK_BUDGET);
    assert_eq!(stop_reason(&mut cache, &w.config(), end).unwrap(), None);
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
    let end = w.after(bd_cache_sync::TICK_BUDGET);
    assert!(
        stop_reason(&mut cache, &only_z, end).unwrap().is_some(),
        "z is closed and the list excludes the open x and y"
    );
    assert_eq!(
        stop_reason(&mut cache, &w.config(), end).unwrap(),
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
        assert_eq!(
            reply.refused[0].code,
            RefusalCode::MalformedPhases,
            "{reply:?}"
        );
        assert!(
            reply.refused[0].detail.contains("phases entry"),
            "{reply:?}"
        );
        assert!(!reply.refused[0].fix.is_empty());
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
            .any(|r| r.detail.contains("phase-broken.toml"))
    );
    assert!(
        reply
            .refused
            .iter()
            .any(|r| r.detail.contains("phase-noroot.toml"))
    );
    assert_eq!(reply.active.len(), 1, "x still ran");

    let mut cfg = w.config();
    cfg.phases = Some(vec!["x".into(), "ghost".into()]);
    let reply = w.tick_with(&mut cache, &cfg);
    assert!(
        reply.refused.iter().any(|r| r.detail.contains("ghost")),
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
    assert_eq!(reply.refused[0].code, RefusalCode::PhasesDirUnreadable);
    assert!(
        reply.refused[0]
            .detail
            .contains("cannot read the phases directory")
    );
    assert_eq!(
        reply.drain.unwrap().applied,
        [seq],
        "the drain does not wait on phases"
    );
    // And an unreadable directory never ends the schedule.
    let end = w.after(bd_cache_sync::TICK_BUDGET);
    assert_eq!(stop_reason(&mut cache, &cfg, end).unwrap(), None);
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
        assert_eq!(reply.refused[0].code, RefusalCode::MissingTeam, "{reply:?}");
        assert!(reply.drain.is_none() && reply.synced.is_empty());
        assert!(w.bd.calls().is_empty(), "a refused tick makes no bd call");
        assert!(w.sender.attempts().is_empty());
        assert!(
            schedule_spec(&cfg, "r", None).is_err(),
            "no schedule is started either"
        );
    }
    // The fallback lead is required too (REQ-BDC-0701).
    let mut cfg = w.config();
    cfg.fallback_lead = None;
    let reply = w.tick_with(&mut cache, &cfg);
    assert_eq!(reply.outcome, TickOutcome::Refused);
    assert_eq!(reply.refused[0].code, RefusalCode::MissingFallbackLead);
    assert!(schedule_spec(&cfg, "r", None).is_err());
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
    assert!(schedule_spec(&TickConfig::default(), "r", None).is_err());
}

#[test]
fn an_empty_phase_list_is_refused_not_a_schedule_that_ends_at_once() {
    let w = World::new();
    w.phase("x", Some("lead-x"));
    let mut cache = w.cache();
    let mut cfg = w.config();
    cfg.phases = Some(Vec::new());
    let reply = w.tick_with(&mut cache, &cfg);
    assert_eq!(reply.refused[0].code, RefusalCode::EmptyPhases, "{reply:?}");
    assert!(w.bd.calls().is_empty());
    assert!(schedule_spec(&cfg, "r", None).is_err());
    let end = w.after(bd_cache_sync::TICK_BUDGET);
    assert_eq!(stop_reason(&mut cache, &cfg, end).unwrap(), None);
}

#[test]
fn a_phase_file_over_the_size_cap_and_a_directory_over_the_file_cap_are_bounded() {
    let w = World::new();
    w.phase("x", Some("lead-x"));
    let big = format!("root = \"rbig\"\n# {}\n", "x".repeat(70 * 1024));
    std::fs::write(w.dir.path().join("phase-big.toml"), big).unwrap();
    let set = bd_cache_sync::load_phases(w.dir.path(), None).unwrap();
    assert_eq!(set.phases.len(), 1);
    assert!(matches!(
        set.problems.as_slice(),
        [bd_cache_sync::PhaseFileProblem::TooLarge { bytes, .. }] if *bytes > bd_cache_sync::MAX_PHASE_FILE_BYTES
    ));

    let many = tempfile::tempdir().unwrap();
    for i in 0..bd_cache_sync::MAX_PHASE_FILES + 5 {
        std::fs::write(
            many.path().join(format!("phase-p{i}.toml")),
            "root = \"r\"\n",
        )
        .unwrap();
    }
    let set = bd_cache_sync::load_phases(many.path(), None).unwrap();
    assert!(set.phases.len() <= bd_cache_sync::MAX_PHASE_FILES);
    assert!(
        set.problems
            .iter()
            .any(|p| matches!(p, bd_cache_sync::PhaseFileProblem::TooManyFiles { .. }))
    );
}

/// With `[tick] phases` set, every configured phase is read by name, however
/// many phase files the directory holds: the file cap cannot hide one
/// (PE-RSH-010).
#[test]
fn every_configured_phase_is_read_from_a_directory_over_the_file_cap() {
    let dir = tempfile::tempdir().unwrap();
    let count = bd_cache_sync::MAX_PHASE_FILES + 44;
    let mut names = Vec::new();
    for i in 0..count {
        std::fs::write(
            dir.path().join(format!("phase-p{i}.toml")),
            format!("root = \"r{i}\"\n"),
        )
        .unwrap();
        names.push(bd_cache_model::PhaseName::new(format!("p{i}")).unwrap());
    }
    // One configured phase has no file.
    names.push(bd_cache_model::PhaseName::new("absent").unwrap());

    let set = bd_cache_sync::load_phases(dir.path(), Some(&names)).unwrap();

    assert_eq!(set.phases.len(), count, "every configured file was read");
    assert!(
        matches!(
            set.problems.as_slice(),
            [bd_cache_sync::PhaseFileProblem::NoFile { phase, .. }] if phase.as_str() == "absent"
        ),
        "{:?}",
        set.problems
    );
    let last = format!("p{}", count - 1);
    assert!(
        set.phases
            .iter()
            .any(|p| p.name.as_str() == last && p.root == id(&format!("r{}", count - 1)))
    );
}

/// Without `[tick] phases`, the scan visits at most `MAX_PHASES_DIR_ENTRIES`
/// entries of any kind, so a directory full of other files bounds the tick
/// (PE-RSH-010).
#[test]
fn the_scan_of_the_phases_directory_visits_a_bounded_number_of_entries() {
    let dir = tempfile::tempdir().unwrap();
    for i in 0..bd_cache_sync::MAX_PHASES_DIR_ENTRIES + 10 {
        std::fs::write(dir.path().join(format!("note-{i}.txt")), "").unwrap();
    }

    let set = bd_cache_sync::load_phases(dir.path(), None).unwrap();

    assert!(set.phases.is_empty());
    assert!(
        matches!(
            set.problems.as_slice(),
            [bd_cache_sync::PhaseFileProblem::TooManyEntries { .. }]
        ),
        "{:?}",
        set.problems
    );
    assert_eq!(set.problems[0].code(), RefusalCode::PhaseFilesTooMany);
}

/// A missing phases directory has one code on both surfaces: the schedule's
/// check and the tick's refusal (PE-QA-003).
#[test]
fn a_missing_phases_directory_has_one_code_for_the_schedule_and_the_tick() {
    let w = World::new();
    let mut cfg = w.config();
    cfg.phases_dir = Some(w.dir.path().join("missing"));
    let atm = bd_cache_sync::AtmSender::default();

    let err = schedule_spec(&cfg, "r", Some(&atm)).unwrap_err();
    let mut cache = w.cache();
    let reply = w.tick_with(&mut cache, &cfg);

    assert!(
        matches!(err, bd_cache_sync::ScheduleError::PhasesDir(_)),
        "{err:?}"
    );
    let refusal = reply
        .refused
        .iter()
        .find(|r| r.code == RefusalCode::PhasesDirUnreadable)
        .unwrap_or_else(|| panic!("the tick refuses the directory: {reply:?}"));
    assert_eq!(err.code(), refusal.code.as_str());
    assert_eq!(err.code(), "phases_dir_unreadable");
    assert_eq!(err.fix(), refusal.code.fix());
}

/// The phase files atm-core keeps (`.atm-bd/phase-bd.toml` and `phase-bk.toml`
/// at `05831df`, see `fixtures/PROVENANCE.md`) load: `root` and `lead` are
/// read and `plan`, `integration_branch` and `[templates]` are ignored.
#[test]
fn the_phase_files_atm_core_keeps_are_read() {
    let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let set = bd_cache_sync::load_phases(&fixtures, None).unwrap();
    assert!(set.problems.is_empty(), "{:?}", set.problems);
    let got: Vec<(String, String, Option<String>)> = set
        .phases
        .iter()
        .map(|p| {
            (
                p.name.to_string(),
                p.root.to_string(),
                p.lead.as_ref().map(ToString::to_string),
            )
        })
        .collect();
    assert_eq!(
        got,
        [
            (
                "bd".to_owned(),
                "atm-phase-bd".to_owned(),
                Some("team-lead".to_owned())
            ),
            (
                "bk".to_owned(),
                "atm-phase-bk".to_owned(),
                Some("solar".to_owned())
            ),
        ]
    );
}

/// A path named like a phase file that is not a regular file is reported, not
/// opened (a pipe would block the tick) (RSH-017).
#[test]
fn a_phase_file_that_is_not_a_regular_file_is_reported_not_read() {
    let w = World::new();
    w.phase("x", Some("lead-x"));
    std::fs::create_dir(w.dir.path().join("phase-dir.toml")).unwrap();
    let set = bd_cache_sync::load_phases(w.dir.path(), None).unwrap();
    assert_eq!(set.phases.len(), 1);
    assert!(matches!(
        set.problems.as_slice(),
        [bd_cache_sync::PhaseFileProblem::NotAFile { .. }]
    ));
    assert_eq!(set.problems[0].code(), RefusalCode::PhaseFileNotRegular);
    assert!(!set.problems[0].fix().is_empty());
}

/// Each problem has its own code and fix (RBP-F018).
#[test]
fn phase_file_problems_have_their_own_codes() {
    let w = World::new();
    std::fs::write(w.dir.path().join("phase-a.toml"), "root = 1\n").unwrap();
    std::fs::write(
        w.dir.path().join("phase-b.toml"),
        "root = \"r\"\nlead = \"a b\"\n",
    )
    .unwrap();
    std::fs::write(w.dir.path().join("phase-c d.toml"), "root = \"r\"\n").unwrap();
    let mut cfg = w.config();
    cfg.phases = Some(vec!["ghost".into(), "a".into(), "b".into()]);
    let mut cache = w.cache();
    let reply = w.tick_with(&mut cache, &cfg);
    let codes: std::collections::BTreeSet<&str> =
        reply.refused.iter().map(|r| r.code.as_str()).collect();
    assert!(codes.contains("phase_file_invalid"), "{codes:?}");
    assert!(codes.contains("phase_file_missing"), "{codes:?}");
    assert!(reply.refused.iter().all(|r| !r.fix.is_empty()));
    let bad_lead = reply
        .refused
        .iter()
        .find(|r| r.detail.contains("phase-b.toml"))
        .unwrap();
    assert!(bad_lead.fix.contains("whitespace"), "{}", bad_lead.fix);
}

/// A phase file that is a pipe is opened without waiting for a writer and is
/// reported, not read: the tick holds the cache while it reads phase files
/// (RSH-024).
#[cfg(unix)]
#[test]
fn a_phase_file_that_is_a_pipe_does_not_block_the_tick() {
    let w = World::new();
    w.phase("x", Some("lead-x"));
    let fifo = w.dir.path().join("phase-pipe.toml");
    let made = std::process::Command::new("mkfifo")
        .arg(&fifo)
        .status()
        .unwrap();
    assert!(made.success());
    let dir = w.dir.path().to_path_buf();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(bd_cache_sync::load_phases(&dir, None));
    });
    let set = rx
        .recv_timeout(std::time::Duration::from_secs(10))
        .expect("reading the phase files blocked on a pipe")
        .unwrap();
    assert_eq!(set.phases.len(), 1);
    assert!(matches!(
        set.problems.as_slice(),
        [bd_cache_sync::PhaseFileProblem::NotAFile { .. }]
    ));
}

/// A root that carries two phase labels is cached under the first known scope.
/// That scope's sync says nothing about the phase whose root it is: the cached
/// row is trusted only once the phase's own scope has synced (ARCH-304).
#[test]
fn a_root_with_two_phase_labels_is_trusted_only_after_its_own_phase_syncs() {
    let w = World::new();
    w.phase_file("x", "rx", Some("lead-x"));
    w.add("rx", None, |b| {
        b.labels = vec!["phase-a".into(), "phase-x".into()];
    });
    let mut cache = w.cache();
    // Phase a syncs and caches the root (open); phase x never synced.
    cache
        .sync(&scope("a"), w.after(std::time::Duration::from_secs(20)))
        .unwrap();
    assert!(
        cache
            .cached(&id("rx"))
            .unwrap()
            .unwrap()
            .scope_last_ok_at
            .is_some(),
        "the cached reply carries phase a's sync"
    );
    w.close_in_bd("rx");
    w.bd.clear_calls();

    let reply = w.tick(&mut cache);

    assert!(reply.active.is_empty(), "the root closed in bd: {reply:?}");
    assert_eq!(
        w.bd.calls()
            .iter()
            .filter(|c| c.op() == FakeOp::ListByIds)
            .count(),
        1,
        "the root was read from bd, not trusted on phase a's sync"
    );
    assert!(
        stop_reason(&mut cache, &w.config(), w.after(bd_cache_sync::TICK_BUDGET))
            .unwrap()
            .is_some()
    );
}
