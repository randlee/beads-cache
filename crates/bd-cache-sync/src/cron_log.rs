//! Bridges upstream canonical scheduler events to the existing tracing surface.

use sc_command::observability::{CanonicalLog, NoopCanonicalLog, SharedCanonicalLog};
use sc_observability_types::{LogEvent, ServiceName, v2};

pub(crate) type ScheduleNames =
    std::sync::Arc<std::sync::Mutex<std::collections::HashMap<String, String>>>;

struct CronLog(NoopCanonicalLog, ScheduleNames);

impl CanonicalLog for CronLog {
    fn service_name(&self) -> &ServiceName {
        self.0.service_name()
    }

    fn emit(&self, event: LogEvent) -> Result<(), v2::EventError> {
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

pub(crate) fn logger(names: ScheduleNames) -> SharedCanonicalLog {
    std::sync::Arc::new(CronLog(
        NoopCanonicalLog::named_or_default("bd-cache"),
        names,
    ))
}
