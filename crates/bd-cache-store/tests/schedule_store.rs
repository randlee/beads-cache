//! `CacheScheduleStore`: sc-cron's store contract against the schedule rows in the cache
//! file, over the cache's one connection (ADR-BDC-0014, NFR-BDC-0007 as amended).
//!
//! The cases are sc-cron's REQ-CRON-0006 (create, restart, read back), 0007 (generation
//! fencing), 0008 (no clock read: timestamps kept as given), 0014 (the `enabled` flag;
//! `set_next` refused while disabled) and 0017 (the two multi-row methods), plus the
//! cache file's own rules: the table lives in the cache file at `user_version` 3, the
//! process holds one connection to the file, the claim outlives a dropped `SqliteStore`
//! while the view lives, and a file whose `sc_cron_schedule_version` row names another
//! version is refused before any schedule is served.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a failed unwrap is a test failure"
)]

mod common;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use bd_cache_model::{CacheStore, Scope, ScopeId};
use bd_cache_store::{
    CacheScheduleStore, SC_CRON_SCHEDULE_VERSION_KEY, SCHEMA_VERSION, SqliteStore,
};
use common::{TempDir, identity};
use rusqlite::Connection;
use sc_cron::{
    CronError, Generation, NewSchedule, RunOutcome, ScheduleName, ScheduleSpec, ScheduleStore,
    StoreErrorKind, Timestamp,
};

fn file(tmp: &TempDir) -> PathBuf {
    tmp.path().join(".bd-cache").join("testdb.bd-cache.sqlite3")
}

fn stamp(day: u32) -> Timestamp {
    format!("2026-10-{day:02}T12:00:00.000000Z")
        .parse()
        .expect("timestamp")
}

fn name(text: &str) -> ScheduleName {
    ScheduleName::new(text).expect("name")
}

fn new_schedule(n: &str) -> NewSchedule {
    NewSchedule {
        spec: ScheduleSpec::new(
            n,
            "*/5 * * * * *",
            Duration::from_secs(30),
            serde_json::json!({"kind": "builtin"}),
        )
        .expect("spec"),
        jitter: Duration::from_millis(1799),
        next_run_at: stamp(20),
    }
}

/// A store and its schedule view on a fresh cache file.
struct Harness {
    // The views drop before the temp dir, so the file closes first.
    view: CacheScheduleStore,
    _store: SqliteStore,
    path: PathBuf,
    _tmp: TempDir,
}

impl Harness {
    fn new() -> Self {
        let tmp = TempDir::new("schedule");
        let path = file(&tmp);
        let store = SqliteStore::open_at(&path, &identity()).unwrap();
        let view = store.schedule_store().unwrap();
        Self {
            view,
            _store: store,
            path,
            _tmp: tmp,
        }
    }

    fn create(&self, n: &str, day: u32) -> sc_cron::Schedule {
        self.view.create(&new_schedule(n), stamp(day)).unwrap()
    }
}

/// A second connection to the file, as another process has.
fn other(path: &Path) -> Connection {
    let c = Connection::open(path).unwrap();
    c.busy_timeout(Duration::from_secs(5)).unwrap();
    c
}

// --- REQ-CRON-0006 / 0008: the store contract, timestamps as given ---

