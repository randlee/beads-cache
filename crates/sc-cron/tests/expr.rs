//! Expression and spec validation through `Scheduler::start`: everything is
//! refused before anything is stored. Ported from atm-cron's `tests/expr.rs`.

use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use sc_cron::{
    CronError, MAX_TIMEOUT, ManualClock, ScheduleSpec, ScheduleStore, Scheduler, SeqJitter,
    SqliteScheduleStore,
};

struct Fixture {
    store: Arc<SqliteScheduleStore>,
    scheduler: Scheduler<SqliteScheduleStore, ManualClock, SeqJitter>,
}

fn fixture() -> Fixture {
    let store = Arc::new(SqliteScheduleStore::open_in_memory().expect("store"));
    let clock = Arc::new(ManualClock::new(
        "2026-10-19T00:00:00Z".parse().expect("timestamp"),
    ));
    let scheduler = Scheduler::new(store.clone(), clock, Arc::new(SeqJitter::default()));
    Fixture { store, scheduler }
}

fn spec(expr: &str) -> ScheduleSpec {
    ScheduleSpec {
        name: "test".into(),
        expr: expr.into(),
        timeout: Duration::from_secs(1),
        payload: serde_json::json!({}),
    }
}

/// Starts `spec` and, on refusal, checks that nothing was stored.
fn start(spec: ScheduleSpec) -> Result<(), CronError> {
    let fixture = fixture();
    let result = fixture.scheduler.start(spec).map(drop);
    if result.is_err() {
        assert!(
            fixture.store.list_active().expect("list").is_empty(),
            "a refused spec stores nothing"
        );
    }
    result
}

fn bad_expression(expr: &str) -> String {
    match start(spec(expr)) {
        Err(CronError::BadExpression(message)) => message,
        other => panic!("{expr}: expected BadExpression, got {other:?}"),
    }
}

#[test]
fn accepts_six_and_seven_fields_and_macros() {
    for expr in [
        "0 * * * * *",
        "0 * * * * * *",
        "@daily",
        "@hourly",
        "@weekly",
        "@monthly",
        "@yearly",
        "  */30 * * * * *  ",
    ] {
        start(spec(expr)).unwrap_or_else(|error| panic!("{expr}: {error}"));
    }
}

#[test]
fn refuses_five_fields_with_the_accepted_form_named() {
    assert!(bad_expression("* * * * *").contains("6 or 7 fields"));
}

#[test]
fn refuses_garbage_and_out_of_range_with_the_parser_message() {
    for expr in ["x y", "0 0 25 * * *"] {
        let message = bad_expression(expr);
        let parser = cron::Schedule::from_str(expr).expect_err("parser refuses it");
        assert!(message.contains(&parser.to_string()), "{expr}: {message}");
    }
}

#[test]
fn refuses_an_expression_with_no_future_occurrence() {
    assert!(bad_expression("0 0 0 1 1 * 2020").contains("no future occurrence"));
}

#[test]
fn refuses_an_empty_name_and_out_of_range_timeouts_before_storing() {
    let mut empty = spec("@hourly");
    empty.name = "  ".into();
    assert!(matches!(start(empty), Err(CronError::InvalidSpec(_))));
    for timeout in [
        Duration::ZERO,
        Duration::from_micros(999),
        MAX_TIMEOUT + Duration::from_millis(1),
    ] {
        let mut bad = spec("@hourly");
        bad.timeout = timeout;
        assert!(
            matches!(start(bad), Err(CronError::InvalidSpec(_))),
            "{timeout:?}"
        );
    }
}

#[test]
fn an_active_name_is_refused_and_nothing_changes() {
    let fixture = fixture();
    let first = fixture.scheduler.start(spec("@hourly")).expect("first");
    assert_eq!(
        fixture.scheduler.start(spec("@daily")),
        Err(CronError::AlreadyActive("test".into()))
    );
    assert_eq!(fixture.store.list_active().expect("list"), [first]);
}

#[test]
fn start_returns_the_expression_byte_identical() {
    let fixture = fixture();
    let schedule = fixture
        .scheduler
        .start(spec(" 0 */5 * * * * "))
        .expect("start");
    assert_eq!(schedule.spec.expr, " 0 */5 * * * * ");
}
