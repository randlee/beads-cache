//! Cooperative shutdown of a running tick (ADR-SYNC-0002).
//!
//! `sc-cron` times a run out by dropping its future, which never stops a
//! blocking thread. A host that is shutting down sets a [`StopFlag`]: the tick
//! checks it between phases and before every message, and an [`AtmSender`]
//! built with the same flag kills its child process. Shutdown order:
//!
//! The drain keeps its own budget and does not look at the flag: a stop that
//! arrives while it runs is seen when it returns, so the worst-case delay is the
//! drain's hard deadline, `T - 2 s` (the start gate stops new entries at `T - 5 s`,
//! the entry in flight is capped at `T - 2 s`), that is the start gate plus one
//! `bd` call (ADR-SYNC-0002; `a_stop_that_arrives_mid_drain_is_seen_within_the_drains_hard_deadline`).
//!
//! 1. [`StopFlag::stop`] (and the scheduler's `shutdown`, which stops starting runs);
//! 2. wait for the worker (`WorkerHandle::shutdown`), which waits for the run in flight;
//! 3. drop the job.
//!
//! [`AtmSender`]: crate::AtmSender

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use sc_beads::{Clock, Deadline};

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

/// The least time worth starting a message with: below it the send is deferred
/// to the next tick instead of started and cut.
///
/// A local `atm send` takes tens of milliseconds; half a second leaves room for
/// a loaded host without letting a tick start a send it cannot finish.
pub(crate) const MIN_SEND_BUDGET: Duration = Duration::from_millis(500);

/// What the tick keeps for its own bookkeeping after the last message (marking
/// entries, recording alerts, building the reply): a send is never given the
/// last half second before `T`.
pub(crate) const SEND_RESERVE: Duration = Duration::from_millis(500);

/// What a tick may still spend: its end `T`, its clock and the stop flag.
pub(crate) struct Budget<'a> {
    pub clock: &'a dyn Clock,
    pub end: Deadline,
    pub stop: &'a StopFlag,
}

impl Budget<'_> {
    /// The time left for one message, or `None` when the tick was asked to
    /// stop or has less than [`MIN_SEND_BUDGET`] left.
    pub fn send_budget(&self) -> Option<Duration> {
        if self.stop.is_stopped() {
            return None;
        }
        let left = self.end.remaining(self.clock).saturating_sub(SEND_RESERVE);
        (left >= MIN_SEND_BUDGET).then_some(left)
    }
}

#[cfg(test)]
mod tests {
    use sc_beads::testing::ManualClock;

    use super::*;

    fn budget_left(clock: &Arc<ManualClock>, stop: &StopFlag, left: Duration) -> Option<Duration> {
        Budget {
            clock: &**clock,
            end: Deadline::after(&**clock, left),
            stop,
        }
        .send_budget()
    }

    #[test]
    fn a_send_is_given_the_time_left_minus_the_reserve_and_nothing_below_the_floor() {
        let clock = Arc::new(ManualClock::default());
        let stop = StopFlag::new();
        assert_eq!(
            budget_left(&clock, &stop, Duration::from_secs(25)),
            Some(Duration::from_millis(24_500))
        );
        // 1 s left: 0.5 s after the reserve is exactly the floor.
        assert_eq!(
            budget_left(&clock, &stop, Duration::from_secs(1)),
            Some(Duration::from_millis(500))
        );
        assert_eq!(budget_left(&clock, &stop, Duration::from_millis(999)), None);
        assert_eq!(budget_left(&clock, &stop, Duration::ZERO), None);
        stop.stop();
        assert_eq!(budget_left(&clock, &stop, Duration::from_secs(25)), None);
    }
}