#[test]
fn create_reads_back_every_field_and_keeps_timestamps_as_given() {
    let h = Harness::new();
    let s = h.create("builtin", 3);
    assert_eq!(s.generation, Generation::FIRST);
    assert_eq!(s.spec, new_schedule("builtin").spec);
    assert_eq!((s.created_at, s.next_run_at), (stamp(3), Some(stamp(20))));
    assert!(s.is_active() && s.enabled);
    assert_eq!(h.view.get(s.id).unwrap(), Some(s.clone()));
    assert_eq!(h.view.list_active().unwrap(), vec![s.clone()]);
    let again = h.view.create(&new_schedule("builtin"), stamp(4));
    assert!(
        matches!(again, Err(CronError::AlreadyActive(_))),
        "{again:?}"
    );
    // Ended, then restarted in place: same id, next generation.
    assert!(h.view.stop(s.id, s.generation, stamp(5), "done").unwrap());
    assert!(h.view.list_active().unwrap().is_empty());
    let restarted = h.create("builtin", 6);
    assert_eq!((restarted.id, restarted.generation.get()), (s.id, 2));
    let stored: String = other(&h.path)
        .query_row("SELECT created_at FROM sc_cron_schedule", [], |r| r.get(0))
        .unwrap();
    assert_eq!(
        stored, "2026-10-06T12:00:00.000000Z",
        "kept as given (REQ-CRON-0008)"
    );
}

#[test]
fn a_payload_over_the_limit_is_refused_before_any_write() {
    let h = Harness::new();
    let mut big = new_schedule("big");
    big.spec.payload = serde_json::json!("x".repeat(sc_cron::MAX_PAYLOAD_BYTES));
    let refused = h.view.create(&big, stamp(1));
    assert!(
        matches!(refused, Err(CronError::InvalidSpec(_))),
        "{refused:?}"
    );
    assert!(h.view.list_active().unwrap().is_empty());
}

// --- REQ-CRON-0007: fencing ---

#[test]
fn a_stale_generation_is_fenced_on_every_run_state_write() {
    let h = Harness::new();
    let s = h.create("builtin", 1);
    assert!(
        h.view
            .record_run(s.id, s.generation, stamp(2), &RunOutcome::Ok)
            .unwrap()
    );
    assert!(
        h.view
            .stop(s.id, s.generation, stamp(3), "restart")
            .unwrap()
    );
    let fresh = h.create("builtin", 4);
    let old = s.generation;
    assert!(
        !h.view
            .record_run(s.id, old, stamp(5), &RunOutcome::Ok)
            .unwrap()
    );
    assert!(!h.view.set_next(s.id, old, stamp(6)).unwrap());
    assert!(!h.view.stop(s.id, old, stamp(7), "late").unwrap());
    assert!(!h.view.disable(s.id, old, stamp(7), "late").unwrap());
    assert_eq!(h.view.get(fresh.id).unwrap(), Some(fresh));
}

// --- REQ-CRON-0014: the enabled flag ---

#[test]
fn set_enabled_flips_only_the_flag_and_set_next_is_refused_while_disabled() {
    let h = Harness::new();
    let s = h.create("builtin", 1);
    assert!(h.view.set_enabled(&name("builtin"), false).unwrap());
    let off = h.view.get(s.id).unwrap().unwrap();
    assert_eq!(
        off,
        sc_cron::Schedule {
            enabled: false,
            ..s.clone()
        }
    );
    assert!(
        !h.view.set_next(s.id, s.generation, stamp(9)).unwrap(),
        "set_next is refused while disabled"
    );
    assert!(h.view.set_enabled(&name("builtin"), true).unwrap());
    assert!(h.view.set_next(s.id, s.generation, stamp(9)).unwrap());
    assert!(!h.view.set_enabled(&name("nobody"), true).unwrap());
    assert!(
        h.view
            .disable(s.id, s.generation, stamp(10), "stop hook")
            .unwrap()
    );
    let disabled = h.view.get(s.id).unwrap().unwrap();
    assert!(!disabled.enabled);
    assert_eq!(
        disabled.last_outcome,
        Some(RunOutcome::Refused("stop hook".into()))
    );
}

// --- REQ-CRON-0017: the two multi-row methods ---

