//! A SQLite-backed cron scheduler with generation fencing.
//!
//! `sc-cron` runs a caller's [`Job`] on cron schedules kept in a
//! [`ScheduleStore`]. It was extracted from atm-core's `atm-cron`
//! (ADR-BDC-0006) and carries no ATM or bd-cache types: a schedule's input
//! is an opaque JSON payload and every failure is a [`CronError`].
//!
//! - [`Scheduler::start`] parses the expression before anything is written,
//!   draws a per-schedule jitter and stores the first run time.
//! - [`Scheduler::spawn_worker`] starts one tokio task that runs every
//!   active schedule: it asks [`Job::should_stop`] before every run, bounds
//!   stop check and run by the schedule's timeout (recording
//!   [`RunOutcome::TimedOut`] without delaying the next tick), skips a tick
//!   whose previous run is still in flight and handles missed ticks per
//!   [`MissedTickPolicy`].
//! - Every run-state write names `(id, generation)`; a run from an older
//!   generation is fenced and records nothing.
//! - Stores read no clock: every timestamp is an argument.
//!   [`SqliteScheduleStore`] is the rusqlite (bundled) implementation.
//! - Time and randomness are injected through [`Clock`] and
//!   [`JitterSource`]. Feature `test-utils` adds the deterministic
//!   `ManualClock` and `SeqJitter`.
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
//! scheduler.start(ScheduleSpec {
//!     name: "sync".into(),
//!     expr: "*/30 * * * * *".into(),
//!     timeout: Duration::from_secs(25),
//!     payload: serde_json::json!({}),
//! })?;
//! let worker = scheduler.spawn_worker(Arc::new(Sync));
//! // ... later
//! worker.shutdown(tokio::time::Instant::now() + Duration::from_secs(30)).await;
//! # Ok(())
//! # }
//! ```

mod clock;
mod error;
mod expr;
mod jitter;
mod job;
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
pub use error::CronError;
#[cfg(feature = "test-utils")]
#[doc(inline)]
pub use jitter::SeqJitter;
#[doc(inline)]
pub use jitter::{JitterSource, SystemJitter};
#[doc(inline)]
pub use job::Job;
#[doc(inline)]
pub use scheduler::{MAX_TIMEOUT, MissedTickPolicy, Scheduler, WorkerHandle};
#[doc(inline)]
pub use sqlite::SqliteScheduleStore;
#[doc(inline)]
pub use store::ScheduleStore;
#[doc(inline)]
pub use types::{
    Generation, NewSchedule, RunOutcome, Schedule, ScheduleId, ScheduleSpec, StopDecision,
};
