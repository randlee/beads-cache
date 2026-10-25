//! Named size and time limits on schedule input.
//!
//! The limits bound what one schedule can cost: every reload reads every
//! active row, payload included, into memory. They are enforced once, by
//! the validated types ([`ScheduleName`](crate::ScheduleName),
//! [`CronExpr`](crate::CronExpr), [`RunTimeout`](crate::RunTimeout)) and by
//! the payload check that both [`Scheduler::start`](crate::Scheduler::start)
//! and [`SqliteScheduleStore`](crate::SqliteScheduleStore) apply, and again
//! by table CHECKs.
//!
//! The number of active schedules is not capped. `sc-cron` is an
//! in-process library and every schedule is created by its own caller, not
//! by remote input. With each row bounded here, memory is linear in a count
//! the caller controls. A caller that accepts schedules from untrusted input
//! must cap the count itself.

use std::time::Duration;

/// Longest schedule name, in bytes.
pub const MAX_NAME_LEN: usize = 128;

/// Longest cron expression, in bytes.
pub const MAX_EXPR_LEN: usize = 256;

/// Largest payload, in bytes of its compact JSON encoding.
pub const MAX_PAYLOAD_BYTES: usize = 64 * 1024;

/// Longest run timeout.
pub const MAX_TIMEOUT: Duration = Duration::from_hours(24);

/// Encodes `payload` as compact JSON, refusing it above [`MAX_PAYLOAD_BYTES`].
pub(crate) fn encode_payload(payload: &serde_json::Value) -> Result<String, crate::CronError> {
    let encoded = payload.to_string();
    if encoded.len() > MAX_PAYLOAD_BYTES {
        return Err(crate::CronError::InvalidSpec(format!(
            "payload is {} bytes; the limit is {MAX_PAYLOAD_BYTES}",
            encoded.len()
        )));
    }
    Ok(encoded)
}
