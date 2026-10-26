//! The tokio runtime of the phase-B verbs (ADR-SYNC-0008, ADR-BDC-0015).

use bd_cache_api::ApiError;

/// The worker threads of a command's runtime: one for sc-cron's worker task
/// (`cron run`) and one for the runs' async halves (a hook's or a script's
/// supervisor, the stop and kill waits). The runs' blocking parts (SQLite,
/// `bd`) run on tokio's blocking pool, not on these, so two suffice.
const WORKER_THREADS: usize = 2;

/// A multi-thread runtime for one command; dropped (or shut down within a
/// bound) when the command ends.
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
