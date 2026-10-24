//! The crate's one error type.
//!
//! `CronError` is deliberately small: a caller of the scheduler needs to know
//! whether its request was wrong (a bad expression or spec, or a name that is
//! already running), whether storage failed, or whether it acted on a stale
//! generation. Store messages carry their context as text so the type stays
//! independent of the storage backend (ADR-CRON-0001).

use std::fmt;

/// Every failure the scheduler and its stores report.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum CronError {
    /// The cron expression does not parse or has no future occurrence.
    BadExpression(String),
    /// A schedule field other than the expression is invalid.
    InvalidSpec(String),
    /// A schedule with this name is already active; stop it first.
    AlreadyActive(String),
    /// The schedule store failed; the message names the operation and row.
    Store(String),
    /// The `(id, generation)` pair no longer names the active schedule.
    Fenced,
}

impl fmt::Display for CronError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadExpression(message) => write!(f, "invalid cron expression: {message}"),
            Self::InvalidSpec(message) => write!(f, "invalid schedule: {message}"),
            Self::AlreadyActive(name) => {
                write!(f, "schedule {name} is already active; stop it first")
            }
            Self::Store(message) => write!(f, "schedule store failed: {message}"),
            Self::Fenced => f.write_str("schedule generation is stale"),
        }
    }
}

impl std::error::Error for CronError {}
