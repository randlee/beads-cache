//! Cooperative shutdown ([`StopFlag`]) and the two deadlines of a built-in run ([`Budget`]).
//!
//! `sc-cron` times a run out by dropping its future, which never stops a
//! blocking thread. A host that is shutting down (a signal to `cron run`,
//! `tick` or `sync use-python`, REQ-BDC-0730) sets a [`StopFlag`]: the built-in
//! run checks it after the drain and between scopes, and the post-sync hook's
//! and the Python row's children are killed when it is set, so their process
//! trees go with the run (ADR-SYNC-0008).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use bd_cache_model::TickBudget;
use sc_beads::{Clock, Deadline};
use tokio::sync::Notify;

/// A shared "stop what you are doing" signal.
///
/// # Examples
///
/// ```
/// use bd_cache_sync::StopFlag;
/// let flag = StopFlag::new();
/// let seen_by_the_run = flag.clone();
/// assert!(!seen_by_the_run.is_stopped());
/// flag.stop();
/// assert!(seen_by_the_run.is_stopped());
/// ```
#[derive(Debug, Clone, Default)]
pub struct StopFlag(Arc<Inner>);

#[derive(Debug, Default)]
struct Inner {
    stopped: AtomicBool,
    notify: Notify,
}

impl StopFlag {
    /// A flag that is not set.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the flag; every clone sees it, and every [`wait`](Self::wait) returns.
    pub fn stop(&self) {
        self.0.stopped.store(true, Ordering::SeqCst);
        self.0.notify.notify_waiters();
    }

    /// True once [`stop`](Self::stop) was called on any clone.
    #[must_use]
    pub fn is_stopped(&self) -> bool {
        self.0.stopped.load(Ordering::SeqCst)
    }

    /// Completes once the flag is set (at once if it already is).
    ///
    /// Causal: the waiter registers before it reads the flag, so a `stop`
    /// between the read and the wait is never missed.
    pub async fn wait(&self) {
        loop {
            let notified = self.0.notify.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            if self.is_stopped() {
                return;
            }
            notified.await;
        }
    }
}

/// The two deadlines of one built-in run (REQ-BDC-0703 as amended, ADR-SYNC-0008).
///
/// Made from the run's start and its [`TickBudget`]: the sync deadline
/// (start + `budget_ms` − `hook_reserve_ms`), which the drain and the scope
/// syncs get, and the hook deadline (start + `budget_ms`), so the post-sync
/// hook always keeps `hook_reserve_ms`.
///
/// # Examples
///
/// ```
/// use std::sync::Arc;
/// use std::time::Duration;
/// use bd_cache_model::TickBudget;
/// use bd_cache_sync::Budget;
/// use sc_beads::testing::ManualClock;
///
/// let clock = Arc::new(ManualClock::default());
/// let budget = Budget::new(&*clock, &TickBudget::new(Some(10_000), Some(4_000), true)?);
/// assert_eq!(budget.sync_deadline().remaining(&*clock), Duration::from_secs(6));
/// assert_eq!(budget.hook_deadline().remaining(&*clock), Duration::from_secs(10));
/// # Ok::<(), bd_cache_model::BudgetError>(())
/// ```
#[derive(Debug, Clone, Copy)]
pub struct Budget {
    sync: Deadline,
    hook: Deadline,
}

impl Budget {
    /// The deadlines of a run starting now on `clock`.
    #[must_use]
    pub fn new(clock: &dyn Clock, budget: &TickBudget) -> Self {
        Self {
            sync: Deadline::after(clock, budget.work()),
            hook: Deadline::after(clock, budget.budget()),
        }
    }

    /// The drain's and the scope syncs' deadline.
    #[must_use]
    pub fn sync_deadline(&self) -> Deadline {
        self.sync
    }

    /// The post-sync hook's deadline, the end of the run's budget.
    #[must_use]
    pub fn hook_deadline(&self) -> Deadline {
        self.hook
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn wait_returns_once_the_flag_is_set_and_at_once_after() {
        let flag = StopFlag::new();
        let waiter = tokio::spawn({
            let flag = flag.clone();
            async move { flag.wait().await }
        });
        tokio::task::yield_now().await;
        assert!(!waiter.is_finished());
        flag.stop();
        assert!(waiter.await.is_ok());
        flag.wait().await;
    }
}
