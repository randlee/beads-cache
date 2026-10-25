//! The bd-cache sync task: tick, stop rule and announcements, scheduled on `sc-cron`.
//!
//! This crate composes the pieces phase A builds: the `bd-cache` core, the
//! SQLite store (`bd-cache-store`), the `bd` CLI backend (`sc-bd-cli`) and the
//! scheduler (`sc-cron`). It holds the BD-CACHE cron module
//! (`bd_cache_cron.py`) in Rust:
//!
//! - [`tick`]: within a 25 s budget ending at `T`, drain with
//!   `DrainBudget { start_gate: T - 5 s, hard: T - 2 s }`, then sync each
//!   active phase with deadline `T`, deferring any not started by `T - 2 s`
//!   (REQ-BDC-0703); phases come from `.atm-bd/phase-*.toml` and the
//!   configured list, and a phase is active while its root bead is not closed
//!   (REQ-BDC-0702);
//! - announcements through the [`EscalationSender`](bd_cache_model::EscalationSender)
//!   port (REQ-BDC-0704..0706): sync failures once per cause, an outage once
//!   per active scope under the cause `backend_unavailable`, escalations once
//!   with the `resolve` command; a message not delivered makes the reply
//!   `refused` and is retried next tick. [`AtmSender`] is the implementation
//!   that runs `atm send`;
//! - [`stop_reason`]: stop when no phase is active and no journal entry is
//!   pending or escalated and unannounced (REQ-BDC-0707);
//! - [`SyncJob`]: the `sc-cron` [`Job`](sc_cron::Job) that runs both under
//!   `spawn_blocking` with the `Cache` in an `Arc<Mutex<_>>` (ADR-CORE-0001),
//!   and [`schedule_spec`], the one schedule per repository (REQ-BDC-0701);
//! - [`compose`]: the real `SqliteStore` + `FlockDrainLock` + `BdCli`
//!   cache of a repository, read from `.beads/bd-cache.toml`, over one
//!   process-wide `bd` lock set.
//!
//! # Codes
//!
//! Every error this crate reports has a stable lower-case code
//! (`RefusalCode::as_str`, `ScheduleError::code`, `ComposeError::code`,
//! `CacheLost::code`), with
//! the `BD_CACHE.*` codes of the core passed through where the cause is a
//! `CacheError`. The code, not the message, is what a program matches on.
//!
//! # One ticker per cache file
//!
//! Either `sc-cron` runs this crate, or an external scheduler (the ATM cron
//! module through the later `bdc tick`, or `bd_cache_cron.py` through the
//! Python bindings) drives `drain`, `sync` and the announcement verbs. Never
//! both on one cache file.
//!
//! # Example
//!
//! ```
//! use bd_cache_model::TickConfig;
//! use bd_cache_sync::schedule_spec;
//!
//! let cfg = TickConfig {
//!     team: Some("atm-dev".into()),
//!     fallback_lead: Some("team-lead".into()),
//!     ..TickConfig::default()
//! };
//! let spec = schedule_spec(&cfg, "atm-dev", None).unwrap();
//! assert_eq!(spec.name.as_str(), "bd-cache:atm-dev");
//! ```

#![deny(missing_docs)]

mod active;
mod announce;
mod atm;
mod compose;
mod control;
mod job;
mod phases;
mod reply;
mod text;
mod tick;

#[doc(inline)]
pub use announce::{AlertCause, OUTAGE_CAUSE};
#[doc(inline)]
pub use atm::{AtmSender, InvalidTimeout, SEND_TIMEOUT, SENDER_IDENTITY, SENDER_TEAM};
#[doc(inline)]
pub use compose::{ComposeError, Composed, RealCache, compose, compose_with};
#[doc(inline)]
pub use control::StopFlag;
#[doc(inline)]
pub use job::{
    CacheLost, JobCacheError, STOP_CHECK_BUDGET, ScheduleError, SyncJob, TICK_EXPR, TickRun,
    schedule_spec,
};
#[doc(inline)]
pub use phases::{
    ConfigRefusal, DEFAULT_PHASES_DIR, InvalidMember, MAX_PHASE_FILE_BYTES, MAX_PHASE_FILES,
    MAX_PHASES_DIR_ENTRIES, MemberName, Phase, PhaseFileProblem, PhaseSet, PhasesError,
    TickSettings, load_phases, rooted_phases_dir,
};
#[doc(inline)]
pub use reply::{Announcement, AnnouncementKind, Refusal, RefusalCode, TickOutcome, TickReply};
#[doc(inline)]
pub use tick::{TICK_BUDGET, stop_reason, stop_reason_with, tick, tick_with};
