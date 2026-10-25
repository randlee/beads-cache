//! Cooperative shutdown of a running tick (ADR-SYNC-0002).
//!
//! `sc-cron` times a run out by dropping its future, which never stops a
//! blocking thread. A host that is shutting down sets a [`StopFlag`]: the tick
//! checks it after the drain, before the one `bd list --id` of the uncached
//! phase roots, between phases and before every message, and an [`AtmSender`]
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

use bd_cache_model::HARD_MARGIN;
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

/// What the tick keeps after the time it gives a send: the most a send can run
/// past its deadline, plus the tick's own bookkeeping after the last message
/// (marking entries, recording alerts, building the reply). A send is never
/// given the last 1.5 s before `T`.
///
/// Sized by lead ruling S27 (REQ-SYNC-0011, ADR-SYNC-0007): it covers
/// `atm.rs`'s read grace (250 ms) plus its shared kill bound (1 s), which const
/// assertions there check, with a quarter second to spare. The tick's bounded
/// writes are not sized into it: the marker writes after a send
/// (`mark_announced`, a sync alert; lead ruling S33) and the alert clears
/// without a message (after a passing sync, at the end of an outage; lead
/// ruling S35) each wait for the store's write lock at most the time left
/// before `T` ([`Budget::write_wait`]). A write lock another process holds
/// bounds such a write by `T` plus the bounded write's own run time
/// (statement, commit, fsync, any automatic checkpoint) plus scheduler
/// jitter (lead ruling S39, RSH-032). The lock wait is capped at the time
/// left before `T` and may overshoot it by the busy handler's sleep slack,
/// which counts as scheduler jitter (lead ruling S46, RSH-034). A marker not
/// written in time fails the tick and its message is
/// sent again next tick; a clear not written in time is logged and retried
/// next tick. The tick's other store writes keep the store's own busy timeout.
pub(crate) const SEND_RESERVE: Duration = Duration::from_millis(1500);

// The sync step's gate sits HARD_MARGIN before `T`; the reserve stays inside
// it, and a tick that reaches the gate can still start one message (S27).
const _: () = assert!(SEND_RESERVE.as_millis() < HARD_MARGIN.as_millis());
const _: () = assert!(
    MIN_SEND_BUDGET.as_millis() + SEND_RESERVE.as_millis() <= HARD_MARGIN.as_millis(),
    "a tick at the sync gate must be able to start a message"
);

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

    /// The most a bounded write may wait for the store's write lock: a marker
    /// write after a send (`mark_announced`, a sync alert; lead ruling S33) or
    /// an alert clear without a message (S35). It is the time left before
    /// `T`, all of it: the lock wait is capped at it and may overshoot it by
    /// the busy handler's sleep slack, which counts as scheduler jitter (lead
    /// ruling S46, RSH-034). The write then ends by `T` plus its own run time
    /// (statement, commit, fsync, any automatic checkpoint) plus scheduler
    /// jitter (lead ruling S39, RSH-032). The store waits no longer than its
    /// own busy timeout either. Zero once `T` has passed: the outage-over
    /// clears then stop (lead ruling S45).
    pub fn write_wait(&self) -> Duration {
        self.end.remaining(self.clock)
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
            Some(Duration::from_millis(23_500))
        );
        // 2 s left: 0.5 s after the 1.5 s reserve is exactly the floor.
        assert_eq!(
            budget_left(&clock, &stop, Duration::from_secs(2)),
            Some(Duration::from_millis(500))
        );
        assert_eq!(
            budget_left(&clock, &stop, Duration::from_millis(1999)),
            None
        );
        assert_eq!(budget_left(&clock, &stop, Duration::ZERO), None);
        stop.stop();
        assert_eq!(budget_left(&clock, &stop, Duration::from_secs(25)), None);
    }

    /// A marker write may wait the time left before `T`, all of it and no
    /// more, and nothing once `T` has passed (S33).
    #[test]
    fn a_marker_write_may_wait_the_time_left_before_t() {
        let clock = Arc::new(ManualClock::default());
        let stop = StopFlag::new();
        let budget = Budget {
            clock: &*clock,
            end: Deadline::after(&*clock, Duration::from_secs(3)),
            stop: &stop,
        };
        assert_eq!(budget.write_wait(), Duration::from_secs(3));
        clock.advance(Duration::from_millis(2_250));
        assert_eq!(budget.write_wait(), Duration::from_millis(750));
        clock.advance(Duration::from_secs(1));
        assert_eq!(budget.write_wait(), Duration::ZERO);
    }
}
