//! The tokio runtime of the phase-B verbs (ADR-SYNC-0008, ADR-BDC-0015).

use std::time::Duration;

use bd_cache_api::ApiError;

/// The worker threads of a command's runtime: one for sc-cron's worker task
/// (`cron run`) and one for the runs' async halves (a hook's or a script's
/// supervisor, the stop and kill waits). The runs' blocking parts (SQLite,
/// `bd`) run on tokio's blocking pool, not on these, so two suffice.
const WORKER_THREADS: usize = 2;

/// How long a one-shot command waits, once its result is known, for the
/// blocking tasks its runtime still runs before the process exits (`cron run
/// --once` and every front-end verb but `cron run`, which derives its own
/// bound from `--shutdown-timeout-ms`).
///
/// A plain drop of the runtime waits for them without limit, and after a
/// signal a task can still sit in the mode-lock spin (up to 30 s) or a SQLite
/// busy wait. Five seconds is long enough for a normal lease release or a
/// killed `bd` call to finish on the pool and short enough that the exit
/// stays near what the signal asked for; a task cut off by it is abandoned
/// with the process, and a lease whose release it was left to expiry.
pub(crate) const ONE_SHOT_TEARDOWN: Duration = Duration::from_secs(5);

/// A multi-thread runtime for one command; shut down within a bound when the
/// command ends ([`ONE_SHOT_TEARDOWN`], or `cron run`'s own).
///
/// # Errors
///
/// `INTERNAL` when the runtime cannot start.
pub(crate) fn runtime() -> Result<tokio::runtime::Runtime, ApiError> {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(WORKER_THREADS)
        .enable_all()
        .build()
        .map_err(|e| ApiError::internal(format!("cannot start the tokio runtime: {e}")))
}