/// Every pair of flags a second connection read while `flip` ran.
fn poll_while(path: &Path, names: [&str; 2], flip: impl FnOnce()) -> Vec<(bool, bool)> {
    let path = path.to_path_buf();
    let done = Arc::new(AtomicBool::new(false));
    let stop = Arc::clone(&done);
    let names = names.map(str::to_owned);
    let poller = std::thread::spawn(move || {
        let c = other(&path);
        let mut seen = Vec::new();
        while !stop.load(Ordering::SeqCst) {
            let pair: (bool, bool) = c
                .query_row(
                    "SELECT (SELECT enabled FROM sc_cron_schedule WHERE name = ?1),
                            (SELECT enabled FROM sc_cron_schedule WHERE name = ?2)",
                    [&names[0], &names[1]],
                    |r| {
                        Ok((
                            r.get::<_, Option<bool>>(0)?.unwrap_or(false),
                            r.get::<_, Option<bool>>(1)?.unwrap_or(false),
                        ))
                    },
                )
                .unwrap();
            seen.push(pair);
        }
        seen
    });
    flip();
    done.store(true, Ordering::SeqCst);
    poller.join().unwrap()
}

#[test]
fn set_enabled_many_flips_two_rows_seen_whole_by_a_second_connection() {
    let h = Harness::new();
    let a = h.create("a", 1);
    h.create("b", 2);
    let seen = poll_while(&h.path, ["a", "b"], || {
        for round in 0..200 {
            let on = round % 2 == 1;
            assert!(
                h.view
                    .set_enabled_many(&[(name("a"), on), (name("b"), on)])
                    .unwrap()
            );
        }
    });
    assert!(!seen.is_empty());
    assert!(
        seen.iter().all(|(x, y)| x == y),
        "one row seen flipped without the other"
    );
    let after = h.view.get(a.id).unwrap().unwrap();
    // In place: only the flag (REQ-CRON-0014).
    assert_eq!(after, sc_cron::Schedule { enabled: true, ..a });
}

#[test]
fn set_enabled_many_naming_a_missing_row_changes_nothing() {
    let h = Harness::new();
    let a = h.create("a", 1);
    let b = h.create("b", 2);
    assert!(
        !h.view
            .set_enabled_many(&[(name("a"), false), (name("missing"), false)])
            .unwrap()
    );
    assert!(h.view.get(a.id).unwrap().unwrap().enabled);
    assert!(h.view.get(b.id).unwrap().unwrap().enabled);
}

#[test]
fn replace_and_set_enabled_never_shows_both_rows_enabled() {
    let h = Harness::new();
    h.create("builtin", 1);
    let seen = poll_while(&h.path, ["builtin", "python"], || {
        for round in 0..100 {
            let python_on = round % 2 == 0;
            let written = h
                .view
                .replace_and_set_enabled(
                    &new_schedule("python"),
                    python_on,
                    stamp(2),
                    &[(name("builtin"), !python_on)],
                )
                .unwrap()
                .expect("builtin is active");
            assert_eq!(written.enabled, python_on);
        }
    });
    assert!(!seen.is_empty());
    assert!(
        !seen.iter().any(|(b, p)| *b && *p),
        "both rows seen enabled"
    );
}

#[test]
fn replace_of_an_active_row_keeps_its_id_raises_its_generation_and_keeps_the_flag() {
    let h = Harness::new();
    let first = h.create("python", 1);
    assert!(
        h.view
            .record_run(first.id, first.generation, stamp(2), &RunOutcome::Ok)
            .unwrap()
    );
    let replaced = h
        .view
        .replace_and_set_enabled(&new_schedule("python"), false, stamp(3), &[])
        .unwrap()
        .unwrap();
    assert_eq!(
        (replaced.id, replaced.generation.get(), replaced.enabled),
        (first.id, 2, false)
    );
    assert_eq!((replaced.last_run_at, replaced.last_outcome), (None, None));
    assert!(
        !h.view
            .record_run(first.id, first.generation, stamp(4), &RunOutcome::Ok)
            .unwrap(),
        "a run of the old generation is fenced"
    );
}

