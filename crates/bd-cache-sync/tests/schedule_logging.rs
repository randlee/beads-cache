//! The `sync use-python` log events carry the run's `run_id` and the row
//! (PE1-RSH-008): an operator ties the restore warning to the run's step
//! events by id, not by name and time. One test per binary: the default
//! subscriber can be set once.

#![cfg(unix)]
#![expect(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: a failed unwrap is a test failure"
)]

mod common;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use bd_cache_sync::{PYTHON_ROW, ScheduleSet, StopFlag, UsePython};
use common::repo::{PyStub, Repo};
use sc_cron::ScheduleStore as _;
use tracing::field::{Field, Visit};
use tracing::span::{Attributes, Id, Record};
use tracing::{Event, Metadata, Subscriber};

#[derive(Default)]
struct Fields(Vec<(String, String)>);

impl Visit for Fields {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.0.push((
            field.name().to_owned(),
            format!("{value:?}").trim_matches('"').to_owned(),
        ));
    }
    fn record_str(&mut self, field: &Field, value: &str) {
        self.0.push((field.name().to_owned(), value.to_owned()));
    }
}

type Recorded = Vec<(String, String)>;

#[derive(Clone, Default)]
struct Capture(Arc<Mutex<Vec<Recorded>>>);

impl Subscriber for Capture {
    fn enabled(&self, _: &Metadata<'_>) -> bool {
        true
    }
    fn new_span(&self, _: &Attributes<'_>) -> Id {
        Id::from_u64(1)
    }
    fn record(&self, _: &Id, _: &Record<'_>) {}
    fn record_follows_from(&self, _: &Id, _: &Id) {}
    fn event(&self, event: &Event<'_>) {
        let mut fields = Fields::default();
        event.record(&mut fields);
        self.0.lock().unwrap().push(fields.0);
    }
    fn enter(&self, _: &Id) {}
    fn exit(&self, _: &Id) {}
}

fn get<'a>(e: &'a [(String, String)], k: &str) -> Option<&'a str> {
    e.iter().find(|(n, _)| n == k).map(|(_, v)| v.as_str())
}

/// A failed immediate run whose row another command replaced is not
/// restored, and says so with the row and the run's id (the id its error
/// reports). Planted negative: without the two fields the event is not found
/// with them.
#[tokio::test(flavor = "current_thread")]
async fn the_not_restored_warning_names_the_row_and_the_run() {
    let capture = Capture::default();
    tracing::subscriber::set_global_default(capture.clone()).unwrap();
    let repo = Repo::new(vec![]);
    let stub = PyStub::new();
    stub.set("tick", "sleep");
    let schedules = repo.schedules();
    schedules
        .set(&ScheduleSet {
            expr: "0 0 * * * *".into(),
            budget_ms: None,
            hook_reserve_ms: None,
            post_sync: None,
            python: None,
            env: vec![],
        })
        .await
        .unwrap();
    let request = UsePython {
        python: stub.python.clone(),
        module: "consumer.sync".into(),
        cron: Some("0 0 * * * *".to_owned()),
        timeout_ms: Some(1_000),
        env: vec![],
    };
    let run = schedules.use_python(&request, StopFlag::new());
    // Another command ends the Python row while the immediate run is in flight.
    let interfere = async {
        while stub.count("tick") == 0 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        let python = repo
            .store
            .list_active()
            .unwrap()
            .into_iter()
            .find(|s| s.spec.name.as_str() == PYTHON_ROW)
            .unwrap();
        let at = sc_cron::Timestamp::from_datetime(chrono::Utc::now());
        repo.store
            .stop(python.id, python.generation, at, "ended by another command")
            .unwrap()
    };
    let (run, interfered) = tokio::join!(run, interfere);
    assert!(interfered, "the row was ended");

    let events = capture.0.lock().unwrap().clone();
    let warned: Vec<&Recorded> = events
        .iter()
        .filter(|e| {
            get(e, "message").is_some_and(|m| m.contains("another command changed the Python row"))
        })
        .collect();
    assert_eq!(warned.len(), 1, "{events:?}");
    assert_eq!(get(warned[0], "row"), Some(PYTHON_ROW));
    // The id is the failed run's own, the one its error reports.
    let Err(bd_cache_model::CacheError::TickRunFailed { run_id, .. }) = &run else {
        panic!("expected TICK_RUN_FAILED, got {run:?}");
    };
    assert_eq!(get(warned[0], "run_id"), Some(run_id.to_string().as_str()));
}
