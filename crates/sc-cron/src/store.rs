//! The schedule-store port.
//!
//! Stores read no clock: every timestamp is an argument, in the RFC 3339 UTC
//! form the scheduler writes. Every run-state write names `(id, generation)`
//! and changes nothing, returning `Ok(false)`, unless that pair is the active
//! schedule, so a run that outlives its schedule never writes over a
//! restarted one. Calls are synchronous; the scheduler's worker runs them on
//! tokio's blocking pool.

use crate::{CronError, Generation, NewSchedule, RunOutcome, Schedule, ScheduleId};

/// Durable storage for schedules and their run state.
pub trait ScheduleStore: Send + Sync {
    /// Creates a schedule, or restarts an ended one with the same name.
    ///
    /// A new name gets a new id at [`Generation::FIRST`]. An ended name is
    /// restarted in place: same id, generation plus one, every field from
    /// `schedule`, run state and end state cleared.
    ///
    /// # Errors
    ///
    /// [`CronError::AlreadyActive`] if the name is active,
    /// [`CronError::InvalidSpec`] if a field cannot be stored, and
    /// [`CronError::Store`] if storage fails or the generation is exhausted.
    fn create(&self, schedule: &NewSchedule, created_at: &str) -> Result<Schedule, CronError>;

    /// Returns one schedule, active or ended.
    ///
    /// # Errors
    ///
    /// [`CronError::Store`] if storage fails or the row does not decode.
    fn get(&self, id: ScheduleId) -> Result<Option<Schedule>, CronError>;

    /// Returns every active schedule, oldest `created_at` first.
    ///
    /// # Errors
    ///
    /// [`CronError::Store`] if storage fails or a row does not decode.
    fn list_active(&self) -> Result<Vec<Schedule>, CronError>;

    /// Records a finished run's time and outcome.
    ///
    /// Returns `Ok(false)` without writing when `(id, generation)` is not the active
    /// schedule: the run is fenced.
    ///
    /// # Errors
    ///
    /// [`CronError::Store`] if storage fails.
    fn record_run(
        &self,
        id: ScheduleId,
        generation: Generation,
        at: &str,
        outcome: &RunOutcome,
    ) -> Result<bool, CronError>;

    /// Stores the next run time; `Ok(false)` when fenced.
    ///
    /// # Errors
    ///
    /// [`CronError::Store`] if storage fails.
    fn set_next(
        &self,
        id: ScheduleId,
        generation: Generation,
        next_run_at: &str,
    ) -> Result<bool, CronError>;

    /// Ends the schedule at `at` with `reason`; `Ok(false)` when fenced.
    ///
    /// # Errors
    ///
    /// [`CronError::Store`] if storage fails.
    fn stop(
        &self,
        id: ScheduleId,
        generation: Generation,
        at: &str,
        reason: &str,
    ) -> Result<bool, CronError>;
}
