//! `cron run` logs (REQ-BDC-0730): sc-cron's `launched` and `recorded`
//! events, and one event per run step with the row and the `run_id`, from
//! every thread (the subscriber is the process default, as `cron run`
//! installs it). One test per binary: the default can be set once.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code"
)]

mod common;

use std::sync::{Arc, Mutex};

use bd_cache_sync::{BUILTIN_ROW, ScheduleSet, cron_run_once, schedule_set};
use common::repo::Repo;
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

/// One event's fields.
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

#[tokio::test(flavor = "multi_thread")]
async fn a_run_logs_sc_crons_events_and_one_event_per_step_with_the_row_and_run_id() {
    let capture = Capture::default();
    tracing::subscriber::set_global_default(capture.clone()).unwrap();
    let repo = Repo::new(vec![common::label_scope("lbl", "area")]);
    schedule_set(
        &repo.scheduler,
        &ScheduleSet {
            expr: "0 * * * * *".into(),
            ..ScheduleSet::default()
        },
    )
    .await
    .unwrap();
    repo.cron_clock.advance(chrono::Duration::seconds(60));
    cron_run_once(&repo.scheduler, Arc::new(repo.ctx()))
        .await
        .unwrap();
    let events = capture.0.lock().unwrap().clone();
    let action = |a: &str| events.iter().any(|e| get(e, "action") == Some(a));
    assert!(
        action("launched") && action("recorded"),
        "sc-cron's events: {events:?}"
    );
    let steps: Vec<&str> = events
        .iter()
        .filter(|e| get(e, "row") == Some(BUILTIN_ROW))
        .filter_map(|e| get(e, "step"))
        .collect();
    assert_eq!(steps, ["lease", "drain", "scope", "reply"], "{events:?}");
    let ids: std::collections::BTreeSet<&str> = events
        .iter()
        .filter(|e| get(e, "step").is_some())
        .filter_map(|e| get(e, "run_id"))
        .collect();
    assert_eq!(ids.len(), 1, "one run id on every step: {ids:?}");
}
