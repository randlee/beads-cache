//! `SqliteScheduleStore`: the store contract with fixed timestamps,
//! generation fencing, the table CHECKs and reopen durability. Ported from
//! atm-core's `atm-storage-rusqlite/tests/cron_schedule_store.rs`.

use std::path::{Path, PathBuf};
use std::time::Duration;

use rusqlite::Connection;
use sc_cron::{
    CronError, Generation, NewSchedule, RunOutcome, ScheduleId, ScheduleSpec, ScheduleStore,
    SqliteScheduleStore,
};
use tempfile::TempDir;

fn stamp(day: u32) -> String {
    format!("2026-10-{day:02}T12:00:00.000000Z")
}

fn new_schedule(name: &str) -> NewSchedule {
    NewSchedule {
        spec: ScheduleSpec {
            name: name.into(),
            expr: "*/5 * * * * *".into(),
            timeout: Duration::from_secs(30),
            payload: serde_json::json!({"goal": "x", "n": [1, {"y": null}]}),
        },
        jitter: Duration::from_millis(1799),
        next_run_at: stamp(20),
    }
}

fn generation(value: u32) -> Generation {
    Generation::new(value).expect("generation")
}

struct Harness {
    _root: TempDir,
    path: PathBuf,
    store: SqliteScheduleStore,
}

impl Harness {
    fn new() -> Self {
        let root = tempfile::tempdir().expect("tempdir");
        let path = root.path().join("cron.db");
        let store = SqliteScheduleStore::open(&path).expect("store");
        Self {
            _root: root,
            path,
            store,
        }
    }

    fn sql(&self) -> Connection {
        Connection::open(&self.path).expect("connection")
    }
}

/// Inserts a row by SQL with explicit end-state columns, for the CHECK tests.
fn sql_insert(
    path: &Path,
    name: &str,
    next_run_at: Option<&str>,
    ended_at: Option<&str>,
    stop_reason: Option<&str>,
) -> rusqlite::Result<usize> {
    Connection::open(path)?.execute(
        "INSERT INTO sc_cron_schedule
            (generation, name, expr, timeout_ms, jitter_ms, payload, created_at,
             next_run_at, ended_at, stop_reason)
         VALUES (1, ?1, '@hourly', 1000, 0, '{}', ?2, ?3, ?4, ?5)",
        rusqlite::params![name, stamp(1), next_run_at, ended_at, stop_reason],
    )
}

fn assert_check_violation(result: rusqlite::Result<usize>, case: &str) {
    let message = result.expect_err(case).to_string();
    assert!(
        message.contains("CHECK constraint failed"),
        "{case}: expected a CHECK violation, got: {message}"
    );
}

#[test]
fn create_round_trips_every_field_and_timestamps_are_kept_as_given() {
    let h = Harness::new();
    let created = h
        .store
        .create(&new_schedule("nightly"), &stamp(3))
        .expect("create");
    assert_eq!(created.generation, Generation::FIRST);
    assert_eq!(created.spec, new_schedule("nightly").spec);
    assert_eq!(created.jitter, Duration::from_millis(1799));
    assert_eq!(created.created_at, stamp(3));
    assert_eq!(created.next_run_at, Some(stamp(20)));
    assert_eq!(created.last_run_at, None);
    assert_eq!(created.last_outcome, None);
    assert!(created.is_active());
    assert_eq!(h.store.get(created.id).expect("get"), Some(created));
}

#[test]
fn list_active_survives_reopen_and_orders_by_created_at() {
    let h = Harness::new();
    h.store
        .create(&new_schedule("later"), &stamp(5))
        .expect("later");
    let ended = h
        .store
        .create(&new_schedule("ended"), &stamp(2))
        .expect("ended");
    h.store
        .create(&new_schedule("earlier"), &stamp(3))
        .expect("earlier");
    assert!(
        h.store
            .stop(ended.id, ended.generation, &stamp(6), "done")
            .expect("stop")
    );
    let before = h.store.list_active().expect("list");
    let names: Vec<_> = before.iter().map(|row| row.spec.name.as_str()).collect();
    assert_eq!(names, ["earlier", "later"]);

    let reopened = SqliteScheduleStore::open(&h.path).expect("reopen");
    assert_eq!(reopened.list_active().expect("list"), before);
}

