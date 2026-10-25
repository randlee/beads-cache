//! Acceptance 3 and 4 of sprint b-3a: the `fake-bd` process behaves like the in-process
//! `FakeBd` (state persists between processes), and its injected faults reach `BdCli`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a failed unwrap or index is a test failure"
)]

mod support;

use std::time::{Duration, Instant};

use bd_cache_cli::scenario::{Faults, RefuseWrite, Scenario};
use sc_bd_cli::BdCli;
use sc_beads::testing::actor;
use sc_beads::{
    Bead, BeadsCacheError, CacheBackend, Clock, Deadline, Guards, Patch, ProjectRef, Refusal,
    ScopeFilter, SystemClock, schema_sha256,
};
use support::{LABEL, TestRepo, in_process_backend, labelled, two_beads};

/// A generous deadline for calls that must succeed.
const LONG: Duration = Duration::from_secs(30);

fn deadline(timeout: Duration) -> Deadline {
    Deadline::after(&SystemClock as &dyn Clock, timeout)
}

fn list(bd: &BdCli, project: &ProjectRef, timeout: Duration) -> Result<Vec<Bead>, BeadsCacheError> {
    bd.list_scope(
        project,
        &ScopeFilter::Label(LABEL.to_owned()),
        None,
        deadline(timeout),
    )
}

fn start_b1(bd: &BdCli, project: &ProjectRef, timeout: Duration) -> Result<(), BeadsCacheError> {
    let patch = Patch {
        status: Some("in_progress".to_owned()),
        ..Patch::default()
    };
    bd.update_guarded(
        project,
        &sc_beads::testing::id("b-1"),
        &patch,
        &Guards::default(),
        &actor("tester"),
        deadline(timeout),
    )
}

/// Runs list, update, list on the process backend and on the in-process one, and
/// returns the first difference. `forget` is called between the update and the second
/// list; it is a no-op in the real test and the planted "no persistence" in the negative.
fn sequence_matches(repo: &TestRepo, forget: impl FnOnce(&TestRepo)) -> Result<(), String> {
    let seed = repo.scenario();
    let reference = in_process_backend(&seed);
    let reference_project = ProjectRef::local(None);
    let process = repo.process_backend();
    let process_project = repo.project();

    let first_ref = list(&reference, &reference_project, LONG).map_err(|e| e.to_string())?;
    let first = list(&process, &process_project, LONG).map_err(|e| e.to_string())?;
    if first != first_ref {
        return Err(format!("first list differs: {first:?} vs {first_ref:?}"));
    }
    start_b1(&reference, &reference_project, LONG).map_err(|e| e.to_string())?;
    start_b1(&process, &process_project, LONG).map_err(|e| e.to_string())?;
    forget(repo);
    let second_ref = list(&reference, &reference_project, LONG).map_err(|e| e.to_string())?;
    let second = list(&process, &process_project, LONG).map_err(|e| e.to_string())?;
    if second != second_ref {
        return Err(format!("second list differs: {second:?} vs {second_ref:?}"));
    }
    if second == first {
        return Err("the update changed nothing, so the sequence proves nothing".to_owned());
    }
    Ok(())
}

#[test]
fn list_update_list_gives_the_same_rows_through_the_process_and_in_process() {
    let repo = TestRepo::new(&two_beads());
    sequence_matches(&repo, |_| {}).expect("process and in-process agree");
    // The second process really saw the first one's write.
    let after = list(&repo.process_backend(), &repo.project(), LONG).expect("a list");
    let b1 = after.iter().find(|b| b.id.as_str() == "b-1").expect("b-1");
    assert_eq!(b1.status.as_deref(), Some("in_progress"));
    assert_eq!(repo.scenario().rows.len(), 2);
}

/// The planted negative of acceptance 3: a fake bd that does not persist between
/// calls makes the second list differ, and the sequence check reports it.
#[test]
fn a_fake_bd_that_forgets_its_writes_fails_the_sequence() {
    let repo = TestRepo::new(&two_beads());
    let original = repo.scenario();
    let err = sequence_matches(&repo, |repo| repo.set_scenario(&original))
        .expect_err("the second list must differ");
    assert!(err.contains("second list differs"), "{err}");
}

