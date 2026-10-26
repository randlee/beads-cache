//! The schedule-store port.
//!
//! Stores read no clock: every instant is a [`Timestamp`] argument, valid by
//! construction. Every run-state write names `(id, generation)` and changes
//! nothing, returning `Ok(false)`, unless that pair is the active schedule,
//! so a run that outlives its schedule never writes over a restarted one.
//! Calls are synchronous; the scheduler's worker runs them on tokio's
//! blocking pool.
//!
//! A schedule also has an `enabled` flag (REQ-CRON-0014): [`ScheduleStore::set_enabled`]
//! flips it by name and touches no other column, and [`ScheduleStore::disable`]
//! is the same flip made by a stop check, fenced by `(id, generation)`.
//!
//! Two methods change several rows in one transaction (REQ-CRON-0017,
//! ADR-CRON-0009): [`ScheduleStore::set_enabled_many`] flips several rows'
//! flags, and [`ScheduleStore::replace_and_set_enabled`] writes one row with
//! its flag together with other rows' flags, so a caller can keep "at most
//! one of these rows enabled" with no moment where another connection sees
//! otherwise. Both are required, with no default body: a default made of
//! separate calls would not be one transaction.

use crate::{
    CronError, Generation, NewSchedule, RunOutcome, Schedule, ScheduleId, ScheduleName, Timestamp,
};

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
    /// [`CronError::InvalidSpec`] if the payload is over
    /// [`MAX_PAYLOAD_BYTES`](crate::MAX_PAYLOAD_BYTES), and
    /// [`CronError::Store`] if storage fails or the generation is exhausted.
    fn create(&self, schedule: &NewSchedule, created_at: Timestamp) -> Result<Schedule, CronError>;

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
    /// [`CronError::Store`] if storage fails or any row does not decode; a
    /// corrupt row is reported, never skipped.
    fn list_active(&self) -> Result<Vec<Schedule>, CronError>;

    /// Records a finished run's time and outcome.
    ///
    /// Returns `Ok(false)` without writing when `(id, generation)` is not
    /// the active schedule: the run is fenced.
    ///
    /// # Errors
    ///
    /// [`CronError::Store`] if storage fails.
    fn record_run(
        &self,
        id: ScheduleId,
        generation: Generation,
        at: Timestamp,
        outcome: &RunOutcome,
    ) -> Result<bool, CronError>;

    /// Stores the next run time; `Ok(false)` when fenced or when the
    /// schedule is disabled.
    ///
    /// Refusing a disabled schedule is what stops a worker that has not
    /// reloaded, for instance one in another process, from firing it: the
    /// refused write makes the worker reload before it launches (REQ-CRON-0014).
    ///
    /// # Errors
    ///
    /// [`CronError::Store`] if storage fails.
    fn set_next(
        &self,
        id: ScheduleId,
        generation: Generation,
        next_run_at: Timestamp,
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
        at: Timestamp,
        reason: &str,
    ) -> Result<bool, CronError>;

    /// Enables or disables the active schedule named `name`, changing only
    /// its `enabled` flag; `Ok(false)` when no active schedule has that name.
    ///
    /// The generation, expression, payload, jitter and run state are left as
    /// they are, so enabling again resumes the same schedule (REQ-CRON-0014).
    ///
    /// # Errors
    ///
    /// [`CronError::Store`] if storage fails.
    fn set_enabled(&self, name: &ScheduleName, enabled: bool) -> Result<bool, CronError>;

    /// Disables schedule `(id, generation)` because its stop check said so,
    /// recording `reason` as the last outcome ([`RunOutcome::Refused`]) at
    /// `at`; `Ok(false)` when fenced.
    ///
    /// # Errors
    ///
    /// [`CronError::Store`] if storage fails.
    fn disable(
        &self,
        id: ScheduleId,
        generation: Generation,
        at: Timestamp,
        reason: &str,
    ) -> Result<bool, CronError>;

    /// Sets the `enabled` flag of every named active schedule, in one
    /// transaction, touching no other column (REQ-CRON-0017).
    ///
    /// Returns `Ok(false)` and changes nothing when any name has no active
    /// schedule. An empty list changes nothing and returns `Ok(true)`.
    ///
    /// # Errors
    ///
    /// [`CronError::Store`] if storage fails.
    fn set_enabled_many(&self, flags: &[(ScheduleName, bool)]) -> Result<bool, CronError>;

    /// Writes `schedule` with the given `enabled` flag and sets each listed
    /// other schedule's flag, all in one transaction (REQ-CRON-0017).
    ///
    /// The written row: a new name gets a new id at [`Generation::FIRST`]; an
    /// ended name is restarted in place, as [`ScheduleStore::create`] does; an
    /// active name is replaced in place with the same id and generation plus
    /// one (so a run of the old generation is fenced, REQ-CRON-0007), every
    /// field from `schedule` and its run and end state cleared. The row is
    /// written with `enabled` directly and never passes through another
    /// flag. Returns the written schedule, or `Ok(None)` and changes nothing
    /// when a listed name has no active schedule.
    ///
    /// # Errors
    ///
    /// [`CronError::InvalidSpec`] if `flags` names the written schedule or
    /// the payload is over [`MAX_PAYLOAD_BYTES`](crate::MAX_PAYLOAD_BYTES),
    /// and [`CronError::Store`] if storage fails or the generation is
    /// exhausted.
    fn replace_and_set_enabled(
        &self,
        schedule: &NewSchedule,
        enabled: bool,
        created_at: Timestamp,
        flags: &[(ScheduleName, bool)],
    ) -> Result<Option<Schedule>, CronError>;
}