#[test]
fn create_after_stop_restarts_in_place_and_raises_the_generation() {
    let h = Harness::new();
    let first = h
        .store
        .create(&new_schedule("nightly"), &stamp(3))
        .expect("create");
    assert!(
        h.store
            .record_run(first.id, first.generation, &stamp(4), &RunOutcome::Ok)
            .expect("record")
    );
    assert!(
        h.store
            .stop(first.id, first.generation, &stamp(4), "phase closed")
            .expect("stop")
    );
    let mut replacement = new_schedule("nightly");
    replacement.spec.expr = "0 15 * * * *".into();
    replacement.spec.timeout = Duration::from_secs(12);
    replacement.spec.payload = serde_json::json!({"replacement": true});
    replacement.jitter = Duration::from_secs(31);
    replacement.next_run_at = stamp(7);
    let restarted = h.store.create(&replacement, &stamp(6)).expect("restart");
    assert_eq!(restarted.id, first.id);
    assert_eq!(restarted.generation, generation(2));
    assert_eq!(restarted.spec, replacement.spec);
    assert_eq!(restarted.jitter, Duration::from_secs(31));
    assert_eq!(restarted.created_at, stamp(6));
    assert_eq!(restarted.next_run_at, Some(stamp(7)));
    assert_eq!(restarted.last_run_at, None);
    assert_eq!(restarted.last_outcome, None);
    assert_eq!(restarted.ended_at, None);
    assert_eq!(restarted.stop_reason, None);
}

#[test]
fn create_refuses_an_active_duplicate() {
    let h = Harness::new();
    h.store
        .create(&new_schedule("nightly"), &stamp(3))
        .expect("create");
    assert_eq!(
        h.store.create(&new_schedule("nightly"), &stamp(4)),
        Err(CronError::AlreadyActive("nightly".into()))
    );
    assert_eq!(h.store.list_active().expect("list").len(), 1);
}

#[test]
fn create_refuses_unstorable_fields() {
    let h = Harness::new();
    let mut zero = new_schedule("zero");
    zero.spec.timeout = Duration::ZERO;
    assert!(matches!(
        h.store.create(&zero, &stamp(3)),
        Err(CronError::InvalidSpec(_))
    ));
    let mut empty = new_schedule("");
    empty.spec.name = String::new();
    assert!(matches!(
        h.store.create(&empty, &stamp(3)),
        Err(CronError::InvalidSpec(_))
    ));
    assert!(matches!(
        h.store.create(&new_schedule("bad-time"), "yesterday"),
        Err(CronError::InvalidSpec(_))
    ));
    assert!(h.store.list_active().expect("list").is_empty());
}

#[test]
fn a_stale_generation_changes_nothing() {
    let h = Harness::new();
    let first = h
        .store
        .create(&new_schedule("nightly"), &stamp(3))
        .expect("create");
    assert!(
        h.store
            .stop(first.id, first.generation, &stamp(4), "end")
            .expect("stop")
    );
    let restarted = h
        .store
        .create(&new_schedule("nightly"), &stamp(6))
        .expect("restart");
    let stale = first.generation;
    assert!(
        !h.store
            .record_run(first.id, stale, &stamp(7), &RunOutcome::Ok)
            .expect("stale record")
    );
    assert!(
        !h.store
            .set_next(first.id, stale, &stamp(8))
            .expect("stale next")
    );
    assert!(
        !h.store
            .stop(first.id, stale, &stamp(7), "stale")
            .expect("stale stop")
    );
    assert_eq!(h.store.get(first.id).expect("get"), Some(restarted.clone()));

    assert!(
        h.store
            .record_run(
                first.id,
                restarted.generation,
                &stamp(8),
                &RunOutcome::TimedOut
            )
            .expect("current record")
    );
    let row = h.store.get(first.id).expect("get").expect("row");
    assert_eq!(row.last_outcome, Some(RunOutcome::TimedOut));
    assert_eq!(row.last_run_at, Some(stamp(8)));
}

#[test]
fn stop_ends_the_row_once_and_clears_the_next_run() {
    let h = Harness::new();
    let row = h
        .store
        .create(&new_schedule("nightly"), &stamp(3))
        .expect("create");
    assert!(
        h.store
            .stop(row.id, row.generation, &stamp(4), "phase closed")
            .expect("stop")
    );
    assert!(
        !h.store
            .stop(row.id, row.generation, &stamp(5), "again")
            .expect("again")
    );
    let ended = h.store.get(row.id).expect("get").expect("row");
    assert!(!ended.is_active());
    assert_eq!(ended.ended_at, Some(stamp(4)));
    assert_eq!(ended.stop_reason.as_deref(), Some("phase closed"));
    assert_eq!(ended.next_run_at, None);
    assert!(h.store.list_active().expect("list").is_empty());
}

