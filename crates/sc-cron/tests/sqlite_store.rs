//! `SqliteScheduleStore`: the store contract with fixed timestamps,
//! generation fencing, the table CHECKs, the schema version and reopen
//! durability. Ported from atm-core's
//! `atm-storage-rusqlite/tests/cron_schedule_store.rs`.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::clone_on_ref_ptr,
    clippy::unused_self,
    clippy::duration_suboptimal_units,
    clippy::unchecked_time_subtraction,
    reason = "test code: a failed expectation is the test failure"
)]

use std::error::Error;
use std::path::{Path, PathBuf};
use std::time::Duration;

use rusqlite::Connection;
use sc_cron::{
    CronError, Generation, MAX_PAYLOAD_BYTES, NewSchedule, RunOutcome, SCHEMA_VERSION, ScheduleId,
    ScheduleSpec, ScheduleStore, SqliteScheduleStore, StoreErrorKind, Timestamp,
};
use tempfile::TempDir;

fn stamp(day: u32) -> Timestamp {
    stamp_text(day).parse().expect("timestamp")
}

/// The fixed-width text the store writes for `stamp(day)`.
fn stamp_text(day: u32) -> String {
    format!("2026-10-{day:02}T12:00:00.000000Z")
}

fn new_schedule(name: &str) -> NewSchedule {
    NewSchedule {
        spec: ScheduleSpec::new(
            name,
            "*/5 * * * * *",
            Duration::from_secs(30),
            serde_json::json!({"goal": "x", "n": [1, {"y": null}]}),
        )
        .expect("spec"),
        jitter: Duration::from_millis(1799),
        next_run_at: stamp(20),
    }
}

fn generation(value: u32) -> Generation {
    Generation::new(value).expect("generation")
}

