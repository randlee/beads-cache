//! Application state over one existing cache connection and its schedule table.
//!
//! Readiness is read-only and runs on one owned thread. Its rendezvous channel
//! accepts no queued job; the permit lives in the job even after HTTP disconnect.
//! Scheduler jobs retain the existing production scheduler and supervision path.

mod readiness;
#[cfg(test)]
mod shutdown_tests;
#[cfg(test)]
mod startup_tests;
#[cfg(test)]
mod test_support;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use bd_cache_api::{ApiError, Command, decode};
use bd_cache_model::CacheError;
use bd_cache_sync::{Composed, CronStopped, StopFlag, SystemSchedules};

#[doc(inline)]
pub use readiness::{ReadyError, ReadyState};

/// Retained application ownership, consumed by runtime cleanup before unlock.
#[derive(Debug)]
pub struct Application {
    ready: ReadyState,
    reader: readiness::Worker,
    stop: StopFlag,
    worker: tokio::task::JoinHandle<CronStopped>,
    // Unit tests observe this exact application worker, never a second scheduler.
    #[cfg(test)]
    worker_wait: Option<std::sync::mpsc::Receiver<()>>,
}

impl Application {
    /// Starts readiness and the existing scheduler over `composed` exactly once.
    ///
    /// # Errors
    /// Returns the original schedule error or a typed thread-start failure.
    pub fn start(composed: Composed, shutdown_timeout: Duration) -> Result<Self, StartError> {
        Self::start_inner(composed, shutdown_timeout, |_| {})
    }

    /// Observes actual admitted evaluation for owned integration fixtures only.
    /// No runtime config or normal-consumer bypass enables this seam.
    ///
    /// # Errors
    /// Preserves the normal application startup errors.
    #[cfg(feature = "testing")]
    #[doc(hidden)]
    pub fn start_observed(
        composed: Composed,
        shutdown_timeout: Duration,
        before_read: impl Fn(std::time::Instant) + Send + 'static,
    ) -> Result<Self, StartError> {
        Self::start_inner(composed, shutdown_timeout, before_read)
    }

    fn start_inner(
        composed: Composed,
        shutdown_timeout: Duration,
        before_read: impl Fn(std::time::Instant) + Send + 'static,
    ) -> Result<Self, StartError> {
        let health_reads = composed.health_reads.clone();
        let facts = composed.facts();
        let files = composed.files();
        let table = composed.schedules.clone();
        let store = composed.schedule_store().map_err(StartError::Cache)?;
        let cache = Arc::new(Mutex::new(composed.cache));
        let schedules = SystemSchedules::system(store, Arc::clone(&cache), files);
        #[cfg(test)]
        let worker_wait = schedules
            .worker_store_control_for_tests()
            .map(|control| control.wait_attempt_for_tests());
        let request =
            decode(Command::CacheHealth, &serde_json::json!({})).map_err(StartError::Request)?;
        let (ready, reader) = readiness::start(move |deadline| {
            let mut cache = cache.try_lock().map_err(|_error| ReadyError::Busy)?;
            health_reads
                .within(deadline, || {
                    before_read(deadline);
                    bd_cache_sync::execute_api(&table, &mut cache, &request, &facts)
                })
                .map_err(ReadyError::Cache)
        })
        .map_err(StartError::Thread)?;
        let stop = StopFlag::new();
        let worker_stop = stop.clone();
        let worker = tokio::spawn(async move {
            schedules
                .cron_run(worker_stop, shutdown_timeout, || {
                    // Same production process cancellation used by the local CLI.
                    sc_beads_cancel();
                })
                .await
        });
        Ok(Self {
            ready,
            reader,
            stop,
            worker,
            #[cfg(test)]
            worker_wait,
        })
    }

    /// Cloneable HTTP state; cloning opens neither a cache nor a worker.
    #[must_use]
    pub fn state(&self) -> ReadyState {
        self.ready.clone()
    }

    /// Closes admission, stops the scheduler, and joins actual readiness work.
    ///
    /// Called only after the pinned runtime HTTP drain (maximum 31 seconds).
    /// Scheduler cleanup is bounded by twice its configured timeout. The one
    /// admitted read has at most its remaining one-second store budget.
    ///
    /// # Errors
    /// Reports a failed task/thread join or a scheduler that could not go idle.
    pub async fn shutdown(self) -> Result<(), ShutdownError> {
        self.ready.close();
        self.stop.stop();
        let reader = self.reader;
        let join_reader = tokio::task::spawn_blocking(move || reader.join());
        let (worker, reader) = tokio::join!(self.worker, join_reader);
        let worker = worker.map_err(ShutdownError::Worker)?;
        let reader = reader.map_err(ShutdownError::Worker)?;
        reader.map_err(|_error| ShutdownError::Readiness)?;
        if worker.idle && !worker.worker_ended {
            Ok(())
        } else {
            Err(ShutdownError::NotIdle)
        }
    }
}

