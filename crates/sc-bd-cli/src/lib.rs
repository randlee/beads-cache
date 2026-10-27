//! The `bd` CLI implementation of the [`sc_beads`] backend traits.
//!
//! This crate spawns the beads CLI. It is the port of btit's `btit-cli` (process
//! transport) and `btit-bd` (the `bd` backend) at `8d8e82ae9f85`
//! (randlee/beads-task-issue-tracker), plus the cache's calls
//! ([`CacheBackend`](sc_beads::CacheBackend)) with a hard per-call deadline
//! (ADR-BDC-0007, ADR-BDCLI-0001).
//!
//! # Contents
//!
//! - [`BdCli`]: the backend. It implements btit's [`BeadsBackend`](sc_beads::BeadsBackend),
//!   [`CliBackend`](sc_beads::CliBackend) and [`DoltOperations`](sc_beads::DoltOperations)
//!   exactly as btit-bd does, and `CacheBackend` for the cache ([`cache`]).
//! - [`dolt`]: Dolt detection ([`project_uses_dolt_for`]), the [`DoltMode`] switch and
//!   the `DoltOperations` argument builders.
//! - [`runner`]: the [`CliInvoker`] trait (what [`ops`] needs from a CLI) and
//!   [`CliRunner`], one configured binary with its lazily probed, cached version.
//! - [`ops`]: one function per issue operation, generic over `&dyn CliInvoker`.
//! - [`run`], [`probe`], [`path`], [`command`], [`locks`]: the invocation functions, the
//!   `--version` probe, the extended `PATH`, the platform-adjusted `Command` and the
//!   per-project lock ([`ProjectLocks`]).
//! - [`argv`]: the argv of every cache call, as pure functions.
//! - [`bounded`]: the deadline-bounded process run behind the cache calls.
//! - [`shutdown`]: the process-wide shutdown request a host's signal handler sets, which
//!   stops every bounded call (RSH-001).
//! - [`process_group`] (Unix): the group kill and probe of a bounded child, made with
//!   `kill(2)` itself rather than a `kill` program (RSH-002).
//! - `testing` (feature `test-support`): `RecordingInvoker`, a scripted invoker that
//!   spawns nothing and records every argv, and `FakeBd`, an invoker that behaves like
//!   `bd` over `sc_beads::testing::FakeBackend`.
//!
//! # Blocking and deadlines
//!
//! btit's calls block until the child exits (`Command::output()`, no timeout), as they
//! did before the crate split. The cache calls do not: each stops at the
//! [`Deadline`](sc_beads::Deadline) it is handed (NFR-BDC-0004). The crate holds no
//! process-global client state: a [`CliRunner`] owns its binary and probe cache, and
//! the [`ProjectLocks`] it uses is passed in at construction.
//!
//! # Example
//!
//! ```no_run
//! use std::sync::Arc;
//! use std::time::Duration;
//! use sc_beads::{CacheBackend, Clock, Deadline, ProjectRef, SystemClock};
//! use sc_bd_cli::{BdCli, ProjectLocks};
//!
//! let clock: Arc<dyn Clock> = Arc::new(SystemClock);
//! let bd = BdCli::new_with_clock("bd", Arc::new(ProjectLocks::new()), Arc::clone(&clock));
//! let project = ProjectRef::local(Some("/work/project".to_owned()));
//! let deadline = Deadline::after(&*clock, Duration::from_secs(30));
//! let fingerprint = bd.fingerprint(&project, deadline)?;
//! println!("bd {}", fingerprint.bd_version);
//! # Ok::<(), sc_beads::BeadsCacheError>(())
//! ```

#![deny(missing_docs)]

pub mod argv;
pub mod backend;
pub mod bounded;
pub mod cache;
pub mod command;
pub mod dolt;
pub mod locks;
pub mod ops;
pub mod path;
pub mod probe;
#[cfg(unix)]
pub mod process_group;
pub mod run;
pub mod runner;
pub mod shutdown;
#[cfg(feature = "test-support")]
pub mod testing;

#[doc(inline)]
pub use backend::{BdCli, BD_RELEASE_SOURCE};
#[doc(inline)]
pub use bounded::{BoundedError, TimeoutPhase};
#[doc(inline)]
pub use dolt::{project_dolt_mode, project_uses_dolt_for, DoltMode};
#[doc(inline)]
pub use locks::{ProjectLocks, WorkingDir};
#[doc(inline)]
pub use runner::{CliInvoker, CliRunner, ProbeState};