fn store_kind(result: Result<impl std::fmt::Debug, CronError>) -> StoreErrorKind {
    match result {
        Err(CronError::Store(error)) => error.kind(),
        other => panic!("expected a store error, got {other:?}"),
    }
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

    fn create(&self, name: &str, day: u32) -> sc_cron::Schedule {
        self.store
            .create(&new_schedule(name), stamp(day))
            .expect("create")
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
        rusqlite::params![name, stamp_text(1), next_run_at, ended_at, stop_reason],
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
    let created = h.create("nightly", 3);
    assert_eq!(created.generation, Generation::FIRST);
    assert_eq!(created.spec, new_schedule("nightly").spec);
    assert_eq!(created.jitter, Duration::from_millis(1799));
    assert_eq!(created.created_at, stamp(3));
    assert_eq!(created.next_run_at, Some(stamp(20)));
    assert_eq!(created.last_run_at, None);
    assert_eq!(created.last_outcome, None);
    assert!(created.is_active());
    assert_eq!(h.store.get(created.id).expect("get"), Some(created));
    let stored: String = h
        .sql()
        .query_row("SELECT created_at FROM sc_cron_schedule", [], |row| {
            row.get(0)
        })
        .expect("created_at");
    assert_eq!(stored, stamp_text(3));
}

#[test]
fn list_active_survives_reopen_and_orders_by_created_at() {
    let h = Harness::new();
    h.create("later", 5);
    let ended = h.create("ended", 2);
    h.create("earlier", 3);
    assert!(
        h.store
            .stop(ended.id, ended.generation, stamp(6), "done")
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
    let first = h.create("nightly", 3);
    assert!(
        h.store
            .record_run(first.id, first.generation, stamp(4), &RunOutcome::Ok)
            .expect("record")
    );
    assert!(
        h.store
            .stop(first.id, first.generation, stamp(4), "phase closed")
            .expect("stop")
    );
    let replacement = NewSchedule {
        spec: ScheduleSpec::new(
            "nightly",
            "0 15 * * * *",
            Duration::from_secs(12),
            serde_json::json!({"replacement": true}),
        )
        .expect("spec"),
        jitter: Duration::from_secs(31),
        next_run_at: stamp(7),
    };
    let restarted = h.store.create(&replacement, stamp(6)).expect("restart");
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
    h.create("nightly", 3);
    let error = h
        .store
        .create(&new_schedule("nightly"), stamp(4))
        .expect_err("duplicate");
    assert!(
        matches!(&error, CronError::AlreadyActive(name) if name.as_str() == "nightly"),
        "{error:?}"
    );
    assert_eq!(h.store.list_active().expect("list").len(), 1);
}

#[test]
fn create_refuses_an_over_size_payload() {
    let h = Harness::new();
    let mut big = new_schedule("big");
    big.spec.payload = serde_json::Value::String("x".repeat(MAX_PAYLOAD_BYTES));
    assert!(matches!(
        h.store.create(&big, stamp(3)),
        Err(CronError::InvalidSpec(_))
    ));
    assert!(h.store.list_active().expect("list").is_empty());
}

#[test]
fn a_stale_generation_changes_nothing() {
    let h = Harness::new();
    let first = h.create("nightly", 3);
    assert!(
        h.store
            .stop(first.id, first.generation, stamp(4), "end")
            .expect("stop")
    );
    let restarted = h.create("nightly", 6);
    let stale = first.generation;
    assert!(
        !h.store
            .record_run(first.id, stale, stamp(7), &RunOutcome::Ok)
            .expect("stale record")
    );
    assert!(
        !h.store
            .set_next(first.id, stale, stamp(8))
            .expect("stale next")
    );
    assert!(
        !h.store
            .stop(first.id, stale, stamp(7), "stale")
            .expect("stale stop")
    );
    assert_eq!(h.store.get(first.id).expect("get"), Some(restarted.clone()));

    assert!(
        h.store
            .record_run(
                first.id,
                restarted.generation,
                stamp(8),
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
    let row = h.create("nightly", 3);
    assert!(
        h.store
            .stop(row.id, row.generation, stamp(4), "phase closed")
            .expect("stop")
    );
    assert!(
        !h.store
            .stop(row.id, row.generation, stamp(5), "again")
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
    let row = h.create("nightly", 3);
    h.store
        .stop(row.id, row.generation, stamp(4), "end")
        .expect("stop");
    assert!(
        !h.store
            .record_run(row.id, row.generation, stamp(5), &RunOutcome::Ok)
            .expect("late run")
    );
    assert!(
        !h.store
            .set_next(row.id, row.generation, stamp(6))
            .expect("late next")
    );
    let ended = h.store.get(row.id).expect("get").expect("row");
    assert_eq!(ended.last_outcome, None);
    assert_eq!(ended.next_run_at, None);
}

#[test]
fn set_next_stores_the_given_instant() {
    let h = Harness::new();
    let row = h.create("nightly", 3);
    assert!(
        h.store
            .set_next(row.id, row.generation, stamp(9))
            .expect("next")
    );
    let stored = h.store.get(row.id).expect("get").expect("row");
    assert_eq!(stored.next_run_at, Some(stamp(9)));
}

#[test]
fn every_outcome_class_round_trips() {
    let h = Harness::new();
    let row = h.create("nightly", 3);
    for outcome in [
        RunOutcome::Ok,
        RunOutcome::Refused("quota: full".into()),
        RunOutcome::Failed("job panicked: boom".into()),
        RunOutcome::TimedOut,
    ] {
        assert!(
            h.store
                .record_run(row.id, row.generation, stamp(4), &outcome)
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
            .record_run(id, Generation::FIRST, stamp(4), &RunOutcome::Ok)
            .expect("record")
    );
    assert_eq!(h.store.get(id).expect("get"), None);
}

#[test]
fn table_checks_refuse_contradictory_and_over_size_rows() {
    let h = Harness::new();
    let next = stamp_text(2);
    sql_insert(&h.path, "ok-active", Some(&next), None, None).expect("valid active row");
    sql_insert(&h.path, "ok-ended", None, Some(&next), Some("done")).expect("valid ended row");
    assert_check_violation(
        sql_insert(
            &h.path,
            "ended-with-next",
            Some(&next),
            Some(&next),
            Some("done"),
        ),
        "ended row with a next run",
    );
    assert_check_violation(
        sql_insert(&h.path, "active-without-next", None, None, None),
        "active row without a next run",
    );
    assert_check_violation(
        sql_insert(&h.path, "ended-no-reason", None, Some(&next), None),
        "ended row without a reason",
    );
    assert_check_violation(
        sql_insert(
            &h.path,
            &"n".repeat(sc_cron::MAX_NAME_LEN + 1),
            Some(&next),
            None,
            None,
        ),
        "over-long name",
    );
}

#[test]
fn a_corrupt_row_is_a_corrupt_store_error_naming_the_row() {
    for (column, value) in [
        ("payload", "'not json'"),
        ("last_outcome", "'exit'"),
        ("next_run_at", "'soon'"),
        ("expr", "'* * * * *'"),
    ] {
        let fresh = Harness::new();
        let row = fresh.create("nightly", 3);
        fresh
            .sql()
            .execute(
                &format!("UPDATE sc_cron_schedule SET {column} = {value}"),
                [],
            )
            .expect("corrupt");
        let error = fresh.store.list_active().expect_err("corrupt row refused");
        assert!(
            matches!(&error, CronError::Store(store)
                if store.kind() == StoreErrorKind::Corrupt
                    && store.context().contains(&format!("sc_cron_schedule id {}", row.id))),
            "{column}: {error}"
        );
        assert_eq!(store_kind(fresh.store.get(row.id)), StoreErrorKind::Corrupt);
    }
}

#[test]
fn a_file_with_other_tables_is_refused() {
    let root = tempfile::tempdir().expect("tempdir");
    let path = root.path().join("cache.db");
    Connection::open(&path)
        .expect("connection")
        .execute_batch("CREATE TABLE beads (id TEXT);")
        .expect("foreign table");
    assert_eq!(
        store_kind(SqliteScheduleStore::open(&path)),
        StoreErrorKind::SchemaMismatch
    );
}

#[test]
fn an_unknown_schema_version_is_refused_and_the_current_one_is_written() {
    let h = Harness::new();
    let version: i64 = h
        .sql()
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .expect("version");
    assert_eq!(version, SCHEMA_VERSION);
    h.sql()
        .pragma_update(None, "user_version", SCHEMA_VERSION + 1)
        .expect("bump version");
    let error = SqliteScheduleStore::open(&h.path).expect_err("unknown version");
    assert_eq!(error.code(), "STORE_FAILED");
    assert!(
        matches!(&error, CronError::Store(store) if store.kind() == StoreErrorKind::SchemaMismatch),
        "{error}"
    );
}

#[test]
fn store_errors_expose_their_rusqlite_source() {
    let root = tempfile::tempdir().expect("tempdir");
    let missing = root.path().join("no-such-dir").join("cron.db");
    let error = SqliteScheduleStore::open(&missing).expect_err("cannot open");
    assert!(
        matches!(&error, CronError::Store(store) if store.kind() == StoreErrorKind::Unavailable),
        "{error}"
    );
    let store = error.source().expect("store error");
    assert!(store.source().is_some(), "rusqlite error is preserved");
}
