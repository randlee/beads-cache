//! A SQLite-backed cron scheduler with generation fencing.
//!
//! `sc-cron` runs a caller's [`Job`] on cron schedules kept in a
//! [`ScheduleStore`]. It was extracted from an earlier in-house scheduler
//! and carries no consumer-specific type (REQ-CRON-0011): a
//! schedule's input is an opaque JSON payload and every
//! failure is a [`CronError`] with a stable [`code`](CronError::code).
//!
//! - Input is validated once, by type: [`ScheduleName`], [`CronExpr`],
//!   [`RunTimeout`], [`Timestamp`], within the [limits](MAX_PAYLOAD_BYTES).
//! - [`Scheduler::start`] checks the expression has a future occurrence
//!   before anything is written, draws a per-schedule jitter and stores the
//!   first run time.
//! - [`Scheduler::spawn_worker`] starts one tokio task that runs every
//!   active schedule: it asks [`Job::should_stop`] before every run, bounds
//!   stop check and run by the schedule's timeout on the monotonic clock
//!   (recording [`RunOutcome::TimedOut`] without delaying the next tick),
//!   skips a tick whose previous run is still in flight and handles missed
//!   ticks per [`MissedTickPolicy`].
//! - A schedule can be switched off and on again with
//!   [`Scheduler::set_enabled`] without rewriting its row (REQ-CRON-0014),
//!   and its stop check answers [`StopDecision::Continue`], `Disable` or
//!   `Delete` (REQ-CRON-0005).
//! - The worker runs a [`HookTarget`]: a Rust [`Job`], or a Python module run
//!   as `<interpreter> -I -m <module>` with every hook killed at its timeout
//!   (REQ-CRON-0013). [`Scheduler::spawn_worker_with`] takes a
//!   [`TargetResolver`] that chooses the target per row, once per launch, so
//!   one table can mix Rust and Python rows (REQ-CRON-0016);
//!   [`Scheduler::run_due_with`] runs what is due once, for an outside
//!   scheduler. Five-field crontab expressions are accepted (REQ-CRON-0015).
//! - [`process`] is the child-process supervisor the Python target runs on,
//!   exported for a caller's own child (REQ-CRON-0018): process-group or
//!   job-object kill at expiry and on drop, capped output, an environment
//!   policy and a private working directory.
//! - Every run-state write names `(id, generation)`; a run from an older
//!   generation is fenced and records nothing.
//! - Stores read no clock: every instant is a [`Timestamp`] argument.
//!   [`SqliteScheduleStore`] is the rusqlite (bundled) implementation, on
//!   its own database file.
//! - Time and randomness are injected through [`Clock`] and
//!   [`JitterSource`]. Feature `test-utils` adds the deterministic
//!   `ManualClock` and `SeqJitter`.
//!
//! # Runtime requirements
//!
//! - [`Scheduler::start`] and [`Scheduler::stop`] call the store
//!   synchronously. From async code use [`Scheduler::start_async`] and
//!   [`Scheduler::stop_async`], which run on tokio's blocking pool.
//! - Run timeouts are cooperative (see [`Job`]); use a multi-thread tokio
//!   runtime so a misbehaving job cannot stall the worker.
//! - Stop the worker by awaiting [`WorkerHandle::shutdown`]; dropping the
//!   handle drains detached and can lose in-flight outcomes at process exit.
//!
//! # Examples
//!
//! ```no_run
//! use std::sync::Arc;
//! use std::time::{Duration, Instant};
//!
//! use sc_cron::{
//!     Job, RunOutcome, ScheduleSpec, Scheduler, SqliteScheduleStore, StopDecision, SystemClock,
//!     SystemJitter,
//! };
//!
//! struct Sync;
//!
//! #[async_trait::async_trait]
//! impl Job for Sync {
//!     async fn run(&self, payload: &serde_json::Value, deadline: Instant) -> RunOutcome {
//!         let _ = (payload, deadline); // do the work, finishing by `deadline`
//!         RunOutcome::Ok
//!     }
//!
//!     async fn should_stop(&self, _payload: &serde_json::Value) -> StopDecision {
//!         StopDecision::Continue
//!     }
//! }
//!
//! # async fn example() -> Result<(), sc_cron::CronError> {
//! let scheduler = Scheduler::new(
//!     Arc::new(SqliteScheduleStore::open("cron.db")?),
//!     Arc::new(SystemClock),
//!     Arc::new(SystemJitter),
//! );
//! let spec = ScheduleSpec::new("sync", "*/30 * * * * *", Duration::from_secs(25), serde_json::json!({}))?;
//! scheduler.start_async(spec).await?;
//! let worker = scheduler.spawn_worker(Arc::new(Sync));
//! // ... later
//! worker.shutdown(tokio::time::Instant::now() + Duration::from_secs(30)).await;
//! # Ok(())
//! # }
//! ```

#![forbid(unsafe_code)]
#![warn(unreachable_pub)]

mod clock;
mod error;
mod expr;
mod hook;
mod jitter;
mod job;
mod limits;
pub mod process;
mod scheduler;
mod sqlite;
mod store;
mod timestamp;
mod types;
mod worker;

#[cfg(feature = "test-utils")]
#[doc(inline)]
pub use clock::ManualClock;
#[doc(inline)]
pub use clock::{Clock, SystemClock};
#[doc(inline)]
pub use error::{CronError, StoreError, StoreErrorKind};
#[doc(inline)]
pub use expr::CronExpr;
#[doc(inline)]
pub use hook::{
    HOOK_PROTOCOL, HookCall, HookEvent, HookOutput, HookStatus, HookTarget, MAX_HOOK_OUTPUT_BYTES,
    PythonHook, TargetResolver, kill_budget,
};
#[cfg(feature = "test-utils")]
#[doc(inline)]
pub use jitter::SeqJitter;
#[doc(inline)]
pub use jitter::{JitterSource, SystemJitter};
#[doc(inline)]
pub use job::Job;
#[doc(inline)]
pub use limits::{MAX_EXPR_LEN, MAX_NAME_LEN, MAX_PAYLOAD_BYTES, MAX_TIMEOUT};
#[doc(inline)]
pub use scheduler::{DueRun, MissedTickPolicy, Scheduler, WorkerHandle};
#[doc(inline)]
pub use sqlite::{SCHEMA_VERSION, SqliteScheduleStore};
#[doc(inline)]
pub use store::ScheduleStore;
#[doc(inline)]
pub use timestamp::Timestamp;
#[doc(inline)]
pub use types::{
    Generation, NewSchedule, RunOutcome, RunTimeout, Schedule, ScheduleId, ScheduleName,
    ScheduleSpec, StopDecision,
};
#[doc(inline)]
pub use worker::DEFAULT_RELOAD_POLL;
