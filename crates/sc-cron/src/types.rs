//! Schedule identity, specification, stored state and run results.
//!
//! A schedule is identified by its [`ScheduleId`], stable for the life of
//! its name, and fenced by its [`Generation`], which a store raises each time
//! an ended schedule is started again. Every run-state write names both, so a
//! run that began under an older generation can never write over the
//! restarted schedule.

use std::fmt;
use std::num::NonZeroU32;
use std::time::Duration;

/// Stable row id of one schedule; kept when an ended name is started again.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ScheduleId(u64);

impl ScheduleId {
    /// Wraps a raw id, as read from a store.
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the raw id.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl fmt::Display for ScheduleId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Fencing generation of one schedule; a stale run cannot record.
///
/// Generations start at [`Generation::FIRST`] and rise by one each time an
/// ended schedule is started again under the same name.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Generation(NonZeroU32);

impl Generation {
    /// The generation of a newly created schedule.
    pub const FIRST: Self = Self(NonZeroU32::MIN);

    /// Wraps a raw generation; `None` for zero, which no schedule carries.
    #[must_use]
    pub const fn new(value: u32) -> Option<Self> {
        match NonZeroU32::new(value) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }

    /// Returns the raw generation.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0.get()
    }

    /// Returns the following generation, or `None` once `u32::MAX` is reached.
    #[must_use]
    pub fn next(self) -> Option<Self> {
        self.0.checked_add(1).map(Self)
    }
}

impl fmt::Display for Generation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// What a caller asks the scheduler to run.
///
/// `expr` uses the `cron` crate's grammar in UTC: six or seven fields,
/// seconds first and year optional, or one of `@yearly`, `@monthly`,
/// `@weekly`, `@daily`, `@hourly`. Five-field expressions are refused.
/// `timeout` bounds the stop check and the run together; it must be at least
/// one millisecond. `payload` is opaque to the scheduler and handed to the
/// [`Job`](crate::Job) on every stop check and run.
#[derive(Clone, Debug, PartialEq)]
pub struct ScheduleSpec {
    /// Unique name; an active schedule's name cannot be started again.
    pub name: String,
    /// Cron expression, parsed before anything is stored.
    pub expr: String,
    /// Budget for one stop check plus run.
    pub timeout: Duration,
    /// Opaque job input.
    pub payload: serde_json::Value,
}

/// A schedule as the scheduler asks a store to create it.
///
/// The scheduler fills `jitter` and `next_run_at` from its injected jitter
/// source and clock, so the store needs neither.
#[derive(Clone, Debug, PartialEq)]
pub struct NewSchedule {
    /// The caller's specification.
    pub spec: ScheduleSpec,
    /// Fixed offset added to every occurrence of the expression.
    pub jitter: Duration,
    /// First run time, RFC 3339 UTC.
    pub next_run_at: String,
}

/// One stored schedule with its run state.
///
/// Timestamps are RFC 3339 UTC text exactly as the scheduler passed them to
/// the store. A schedule is active while `ended_at` is `None`.
#[derive(Clone, Debug, PartialEq)]
pub struct Schedule {
    /// Stable id.
    pub id: ScheduleId,
    /// Current fencing generation.
    pub generation: Generation,
    /// The specification it was (re)started with.
    pub spec: ScheduleSpec,
    /// Fixed jitter added to each occurrence.
    pub jitter: Duration,
    /// When it was (re)started.
    pub created_at: String,
    /// When it next runs; `None` once ended.
    pub next_run_at: Option<String>,
    /// When the last recorded run finished.
    pub last_run_at: Option<String>,
    /// The last recorded run's result.
    pub last_outcome: Option<RunOutcome>,
    /// When it ended; `None` while active.
    pub ended_at: Option<String>,
    /// Why it ended, when it has.
    pub stop_reason: Option<String>,
}

impl Schedule {
    /// Returns whether the schedule is still active.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.ended_at.is_none()
    }
}

/// The result of one run, as recorded by the store.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RunOutcome {
    /// The job completed its work.
    Ok,
    /// The job declined this run; the schedule stays active.
    Refused(String),
    /// The job failed or panicked; the schedule stays active.
    Failed(String),
    /// The stop check and run together exceeded the schedule's timeout.
    TimedOut,
}

impl fmt::Display for RunOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ok => f.write_str("ok"),
            Self::Refused(message) => write!(f, "refused: {message}"),
            Self::Failed(message) => write!(f, "failed: {message}"),
            Self::TimedOut => f.write_str("timed_out"),
        }
    }
}

/// The answer of a job's stop check, asked before every run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StopDecision {
    /// Run this tick.
    Continue,
    /// End the schedule with this reason; this tick does not run.
    Stop(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generation_rejects_zero_and_counts_up() {
        assert_eq!(Generation::new(0), None);
        assert_eq!(Generation::FIRST.get(), 1);
        assert_eq!(Generation::FIRST.next().map(Generation::get), Some(2));
        assert_eq!(Generation::new(u32::MAX).and_then(Generation::next), None);
    }

    #[test]
    fn schedule_id_reads_back_and_displays_the_integer() {
        let id = ScheduleId::new(42);
        assert_eq!(id.get(), 42);
        assert_eq!(id.to_string(), "42");
    }

    #[test]
    fn run_outcome_display_names_each_class() {
        assert_eq!(RunOutcome::Ok.to_string(), "ok");
        assert_eq!(
            RunOutcome::Refused("quota".into()).to_string(),
            "refused: quota"
        );
        assert_eq!(
            RunOutcome::Failed("boom".into()).to_string(),
            "failed: boom"
        );
        assert_eq!(RunOutcome::TimedOut.to_string(), "timed_out");
    }
}