#[test]
fn the_scenario_clock_is_fixed_across_processes() {
    let mut scenario = two_beads();
    scenario.now = "2031-01-02T03:04:05Z".to_owned();
    let repo = TestRepo::new(&scenario);
    start_b1(&repo.process_backend(), &repo.project(), LONG).expect("an update");
    let rows = list(&repo.process_backend(), &repo.project(), LONG).expect("a list");
    let b1 = rows.iter().find(|b| b.id.as_str() == "b-1").expect("b-1");
    assert_eq!(b1.updated_at, "2031-01-02T03:04:05Z");
    // The untouched bead keeps the time it was seeded with.
    let b2 = rows.iter().find(|b| b.id.as_str() == "b-2").expect("b-2");
    assert_ne!(b2.updated_at, b1.updated_at);
}

#[test]
fn version_schema_and_the_blocked_set_come_from_the_scenario() {
    let mut scenario = two_beads();
    scenario.version = Some("1.3.9".to_owned());
    scenario.schema = Some("{\"schema_version\": 7}".to_owned());
    scenario
        .blocked
        .insert("b-2".to_owned(), vec!["b-1".to_owned()]);
    let repo = TestRepo::new(&scenario);

    let version = repo.fake_bd(&["version"]);
    assert_eq!(version.exit, 0, "{version:?}");
    assert!(version.stdout.contains("1.3.9"), "{version:?}");

    let schema = repo.fake_bd(&["schema"]);
    assert_eq!(
        schema_sha256(&schema.stdout),
        schema_sha256("{\"schema_version\": 7}")
    );

    let blocked = repo
        .process_backend()
        .list_blocked(
            &repo.project(),
            &ScopeFilter::Label(LABEL.to_owned()),
            deadline(LONG),
        )
        .expect("the blocked set");
    let ids: Vec<&str> = blocked.iter().map(|b| b.id.as_str()).collect();
    assert_eq!(ids, ["b-2"]);
    assert_eq!(
        blocked[0]
            .blocked_by
            .iter()
            .map(sc_beads::BeadId::as_str)
            .collect::<Vec<_>>(),
        ["b-1"]
    );
}

#[test]
fn a_missing_or_broken_scenario_is_a_harness_error_not_a_bd_answer() {
    let repo = TestRepo::new(&two_beads());
    std::fs::write(repo.scenario_path(), "{\"nope\": 1}").expect("write");
    let ran = repo.fake_bd(&["version"]);
    assert_eq!(ran.exit, bd_cache_cli::scenario::EXIT_HARNESS, "{ran:?}");
    assert!(ran.stderr.contains("scenario"), "{ran:?}");
}

// ---- faults (acceptance 4) -------------------------------------------------------

fn with_faults(faults: Faults) -> Scenario {
    let mut scenario = two_beads();
    scenario.faults = faults;
    scenario
}

#[test]
fn an_unavailable_scenario_makes_bdcli_return_the_unreachable_error() {
    let repo = TestRepo::new(&with_faults(Faults {
        unavailable: true,
        ..Faults::default()
    }));
    let err = list(&repo.process_backend(), &repo.project(), LONG).expect_err("unreachable");
    assert!(err.is_unavailable(), "{err:?}");
    assert!(!err.is_timeout(), "an outage is not a timeout: {err:?}");
    // Writes see the outage too, and change nothing.
    let err = start_b1(&repo.process_backend(), &repo.project(), LONG).expect_err("unreachable");
    assert!(err.is_unavailable(), "{err:?}");
    assert_eq!(repo.scenario().rows, two_beads().rows);
}

#[test]
fn the_same_scenario_without_the_fault_answers() {
    // The control of the fault tests: only the fault field differs.
    let repo = TestRepo::new(&with_faults(Faults::default()));
    let rows = list(&repo.process_backend(), &repo.project(), LONG).expect("a list");
    assert_eq!(rows, vec![labelled("b-1"), labelled("b-2")]);
}

