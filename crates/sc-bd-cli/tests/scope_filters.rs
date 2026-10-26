//! The scope-filter argv against the bd 1.3.1 recording (ADR-BDC-0013, sprint b-7).
//!
//! `sc-beads/tests/fixtures/bd-1.3.1/scope-filters/` holds, for each filter variant, the
//! argv it was recorded with (`argv.json`) and bd 1.3.1's answer. These tests check that
//! the builders of `argv.rs` produce exactly the recorded argv for the filter each
//! recording names (`sc_beads::testing::contract::recorded_filter`), and that `BdCli` over
//! `FakeBd` sends those argv words and answers as bd did. The answers themselves are also
//! checked by the contract rule `scope_lists_answer_every_filter_variant`
//! (`tests/backend_contract.rs`).
#![allow(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test code: a failed expectation is the test failure"
)]

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use sc_bd_cli::testing::FakeBd;
use sc_bd_cli::{argv, BdCli};
use sc_beads::testing::contract::{recorded_filter, recorded_filter_names, FILTERS_LIST_ALL};
use sc_beads::testing::ManualClock;
use sc_beads::{parse_bd_rows, BeadsCacheError, CacheBackend, Deadline, ProjectRef, ScopeFilter};

const ARGV: &str = include_str!("../../sc-beads/tests/fixtures/bd-1.3.1/scope-filters/argv.json");

fn recorded_argv() -> BTreeMap<String, Vec<String>> {
    serde_json::from_str(ARGV).expect("argv.json is a map of argv lists")
}

fn built(name: &str, filter: &ScopeFilter) -> Vec<String> {
    if name.starts_with("blocked") {
        argv::blocked(filter)
    } else if name.starts_with("ready") {
        argv::ready(filter)
    } else {
        argv::list_scope(filter, None)
    }
}

/// Each variant's argv equals the argv bd 1.3.1 was recorded with (AC3).
#[test]
fn every_filter_variant_has_the_recorded_argv() {
    let recorded = recorded_argv();
    let names = recorded_filter_names();
    assert_eq!(
        names.len(),
        recorded.len(),
        "one recorded argv per recording"
    );
    for name in names {
        let filter = recorded_filter(name).expect("a filter for every recording");
        assert_eq!(
            built(name, &filter),
            recorded[name],
            "{name}: {filter:?} does not build the recorded argv"
        );
        // bd is never sent a status filter for a scope (ADR-BDC-0013).
        assert!(!recorded[name].iter().any(|w| w == "-s"), "{name} names -s");
    }
}

/// `BdCli` over `FakeBd` sends each recorded argv and stores nothing it was not asked.
#[test]
fn bd_cli_sends_the_recorded_argv_and_fake_bd_answers_each_variant() {
    let clock = Arc::new(ManualClock::default());
    let fake = FakeBd::new(Arc::clone(&clock));
    fake.backend()
        .insert_all(parse_bd_rows(FILTERS_LIST_ALL).expect("recorded rows validate"));
    let cli = BdCli::with_invoker(Box::new(fake.clone())).with_clock(Arc::clone(&clock) as _);
    let p = ProjectRef::local(None);
    let d = || Deadline::after(&*clock, Duration::from_secs(30));
    let recorded = recorded_argv();
    for name in recorded_filter_names() {
        let filter = recorded_filter(name).expect("a filter");
        let before = fake.calls().len();
        if name.starts_with("blocked") {
            cli.list_blocked(&p, &filter, d()).expect("blocked answers");
        } else if name.starts_with("ready") {
            cli.list_ready(&p, &filter, d()).expect("ready answers");
        } else {
            cli.list_scope(&p, &filter, None, d())
                .expect("list answers");
        }
        let calls = fake.calls();
        assert_eq!(calls.len(), before + 1, "{name}: one bd call");
        assert_eq!(calls.last(), Some(&recorded[name]), "{name}");
    }
}

/// An empty criteria filter is refused before any bd call (ADR-BDC-0013).
#[test]
fn an_empty_criteria_filter_starts_no_bd_process() {
    let clock = Arc::new(ManualClock::default());
    let fake = FakeBd::new(Arc::clone(&clock));
    let cli = BdCli::with_invoker(Box::new(fake.clone())).with_clock(Arc::clone(&clock) as _);
    let empty = ScopeFilter::Criteria {
        labels_all: Vec::new(),
        labels_any: Vec::new(),
        issue_type: None,
        assignee: None,
    };
    let p = ProjectRef::local(None);
    let d = Deadline::after(&*clock, Duration::from_secs(30));
    assert!(matches!(
        cli.list_scope(&p, &empty, None, d),
        Err(BeadsCacheError::Invalid(_))
    ));
    assert!(matches!(
        cli.list_blocked(&p, &empty, d),
        Err(BeadsCacheError::Invalid(_))
    ));
    assert!(matches!(
        cli.list_ready(&p, &empty, d),
        Err(BeadsCacheError::Invalid(_))
    ));
    assert!(fake.calls().is_empty());
}
