//! Cooperative shutdown of a running sync (ADR-SYNC-0002).
//!
//! `sc-cron` times a run out by dropping its future, which never stops a
//! blocking thread. A host that is shutting down sets a [`StopFlag`]: the run
//! checks it before the drain, after it and between scopes. Shutdown order:
//!
//! 1. [`StopFlag::stop`] (and the scheduler's `shutdown`, which stops starting runs);
//! 2. wait for the worker (`WorkerHandle::shutdown`), which waits for the run in flight;
//! 3. drop the job.
//!
//! The drain keeps its own budget and does not look at the flag: a stop that
//! arrives while it runs is seen when it returns, so the worst-case delay is the
//! drain's hard deadline, `T - 2 s` (the start gate stops new entries at `T - 5 s`,
//! the entry in flight is capped at `T - 2 s`), that is the start gate plus one
//! `bd` call (ADR-SYNC-0002).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// A shared "stop what you are doing" signal.
///
/// # Examples
///
/// ```
/// use bd_cache_sync::StopFlag;
/// let flag = StopFlag::new();
/// let seen_by_the_tick = flag.clone();
/// assert!(!seen_by_the_tick.is_stopped());
/// flag.stop();
/// assert!(seen_by_the_tick.is_stopped());
/// ```
#[derive(Debug, Clone, Default)]
pub struct StopFlag(Arc<AtomicBool>);

impl StopFlag {
    /// A flag that is not set.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the flag; every clone sees it.
    pub fn stop(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    /// True once [`stop`](Self::stop) was called on any clone.
    #[must_use]
    pub fn is_stopped(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}