// Implemented below via the same sc-bd-cli cancellation hook as bdc.
fn sc_beads_cancel() {
    sc_bd_cli::shutdown::request(15);
}

/// A startup failure, retaining its original typed source.
#[derive(Debug)]
pub enum StartError {
    /// The cache schedule view could not be read.
    Cache(CacheError),
    /// The static health request could not be decoded.
    Request(ApiError),
    /// The dedicated readiness thread could not start.
    Thread(std::io::Error),
}
impl std::fmt::Display for StartError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cache(e) => e.fmt(f),
            Self::Request(e) => e.fmt(f),
            Self::Thread(e) => e.fmt(f),
        }
    }
}
impl std::error::Error for StartError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::Cache(e) => e,
            Self::Request(e) => e,
            Self::Thread(e) => e,
        })
    }
}

/// Cleanup failed while instance ownership was retained.
#[derive(Debug)]
pub enum ShutdownError {
    /// A supervised Tokio task panicked.
    Worker(tokio::task::JoinError),
    /// The owned readiness thread panicked.
    Readiness,
    /// The scheduler left blocking work or ended unexpectedly.
    NotIdle,
}
impl std::fmt::Display for ShutdownError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Worker(e) => write!(f, "worker join failed: {e}"),
            Self::Readiness => f.write_str("readiness worker panicked"),
            Self::NotIdle => {
                f.write_str("scheduler stopped unexpectedly or did not leave the cache idle")
            }
        }
    }
}
impl std::error::Error for ShutdownError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Worker(error) => Some(error),
            Self::Readiness | Self::NotIdle => None,
        }
    }
}

#[cfg(test)]
mod tests {
    #![expect(clippy::expect_used, reason = "test assertions")]
    use super::*;
    use bd_cache_api::Data;

    #[test]
    fn health_is_cached_and_cleanup_releases_the_single_connection() {
        crate::test_support::bounded_fixture(
            "tests::health_is_cached_and_cleanup_releases_the_single_connection",
            || {
                tokio::runtime::Runtime::new()
                    .expect("runtime")
                    .block_on(application_scenario());
            },
        );
    }

    #[tokio::test]
    async fn shutdown_error_retains_actual_join_error_source() {
        use std::error::Error;
        let task = tokio::spawn(async { std::future::pending::<()>().await });
        task.abort();
        let error = ShutdownError::Worker(task.await.expect_err("canceled task"));
        let source = error.source().expect("retained source");
        assert!(
            source
                .downcast_ref::<tokio::task::JoinError>()
                .expect("typed join cause")
                .is_cancelled()
        );
        assert!(ShutdownError::Readiness.source().is_none());
        assert!(ShutdownError::NotIdle.source().is_none());
    }

    async fn application_scenario() {
        let repo = tempfile::tempdir().expect("repo");
        std::fs::create_dir(repo.path().join(".beads")).expect("beads");
        std::fs::write(
            repo.path().join(".beads/metadata.json"),
            r#"{"dolt_database":"test","project_id":"p-1"}"#,
        )
        .expect("metadata");
        let composed = bd_cache_sync::compose(
            repo.path(),
            "/never-run-backend",
            Arc::new(sc_beads::SystemClock),
        )
        .expect("compose without backend");
        let app = Application::start(composed, Duration::from_millis(150)).expect("application");
        let state = app.state();
        let data = tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                match state.ready().await {
                    Err(ReadyError::Busy) => tokio::task::yield_now().await,
                    result => break result.expect("cached health"),
                }
            }
        })
        .await
        .expect("bounded readiness startup and evaluation");
        assert!(matches!(&data, Data::CacheHealth(_)));
        let Data::CacheHealth(health) = data else {
            return;
        };
        assert!(
            health.outage.is_none(),
            "readiness must never invoke the missing executable"
        );
        assert!(health.probe.is_none());
        assert!(
            bd_cache_sync::compose(
                repo.path(),
                "/never-run-backend",
                Arc::new(sc_beads::SystemClock)
            )
            .is_err(),
            "the existing connection retains the same-process file claim"
        );
        tokio::time::timeout(Duration::from_secs(2), app.shutdown())
            .await
            .expect("bounded application cleanup")
            .expect("joined");
        assert!(matches!(state.ready().await, Err(ReadyError::Closing)));
        let reopened = bd_cache_sync::compose(
            repo.path(),
            "/never-run-backend",
            Arc::new(sc_beads::SystemClock),
        )
        .expect("file claim released after real thread and scheduler join");
        assert_eq!(reopened.facts().store_schema_version, 3);
    }
}