#[test]
fn replace_and_set_enabled_naming_a_missing_row_changes_nothing() {
    let h = Harness::new();
    let builtin = h.create("builtin", 1);
    let refused = h
        .view
        .replace_and_set_enabled(
            &new_schedule("python"),
            true,
            stamp(2),
            &[(name("builtin"), false), (name("missing"), false)],
        )
        .unwrap();
    assert_eq!(refused, None);
    assert_eq!(h.view.list_active().unwrap(), vec![builtin]);
    let itself = h.view.replace_and_set_enabled(
        &new_schedule("python"),
        true,
        stamp(2),
        &[(name("python"), false)],
    );
    assert!(
        matches!(itself, Err(CronError::InvalidSpec(_))),
        "{itself:?}"
    );
}

// --- the cache file's own rules (ADR-BDC-0014, AC7) ---

/// A v3 cache file with scopes gains `sc_cron_schedule` and stays `user_version` 3.
#[test]
fn a_v3_cache_file_with_scopes_gains_the_schedule_table_at_version_3() {
    let tmp = TempDir::new("schedule-v3");
    let path = file(&tmp);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    {
        let c = Connection::open(&path).unwrap();
        c.execute_batch(include_str!("fixtures/v3-schema.sql"))
            .unwrap();
        c.pragma_update(None, "user_version", 3).unwrap();
        c.execute(
            "INSERT INTO sync_state(phase, last_started_at, last_ok_at) \
             VALUES ('bk', '2026-10-23T10:00:00Z', '2026-10-23T10:00:00Z')",
            [],
        )
        .unwrap();
    }
    let mut store = SqliteStore::open_at(&path, &identity()).unwrap();
    store
        .ensure_scope(&Scope::legacy(ScopeId::new("bk").unwrap()))
        .unwrap();
    let view = store.schedule_store().unwrap();
    view.create(&new_schedule("builtin"), stamp(1)).unwrap();
    drop((view, store));
    let c = Connection::open(&path).unwrap();
    let version: i32 = c
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(version, SCHEMA_VERSION);
    let rows: i64 = c
        .query_row("SELECT COUNT(*) FROM sc_cron_schedule", [], |r| r.get(0))
        .unwrap();
    assert_eq!(rows, 1);
    let marker: String = c
        .query_row(
            "SELECT value FROM meta WHERE key = ?1",
            [SC_CRON_SCHEDULE_VERSION_KEY],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(marker, "2");
}

/// While the store and its view are open the process holds one connection to the
/// file: a second open is `STORE_IN_USE`; after the store is dropped with the view
/// alive, a second open is still `STORE_IN_USE` (planted negative: keep the claim in
/// `SqliteStore`, and the second assertion fails).
#[test]
fn the_view_keeps_the_one_connection_claim_after_the_store_drops() {
    let tmp = TempDir::new("schedule-claim");
    let path = file(&tmp);
    let store = SqliteStore::open_at(&path, &identity()).unwrap();
    let view = store.schedule_store().unwrap();
    let second = SqliteStore::open_at(&path, &identity()).unwrap_err();
    assert_eq!(second.code(), "BD_CACHE.STORE_IN_USE");
    drop(store);
    let after = SqliteStore::open_at(&path, &identity()).unwrap_err();
    assert_eq!(
        after.code(),
        "BD_CACHE.STORE_IN_USE",
        "the view holds the claim after the store drops"
    );
    // The view still serves schedules on the shared connection.
    view.create(&new_schedule("builtin"), stamp(1)).unwrap();
    drop(view);
    assert!(
        SqliteStore::open_at(&path, &identity()).is_ok(),
        "released when the last holder drops"
    );
}

/// A file whose `sc_cron_schedule_version` row names another version is refused
/// before any schedule is served.
#[test]
fn another_schedule_table_version_is_refused_before_any_schedule_is_served() {
    let tmp = TempDir::new("schedule-version");
    let path = file(&tmp);
    drop(SqliteStore::open_at(&path, &identity()).unwrap());
    Connection::open(&path)
        .unwrap()
        .execute(
            "UPDATE meta SET value = '3' WHERE key = ?1",
            [SC_CRON_SCHEDULE_VERSION_KEY],
        )
        .unwrap();
    let store = SqliteStore::open_at(&path, &identity()).unwrap();
    let refused = store.schedule_store().unwrap_err();
    assert!(
        matches!(&refused, CronError::Store(e) if e.kind() == StoreErrorKind::SchemaMismatch),
        "{refused}"
    );
    // The open itself does not overwrite another version.
    drop(store);
    let kept: String = Connection::open(&path)
        .unwrap()
        .query_row(
            "SELECT value FROM meta WHERE key = ?1",
            [SC_CRON_SCHEDULE_VERSION_KEY],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(kept, "3");
}

// --- sc-cron's SqliteScheduleStore contract cases, ported (B7Q1-010) ---

/// The fixed-width text the store writes for `stamp(day)`.
fn stamp_text(day: u32) -> String {
    format!("2026-10-{day:02}T12:00:00.000000Z")
}

/// The DDL of `sc_cron_schedule` in the file at `path`, whitespace collapsed.
fn schedule_ddl(c: &Connection) -> String {
    let sql: String = c
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'sc_cron_schedule'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    sql.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The cache file's `sc_cron_schedule` is sc-cron's own table: its DDL equals
/// the one `SqliteScheduleStore` creates, after whitespace normalisation, so
/// the hand-kept copy cannot drift from sc-cron's (ADR-BDC-0014).
#[test]
fn the_cache_files_schedule_table_is_sc_crons_own_ddl() {
    let h = Harness::new();
    let tmp = TempDir::new("sc-cron-own");
    let own = tmp.path().join("cron.db");
    drop(sc_cron::SqliteScheduleStore::open(&own).unwrap());
    assert_eq!(
        schedule_ddl(&other(&h.path)),
        schedule_ddl(&Connection::open(&own).unwrap())
    );
}

#[test]
fn list_active_survives_reopen_and_orders_by_created_at() {
    let tmp = TempDir::new("schedule-reopen");
    let path = file(&tmp);
    let before = {
        let store = SqliteStore::open_at(&path, &identity()).unwrap();
        let view = store.schedule_store().unwrap();
        view.create(&new_schedule("later"), stamp(5)).unwrap();
        let ended = view.create(&new_schedule("ended"), stamp(2)).unwrap();
        view.create(&new_schedule("earlier"), stamp(3)).unwrap();
        assert!(
            view.stop(ended.id, ended.generation, stamp(6), "done")
                .unwrap()
        );
        let before = view.list_active().unwrap();
        let names: Vec<_> = before.iter().map(|r| r.spec.name.as_str()).collect();
        assert_eq!(names, ["earlier", "later"]);
        before
    };
    let store = SqliteStore::open_at(&path, &identity()).unwrap();
    assert_eq!(
        store.schedule_store().unwrap().list_active().unwrap(),
        before
    );
}

#[test]
fn create_after_stop_restarts_in_place_with_every_field_from_the_new_row() {
    let h = Harness::new();
    let first = h.create("nightly", 3);
    assert!(
        h.view
            .record_run(first.id, first.generation, stamp(4), &RunOutcome::Ok)
            .unwrap()
    );
    assert!(
        h.view
            .stop(first.id, first.generation, stamp(4), "closed")
            .unwrap()
    );
    let replacement = NewSchedule {
        spec: ScheduleSpec::new(
            "nightly",
            "0 15 * * * *",
            Duration::from_secs(12),
            serde_json::json!({"replacement": true}),
        )
        .unwrap(),
        jitter: Duration::from_secs(31),
        next_run_at: stamp(7),
    };
    let restarted = h.view.create(&replacement, stamp(6)).unwrap();
    assert_eq!((restarted.id, restarted.generation.get()), (first.id, 2));
    assert_eq!(restarted.spec, replacement.spec);
    assert_eq!(restarted.jitter, Duration::from_secs(31));
    assert_eq!(
        (restarted.created_at, restarted.next_run_at),
        (stamp(6), Some(stamp(7)))
    );
    assert_eq!(
        (restarted.last_run_at, restarted.last_outcome),
        (None, None)
    );
    assert_eq!((restarted.ended_at, restarted.stop_reason), (None, None));
    assert!(restarted.enabled, "a restarted name is enabled again");
}

#[test]
fn run_state_writes_on_an_ended_row_change_nothing() {
    let h = Harness::new();
    let row = h.create("nightly", 3);
    assert!(
        h.view
            .stop(row.id, row.generation, stamp(4), "end")
            .unwrap()
    );
    assert!(
        !h.view
            .record_run(row.id, row.generation, stamp(5), &RunOutcome::Ok)
            .unwrap()
    );
    assert!(!h.view.set_next(row.id, row.generation, stamp(6)).unwrap());
    assert!(
        !h.view
            .stop(row.id, row.generation, stamp(7), "again")
            .unwrap()
    );
    let ended = h.view.get(row.id).unwrap().unwrap();
    assert_eq!((ended.last_outcome, ended.next_run_at), (None, None));
    assert_eq!(ended.stop_reason.as_deref(), Some("end"));
}

#[test]
fn set_next_stores_the_given_instant() {
    let h = Harness::new();
    let row = h.create("nightly", 3);
    assert!(h.view.set_next(row.id, row.generation, stamp(9)).unwrap());
    assert_eq!(
        h.view.get(row.id).unwrap().unwrap().next_run_at,
        Some(stamp(9))
    );
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
            h.view
                .record_run(row.id, row.generation, stamp(4), &outcome)
                .unwrap()
        );
        assert_eq!(
            h.view.get(row.id).unwrap().unwrap().last_outcome,
            Some(outcome)
        );
    }
}

#[test]
fn an_unstorable_id_changes_nothing_and_reads_as_absent() {
    let h = Harness::new();
    let id = sc_cron::ScheduleId::new(u64::MAX);
    assert!(
        !h.view
            .record_run(id, Generation::FIRST, stamp(4), &RunOutcome::Ok)
            .unwrap()
    );
    assert_eq!(h.view.get(id).unwrap(), None);
}

/// Inserts a row by SQL with explicit end-state columns, for the CHECK tests.
fn sql_insert(
    c: &Connection,
    name: &str,
    next_run_at: Option<&str>,
    ended_at: Option<&str>,
    stop_reason: Option<&str>,
) -> rusqlite::Result<usize> {
    c.execute(
        "INSERT INTO sc_cron_schedule
            (generation, name, expr, timeout_ms, jitter_ms, payload, created_at,
             next_run_at, ended_at, stop_reason)
         VALUES (1, ?1, '@hourly', 1000, 0, '{}', ?2, ?3, ?4, ?5)",
        rusqlite::params![name, stamp_text(1), next_run_at, ended_at, stop_reason],
    )
}

#[test]
fn table_checks_refuse_contradictory_and_over_size_rows() {
    let h = Harness::new();
    let c = other(&h.path);
    let next = stamp_text(2);
    sql_insert(&c, "ok-active", Some(&next), None, None).unwrap();
    sql_insert(&c, "ok-ended", None, Some(&next), Some("done")).unwrap();
    let long = "n".repeat(sc_cron::MAX_NAME_LEN + 1);
    for (case, r) in [
        (
            "ended with a next run",
            sql_insert(&c, "a", Some(&next), Some(&next), Some("done")),
        ),
        (
            "active without a next run",
            sql_insert(&c, "b", None, None, None),
        ),
        (
            "ended without a reason",
            sql_insert(&c, "c", None, Some(&next), None),
        ),
        (
            "over-long name",
            sql_insert(&c, &long, Some(&next), None, None),
        ),
    ] {
        let message = r.expect_err(case).to_string();
        assert!(
            message.contains("CHECK constraint failed"),
            "{case}: {message}"
        );
    }
    let enabled = c.execute("UPDATE sc_cron_schedule SET enabled = 2", []);
    assert!(enabled.is_err(), "enabled accepts only 0 or 1");
}

/// A row that does not decode is named by `get` and skipped by
/// `list_active`, whose good rows still list (schedule.rs's skip path).
#[test]
fn a_corrupt_row_is_named_by_get_and_skipped_by_list_active() {
    for (column, value) in [
        ("payload", "'not json'"),
        ("last_outcome", "'exit'"),
        ("next_run_at", "'soon'"),
        ("expr", "'* * * *'"),
        ("payload", "x'00ff'"),
    ] {
        let h = Harness::new();
        let bad = h.create("bad", 3);
        let good = h.create("good", 4);
        other(&h.path)
            .execute(
                &format!("UPDATE sc_cron_schedule SET {column} = {value} WHERE id = ?1"),
                [i64::try_from(bad.id.get()).unwrap()],
            )
            .unwrap();
        let e = h.view.get(bad.id).unwrap_err();
        assert!(
            matches!(&e, CronError::Store(s) if s.kind() == StoreErrorKind::Corrupt),
            "{column}: {e}"
        );
        // Twice: the second reload skips the same row again.
        for _ in 0..2 {
            assert_eq!(
                h.view.list_active().unwrap(),
                vec![good.clone()],
                "{column}"
            );
        }
    }
}

/// A storage failure is a store error that keeps its rusqlite source.
#[test]
fn store_errors_expose_their_rusqlite_source() {
    use std::error::Error as _;
    let h = Harness::new();
    other(&h.path)
        .execute_batch("DROP TABLE sc_cron_schedule;")
        .unwrap();
    let e = h.view.list_active().unwrap_err();
    assert!(matches!(&e, CronError::Store(_)), "{e}");
    let store = e.source().expect("the store error");
    assert!(store.source().is_some(), "the rusqlite error is preserved");
}

// --- REQ-CRON-0017: one transaction, checked deterministically (B7Q1-027) ---

/// A planted failure on the second row's flag update rolls the first row's
/// back too: `set_enabled_many` is one transaction (the polling test above
/// can only make a split write likely to be seen; this one always sees it).
#[test]
fn set_enabled_many_rolls_back_whole_when_the_second_row_fails() {
    let h = Harness::new();
    let a = h.create("a", 1);
    let b = h.create("b", 2);
    other(&h.path)
        .execute_batch(
            "CREATE TRIGGER planted BEFORE UPDATE OF enabled ON sc_cron_schedule \
             WHEN NEW.name = 'b' BEGIN SELECT RAISE(ABORT, 'planted failure'); END;",
        )
        .unwrap();
    assert!(
        h.view
            .set_enabled_many(&[(name("a"), false), (name("b"), false)])
            .is_err()
    );
    assert!(h.view.get(a.id).unwrap().unwrap().enabled, "a rolled back");
    assert!(h.view.get(b.id).unwrap().unwrap().enabled);
}

/// A planted failure on the other row's flag rolls the written row back:
/// `replace_and_set_enabled` is one transaction.
#[test]
fn replace_and_set_enabled_rolls_back_whole_when_the_other_flag_fails() {
    let h = Harness::new();
    let builtin = h.create("builtin", 1);
    other(&h.path)
        .execute_batch(
            "CREATE TRIGGER planted BEFORE UPDATE OF enabled ON sc_cron_schedule \
             WHEN NEW.name = 'builtin' BEGIN SELECT RAISE(ABORT, 'planted failure'); END;",
        )
        .unwrap();
    let r = h.view.replace_and_set_enabled(
        &new_schedule("python"),
        true,
        stamp(2),
        &[(name("builtin"), false)],
    );
    assert!(r.is_err(), "{r:?}");
    assert_eq!(
        h.view.list_active().unwrap(),
        vec![builtin],
        "no python row"
    );
}
