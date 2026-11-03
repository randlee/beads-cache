//! Internal launch metadata for upstream's payload-only Rust job port.
//!
//! Only scheduler reads use this view. Public reads and persisted payloads stay
//! byte-for-byte unchanged. The wrapper derives identity and timing from the
//! actual row, so a payload on an unrelated row cannot impersonate a sync row.

use sc_cron::{
    CronError, Generation, NewSchedule, RunOutcome, Schedule, ScheduleId, ScheduleLimits,
    ScheduleName, ScheduleStore, Timestamp,
};
use serde_json::{Value, json};
use std::sync::Arc;

pub(crate) struct LaunchStore<S>(pub(crate) Arc<S>, pub(crate) crate::cron_log::ScheduleNames);

fn launch(mut row: Schedule, names: &crate::cron_log::ScheduleNames) -> Schedule {
    names
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(row.id.to_string(), row.spec.name.to_string());
    row.spec.payload = json!({
        "id": row.id.get(), "generation": row.generation.get(),
        "name": row.spec.name.as_str(), "expr": row.spec.expr.as_str(),
        "timeout_ms": row.spec.timeout.get().as_millis(),
        "period_ms": crate::schedule::row_period(&row).map(|d| d.as_millis()),
        "payload": row.spec.payload,
    });
    row
}

impl<S: ScheduleStore> ScheduleStore for LaunchStore<S> {
    fn limits(&self) -> ScheduleLimits {
        self.0.limits()
    }
    fn create(&self, schedule: &NewSchedule, at: Timestamp) -> Result<Schedule, CronError> {
        self.0.create(schedule, at)
    }
    fn get(&self, id: ScheduleId) -> Result<Option<Schedule>, CronError> {
        self.0.get(id).map(|r| r.map(|row| launch(row, &self.1)))
    }
    fn list_active(&self) -> Result<Vec<Schedule>, CronError> {
        self.0
            .list_active()
            .map(|rows| rows.into_iter().map(|row| launch(row, &self.1)).collect())
    }
    fn record_run(
        &self,
        id: ScheduleId,
        gen_: Generation,
        at: Timestamp,
        out: &RunOutcome,
    ) -> Result<bool, CronError> {
        self.0.record_run(id, gen_, at, out)
    }
    fn set_next(&self, id: ScheduleId, gen_: Generation, at: Timestamp) -> Result<bool, CronError> {
        self.0.set_next(id, gen_, at)
    }
    fn stop(
        &self,
        id: ScheduleId,
        gen_: Generation,
        at: Timestamp,
        reason: &str,
    ) -> Result<bool, CronError> {
        self.0.stop(id, gen_, at, reason)
    }
    fn set_enabled(&self, name: &ScheduleName, enabled: bool) -> Result<bool, CronError> {
        self.0.set_enabled(name, enabled)
    }
    fn disable(
        &self,
        id: ScheduleId,
        gen_: Generation,
        at: Timestamp,
        reason: &str,
    ) -> Result<bool, CronError> {
        self.0.disable(id, gen_, at, reason)
    }
    fn set_enabled_many(&self, flags: &[(ScheduleName, bool)]) -> Result<bool, CronError> {
        self.0.set_enabled_many(flags)
    }
    fn replace_and_set_enabled(
        &self,
        schedule: &NewSchedule,
        enabled: bool,
        at: Timestamp,
        flags: &[(ScheduleName, bool)],
    ) -> Result<Option<Schedule>, CronError> {
        self.0.replace_and_set_enabled(schedule, enabled, at, flags)
    }
}

pub(crate) fn key(payload: &Value) -> String {
    format!("{}:{}", payload["id"], payload["generation"])
}