#[test]
fn slow_by_500_ms_with_a_100_ms_call_timeout_times_out() {
    // Start the binary once beforehand (a scenario without the fault), so the first
    // execution of a fresh binary is not what uses up the 100 ms.
    assert_eq!(TestRepo::new(&two_beads()).fake_bd(&["version"]).exit, 0);
    let repo = TestRepo::new(&with_faults(Faults {
        slow_ms: 500,
        ..Faults::default()
    }));
    let started = Instant::now();
    let err = list(
        &repo.process_backend(),
        &repo.project(),
        Duration::from_millis(100),
    )
    .expect_err("the call must time out");
    assert!(err.is_timeout(), "{err:?}");
    // The caller gave up on the child; it did not wait for the 500 ms to pass. Only
    // asserted on Unix: killing a process tree on Windows (`taskkill /T`) can itself
    // take a few hundred milliseconds.
    if cfg!(unix) {
        assert!(
            started.elapsed() < Duration::from_millis(500),
            "{:?}",
            started.elapsed()
        );
    }
}

#[test]
fn slow_by_500_ms_with_a_long_timeout_answers_after_the_delay() {
    // The control: the delay is real (at least 500 ms) and the answer is bd's.
    let repo = TestRepo::new(&with_faults(Faults {
        slow_ms: 500,
        ..Faults::default()
    }));
    let started = Instant::now();
    let rows = list(&repo.process_backend(), &repo.project(), LONG).expect("a slow answer");
    assert!(started.elapsed() >= Duration::from_millis(500));
    assert_eq!(rows.len(), 2);
}

#[test]
fn a_refused_write_carries_its_exit_code_and_changes_nothing() {
    let repo = TestRepo::new(&with_faults(Faults {
        refuse_write: Some(RefuseWrite { exit_code: 13 }),
        ..Faults::default()
    }));
    let err = start_b1(&repo.process_backend(), &repo.project(), LONG).expect_err("refused");
    assert!(
        matches!(err, BeadsCacheError::Refused(Refusal::Precondition)),
        "{err:?}"
    );
    assert_eq!(repo.scenario().rows, two_beads().rows);
    // Reads are not refused.
    assert_eq!(
        list(&repo.process_backend(), &repo.project(), LONG)
            .expect("a list")
            .len(),
        2
    );

    // Another exit code is a failure with that code.
    let repo = TestRepo::new(&with_faults(Faults {
        refuse_write: Some(RefuseWrite { exit_code: 7 }),
        ..Faults::default()
    }));
    let err = start_b1(&repo.process_backend(), &repo.project(), LONG).expect_err("refused");
    assert!(
        matches!(err, BeadsCacheError::Failed { code: 7, .. }),
        "{err:?}"
    );
}

// ---- the documented format -------------------------------------------------------

#[test]
fn the_scenario_example_in_the_architecture_doc_is_a_valid_scenario() {
    let doc = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../docs/bd-cache-cli/architecture.md"
    ))
    .expect("the architecture doc");
    let example = doc
        .split("```json\n")
        .nth(1)
        .and_then(|rest| rest.split("\n```").next())
        .expect("a json example");
    let scenario: Scenario = serde_json::from_str(example).expect("the example is a scenario");
    assert_eq!(scenario.rows.len(), 2);
    assert_eq!(scenario.faults.slow_ms, 500);
    // A key the format does not have is rejected, not ignored.
    let typo = example.replace("slow_ms", "slow_millis");
    assert!(serde_json::from_str::<Scenario>(&typo).is_err());
    // The example applies; a blocked bead that is not a row fails loudly.
    scenario.fake_bd().expect("the example applies");
    let mut unknown = scenario;
    unknown
        .blocked
        .insert("b-9".to_owned(), vec!["b-1".to_owned()]);
    assert!(matches!(
        unknown.fake_bd(),
        Err(bd_cache_cli::scenario::ScenarioError::UnknownBead(id)) if id == "b-9"
    ));
}
