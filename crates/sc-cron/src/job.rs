//! The work a schedule runs.

use std::time::Instant;

use async_trait::async_trait;

use crate::{RunOutcome, StopDecision};

/// The work behind a schedule, given the schedule's opaque payload.
///
/// On every tick the scheduler first calls [`Job::should_stop`], then, if it
/// answers [`StopDecision::Continue`], [`Job::run`]. `Disable` and `Delete`
/// skip the run (REQ-CRON-0005). Both share one budget,
/// the schedule's timeout. When the budget runs out the scheduler drops the
/// future and records [`RunOutcome::TimedOut`]; the next tick is not delayed.
///
/// # Cancellation is cooperative
///
/// Dropping a future stops it only at its next `.await`. Two consequences:
///
/// - Work handed to `tokio::task::spawn_blocking` cannot be stopped at all.
///   A job that blocks passes the deadline down into the blocking closure
///   and has it give up at the deadline, so a timed-out run never keeps
///   working past it.
/// - A job that blocks a runtime thread without awaiting cannot be timed
///   out, and on a current-thread runtime it also stalls the worker and
///   every other schedule. Run the scheduler on a multi-thread runtime for
///   the hard timeout guarantee, and never block inside `run`.
///
/// The deadline is on [`Clock::monotonic_now`](crate::Clock::monotonic_now)'s
/// timeline, which is `Instant::now` under
/// [`SystemClock`](crate::SystemClock).
///
/// A panic in either method is caught and recorded as
/// [`RunOutcome::Failed`]; the schedule keeps ticking.
#[async_trait]
pub trait Job: Send + Sync {
    /// Runs one tick; must finish its work by `deadline`.
    async fn run(&self, payload: &serde_json::Value, deadline: Instant) -> RunOutcome;

    /// Decides, before every run, whether to go on, disable the schedule or
    /// delete it (REQ-CRON-0005, ADR-CRON-0007).
    async fn should_stop(&self, payload: &serde_json::Value) -> StopDecision;
}
