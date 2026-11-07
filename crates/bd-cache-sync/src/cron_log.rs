//! Bridges upstream canonical scheduler events to the existing tracing surface.

use sc_cron::observability::{CanonicalLog, EventError, LogEvent, ServiceName, SharedCanonicalLog};

pub(crate) type ScheduleNames =
    std::sync::Arc<std::sync::Mutex<std::collections::HashMap<String, String>>>;

struct CronLog(ServiceName, ScheduleNames);

impl CanonicalLog for CronLog {
    fn service_name(&self) -> &ServiceName {
        &self.0
    }

    fn emit(&self, event: LogEvent) -> Result<(), EventError> {
        let canonical_action = event.action.to_string();
        let action = match canonical_action.as_str() {
            "run.launched" | "due.launched" => "launched",
            "run.finished" | "due.finished" => "recorded",
            other => other,
        };
        let name = event
            .fields
            .get("schedule")
            .and_then(serde_json::Value::as_str)
            .and_then(|id| self.1.try_lock().ok()?.get(id).cloned())
            .unwrap_or_default();
        let outcome = event
            .fields
            .get("error")
            .or_else(|| event.fields.get("reason"))
            .and_then(serde_json::Value::as_str)
            .map_or_else(
                || {
                    event
                        .outcome
                        .as_ref()
                        .map_or_else(String::new, ToString::to_string)
                },
                str::to_owned,
            );
        // Canonical fields are upstream's already-redacted event projection.
        // Keep legacy action labels so existing operators retain their queries.
        tracing::info!(action, canonical_action, name, outcome, fields = %serde_json::Value::Object(event.fields), "scheduler event");
        Ok(())
    }
}

#[expect(
    clippy::expect_used,
    reason = "the fixed bd-cache service name is valid under the shared validator; existing logging tests construct this receiver"
)]
pub(crate) fn logger(names: ScheduleNames) -> SharedCanonicalLog {
    std::sync::Arc::new(CronLog(
        ServiceName::new("bd-cache").expect("fixed bd-cache service identity is valid"),
        names,
    ))
}