#[test]
fn run_state_writes_on_an_ended_row_change_nothing() {
    let h = Harness::new();
    let row = h
        .store
        .create(&new_schedule("nightly"), &stamp(3))
        .expect("create");
    h.store
        .stop(row.id, row.generation, &stamp(4), "end")
        .expect("stop");
    assert!(
        !h.store
            .record_run(row.id, row.generation, &stamp(5), &RunOutcome::Ok)
            .expect("late run")
    );
    assert!(
        !h.store
            .set_next(row.id, row.generation, &stamp(6))
            .expect("late next")
    );
    let ended = h.store.get(row.id).expect("get").expect("row");
    assert_eq!(ended.last_outcome, None);
    assert_eq!(ended.next_run_at, None);
}

#[test]
fn set_next_stores_the_given_text() {
    let h = Harness::new();
    let row = h
        .store
        .create(&new_schedule("nightly"), &stamp(3))
        .expect("create");
    assert!(
        h.store
            .set_next(row.id, row.generation, &stamp(9))
            .expect("next")
    );
    let stored = h.store.get(row.id).expect("get").expect("row");
    assert_eq!(stored.next_run_at, Some(stamp(9)));
    assert!(matches!(
        h.store.set_next(row.id, row.generation, "soon"),
        Err(CronError::InvalidSpec(_))
    ));
}

#[test]
fn every_outcome_class_round_trips() {
    let h = Harness::new();
    let row = h
        .store
        .create(&new_schedule("nightly"), &stamp(3))
        .expect("create");
    for outcome in [
        RunOutcome::Ok,
        RunOutcome::Refused("quota: full".into()),
        RunOutcome::Failed("job panicked: boom".into()),
        RunOutcome::TimedOut,
    ] {
        assert!(
            h.store
                .record_run(row.id, row.generation, &stamp(4), &outcome)
                .expect("record")
        );
        let stored = h.store.get(row.id).expect("get").expect("row");
        assert_eq!(stored.last_outcome, Some(outcome));
    }
}

#[test]
fn an_unstorable_id_changes_nothing_and_reads_as_absent() {
    let h = Harness::new();
    let id = ScheduleId::new(u64::MAX);
    assert!(
        !h.store
            .record_run(id, Generation::FIRST, &stamp(4), &RunOutcome::Ok)
            .expect("record")
    );
    assert_eq!(h.store.get(id).expect("get"), None);
}

#[test]
fn end_state_checks_refuse_contradictory_rows() {
    let h = Harness::new();
    sql_insert(&h.path, "ok-active", Some(&stamp(2)), None, None).expect("valid active row");
    sql_insert(&h.path, "ok-ended", None, Some(&stamp(2)), Some("done")).expect("valid ended row");
    assert_check_violation(
        sql_insert(
            &h.path,
            "ended-with-next",
            Some(&stamp(2)),
            Some(&stamp(2)),
            Some("done"),
        ),
        "ended row with a next run",
    );
    assert_check_violation(
        sql_insert(&h.path, "ended-no-reason", None, Some(&stamp(2)), None),
        "ended row without a reason",
    );
    assert_check_violation(
        sql_insert(&h.path, "reason-not-ended", None, None, Some("done")),
        "active row with a reason",
    );
}

#[test]
fn a_corrupt_row_is_a_store_error_naming_the_row() {
    let h = Harness::new();
    let row = h
        .store
        .create(&new_schedule("nightly"), &stamp(3))
        .expect("create");
    h.sql()
        .execute("UPDATE sc_cron_schedule SET payload = 'not json'", [])
        .expect("corrupt payload");
    let error = h.store.list_active().expect_err("corrupt payload refused");
    assert!(
        matches!(&error, CronError::Store(message) if message.contains(&format!("sc_cron_schedule id {}", row.id))),
        "{error}"
    );
    h.sql()
        .execute(
            "UPDATE sc_cron_schedule SET payload = '{}', last_outcome = 'exit'",
            [],
        )
        .expect("corrupt outcome");
    assert!(matches!(h.store.get(row.id), Err(CronError::Store(_))));
}

#[test]
fn the_table_shares_a_database_file() {
    let h = Harness::new();
    h.sql()
        .execute_batch("CREATE TABLE other (x INTEGER); INSERT INTO other VALUES (1);")
        .expect("other table");
    let reopened = SqliteScheduleStore::open(&h.path).expect("reopen");
    reopened
        .create(&new_schedule("nightly"), &stamp(3))
        .expect("create");
    let count: i64 = h
        .sql()
        .query_row("SELECT count(*) FROM other", [], |row| row.get(0))
        .expect("other rows");
    assert_eq!(count, 1);
}
