//! Expression and spec validation: everything is refused before anything is
//! stored; five-field crontab expressions are read with crontab's meaning
//! (REQ-CRON-0001, REQ-CRON-0015).

#![expect(
    clippy::expect_used,
    clippy::panic,
    clippy::clone_on_ref_ptr,
    reason = "test code: a failed expectation is the test failure"
)]

use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use sc_cron::{
    CronError, CronExpr, MAX_EXPR_LEN, MAX_NAME_LEN, MAX_PAYLOAD_BYTES, MAX_TIMEOUT, ManualClock,
    ScheduleSpec, ScheduleStore, Scheduler, SeqJitter, SqliteScheduleStore,
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

fn spec(expr: &str) -> Result<ScheduleSpec, CronError> {
    ScheduleSpec::new("test", expr, Duration::from_secs(1), serde_json::json!({}))
}

/// Builds and starts `expr`; on refusal, checks that nothing was stored.
fn start(expr: &str) -> Result<(), CronError> {
    let fixture = fixture();
    let result = spec(expr).and_then(|spec| fixture.scheduler.start(spec).map(drop));
    if result.is_err() {
        assert!(
            fixture.store.list_active().expect("list").is_empty(),
            "a refused spec stores nothing"
        );
    }
    result
}

fn bad_expression(expr: &str) -> String {
    match start(expr) {
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
        start(expr).unwrap_or_else(|error| panic!("{expr}: {error}"));
    }
}

#[test]
fn accepts_standard_five_field_crontab_expressions() {
    for expr in [
        "* * * * *",
        "*/5 * * * *",
        "30 9 * * 1-5",
        "0 0 1,15 * *",
        "0 0 * jan mon",
        "0 12 * * sun,sat",
        "  0 8 * * 7  ",
    ] {
        start(expr).unwrap_or_else(|error| panic!("{expr}: {error}"));
    }
}

#[test]
fn still_refuses_four_fields_and_a_bad_five_field_expression() {
    for expr in ["* * * *", "*/5 * *"] {
        bad_expression(expr);
    }
    for expr in [
        "61 * * * *",
        "0 24 * * *",
        "0 0 32 * *",
        "0 0 * 13 *",
        "0 0 * * 8",
        "0 0 * * 5-1",
        "0 0 * * */0",
        "0 0 * * 3/2",
        "0 0 * * funday",
        "0 0 * * 5L",
    ] {
        bad_expression(expr);
    }
}

/// The first run `start` stores for `expr` when the clock reads `now`.
fn first_run(expr: &str, now: &str) -> String {
    let store = Arc::new(SqliteScheduleStore::open_in_memory().expect("store"));
    let clock = Arc::new(ManualClock::new(now.parse().expect("timestamp")));
    let scheduler = Scheduler::new(store, clock, Arc::new(SeqJitter::default()));
    let schedule = scheduler.start(spec(expr).expect("spec")).expect("start");
    schedule.next_run_at.expect("next run").to_string()
}

#[test]
fn five_fields_fire_at_second_zero_of_the_named_minute() {
    // 2026-10-19 is a Monday.
    assert_eq!(
        first_run("*/15 * * * *", "2026-10-19T00:07:30Z"),
        "2026-10-19T00:15:00.000000Z"
    );
    assert_eq!(
        first_run("30 9 * * *", "2026-10-19T09:30:00Z"),
        "2026-10-20T09:30:00.000000Z"
    );
}

#[test]
fn five_field_day_of_week_counts_monday_as_one() {
    // 2026-10-17 is a Saturday: Mon-Fri next fires on Monday the 19th.
    assert_eq!(
        first_run("0 9 * * 1-5", "2026-10-17T12:00:00Z"),
        "2026-10-19T09:00:00.000000Z"
    );
    // Sunday is 0 and 7; 2026-10-18 is a Sunday.
    assert_eq!(
        first_run("0 9 * * 0", "2026-10-17T12:00:00Z"),
        "2026-10-18T09:00:00.000000Z"
    );
    assert_eq!(
        first_run("0 9 * * 7", "2026-10-17T12:00:00Z"),
        "2026-10-18T09:00:00.000000Z"
    );
    // Names mean the same as numbers.
    assert_eq!(
        first_run("0 9 * * wed", "2026-10-19T12:00:00Z"),
        "2026-10-21T09:00:00.000000Z"
    );
}

#[test]
fn five_field_restricted_day_of_month_and_day_of_week_match_either() {
    // The 13th or a Friday. From Saturday 2026-10-10 the 13th (a Tuesday)
    // comes before the next Friday, the 16th; after the 13th, the Friday.
    assert_eq!(
        first_run("0 0 13 * 5", "2026-10-10T12:00:00Z"),
        "2026-10-13T00:00:00.000000Z"
    );
    assert_eq!(
        first_run("0 0 13 * 5", "2026-10-13T12:00:00Z"),
        "2026-10-16T00:00:00.000000Z"
    );
    // With either field open only the other restricts; a field that starts
    // with `*` is open, so `*/5` and Friday must both match: the 16th.
    assert_eq!(
        first_run("0 0 13 * *", "2026-10-10T12:00:00Z"),
        "2026-10-13T00:00:00.000000Z"
    );
    assert_eq!(
        first_run("0 0 */5 * 5", "2026-10-10T12:00:00Z"),
        "2026-10-16T00:00:00.000000Z"
    );
}

#[test]
fn six_field_day_of_week_keeps_the_cron_crates_numbering() {
    // Seconds-first expressions are read as the `cron` crate reads them,
    // where 1 is Sunday: 2026-10-17 is a Saturday, so the next day 1 is the 18th.
    assert_eq!(
        first_run("0 0 9 * * 1", "2026-10-17T12:00:00Z"),
        "2026-10-18T09:00:00.000000Z"
    );
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
fn start_refuses_an_expression_with_no_future_occurrence_before_storing() {
    // The syntax is valid, so the spec builds; `start` refuses it.
    assert!(spec("0 0 0 1 1 * 2020").is_ok());
    assert!(bad_expression("0 0 0 1 1 * 2020").contains("no future occurrence"));
}

#[test]
fn refuses_invalid_names_timeouts_and_lengths() {
    let invalid = |result: Result<ScheduleSpec, CronError>, case: &str| {
        assert!(
            matches!(result, Err(CronError::InvalidSpec(_))),
            "{case}: {result:?}"
        );
    };
    let payload = serde_json::json!({});
    for name in ["", "  ", " padded", &"n".repeat(MAX_NAME_LEN + 1)] {
        invalid(
            ScheduleSpec::new(name, "@hourly", Duration::from_secs(1), payload.clone()),
            &format!("name {name:?}"),
        );
    }
    for timeout in [
        Duration::ZERO,
        Duration::from_micros(999),
        MAX_TIMEOUT + Duration::from_millis(1),
    ] {
        invalid(
            ScheduleSpec::new("test", "@hourly", timeout, payload.clone()),
            &format!("timeout {timeout:?}"),
        );
    }
    let long_expr = format!("@hourly{}", " ".repeat(MAX_EXPR_LEN));
    invalid(
        ScheduleSpec::new("test", &long_expr, Duration::from_secs(1), payload),
        "long expression",
    );
}

#[test]
fn an_over_size_payload_is_refused_by_the_spec_and_by_start() {
    let big = serde_json::Value::String("x".repeat(MAX_PAYLOAD_BYTES));
    assert!(matches!(
        ScheduleSpec::new("test", "@hourly", Duration::from_secs(1), big.clone()),
        Err(CronError::InvalidSpec(_))
    ));
    // The field is public, so `start` checks it again with the same rule.
    let fixture = fixture();
    let mut spec = spec("@hourly").expect("spec");
    spec.payload = big;
    assert!(matches!(
        fixture.scheduler.start(spec),
        Err(CronError::InvalidSpec(_))
    ));
    assert!(fixture.store.list_active().expect("list").is_empty());
}

#[test]
fn an_active_name_is_refused_and_nothing_changes() {
    let fixture = fixture();
    let first = fixture
        .scheduler
        .start(spec("@hourly").expect("spec"))
        .expect("first");
    let error = fixture
        .scheduler
        .start(spec("@daily").expect("spec"))
        .expect_err("duplicate refused");
    assert!(
        matches!(&error, CronError::AlreadyActive(name) if name.as_str() == "test"),
        "{error:?}"
    );
    assert_eq!(error.code(), "ALREADY_ACTIVE");
    assert_eq!(fixture.store.list_active().expect("list"), [first]);
}

#[test]
fn the_expression_is_kept_byte_identical() {
    let fixture = fixture();
    let schedule = fixture
        .scheduler
        .start(spec(" 0 */5 * * * * ").expect("spec"))
        .expect("start");
    assert_eq!(schedule.spec.expr.as_str(), " 0 */5 * * * * ");
    assert_eq!(
        schedule.spec.expr,
        CronExpr::parse(" 0 */5 * * * * ").expect("expr")
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn async_start_and_stop_run_off_the_executor() {
    let fixture = fixture();
    let schedule = fixture
        .scheduler
        .start_async(spec("@hourly").expect("spec"))
        .await
        .expect("start");
    fixture
        .scheduler
        .stop_async(schedule.id, schedule.generation, "done")
        .await
        .expect("stop");
    let error = fixture
        .scheduler
        .stop_async(schedule.id, schedule.generation, "again")
        .await
        .expect_err("stale stop");
    assert!(
        matches!(error, CronError::Fenced { id, generation }
            if id == schedule.id && generation == schedule.generation),
        "{error:?}"
    );
}
