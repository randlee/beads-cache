//! Wall-clock and monotonic time for the scheduler.
//!
//! The scheduler reads time only through [`Clock`], so tests drive it with
//! [`ManualClock`] (feature `test-utils`) and never wait on real time.
//! [`SystemClock`] sleeps in slices of at most 30 s and re-reads the wall
//! clock after each, so a wall-clock jump or a host suspend is noticed
//! within one slice.

use std::time::{Duration, Instant};

use async_trait::async_trait;
use chrono::{DateTime, Utc};

/// The scheduler's source of time.
///
/// `now` is the wall clock that cron occurrences are computed against.
/// `monotonic_now` is the timeline a [`Job`](crate::Job)'s deadline is on;
/// a job compares its deadline with this, not with `Instant::now`, so it
/// behaves the same under [`ManualClock`]. `sleep_until` returns once `now`
/// reaches `target` and is the only place the scheduler waits on time.
#[async_trait]
pub trait Clock: Send + Sync {
    /// Returns the current wall-clock time in UTC.
    fn now(&self) -> DateTime<Utc>;

    /// Returns the current monotonic instant; deadlines are on this timeline.
    fn monotonic_now(&self) -> Instant {
        Instant::now()
    }

    /// Waits until [`Clock::now`] is at or after `target`.
    async fn sleep_until(&self, target: DateTime<Utc>);
}

/// The real clock: `chrono::Utc::now`, `Instant::now` and tokio timers.
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemClock;

/// Longest single sleep, so wall-clock changes are noticed promptly.
pub(crate) const MAX_SLEEP_SLICE: Duration = Duration::from_secs(30);

pub(crate) fn system_sleep_step(now: DateTime<Utc>, target: DateTime<Utc>) -> Duration {
    (target - now)
        .to_std()
        .unwrap_or_default()
        .min(MAX_SLEEP_SLICE)
}

#[async_trait]
impl Clock for SystemClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }

    async fn sleep_until(&self, target: DateTime<Utc>) {
        loop {
            let step = system_sleep_step(self.now(), target);
            if step.is_zero() {
                return;
            }
            tokio::time::sleep(step).await;
        }
    }
}

#[cfg(feature = "test-utils")]
pub use manual::ManualClock;

#[cfg(feature = "test-utils")]
mod manual {
    use std::sync::{Mutex, PoisonError};
    use std::time::{Duration, Instant};

    use async_trait::async_trait;
    use chrono::{DateTime, Utc};
    use tokio::sync::watch;

    use super::Clock;

    /// Longest a test helper waits for the worker before panicking.
    const SETTLE_TIMEOUT: Duration = Duration::from_secs(10);

    /// A clock that moves only when a test moves it.
    ///
    /// Every time a [`Clock::sleep_until`] caller blocks, the park counter
    /// rises. The scheduler's worker is the only caller, so after
    /// [`ManualClock::advance_settled`] returns the worker has handled every
    /// tick due at the new time and is waiting again. Monotonic time moves
    /// forward with every positive step and never moves back.
    #[derive(Debug)]
    pub struct ManualClock {
        now: watch::Sender<DateTime<Utc>>,
        parks: watch::Sender<u64>,
        monotonic: Mutex<Instant>,
    }

    impl ManualClock {
        /// Creates a clock reading `start`.
        #[must_use]
        pub fn new(start: DateTime<Utc>) -> Self {
            Self {
                now: watch::channel(start).0,
                parks: watch::channel(0).0,
                monotonic: Mutex::new(Instant::now()),
            }
        }

        /// Moves both clocks forward by `step`.
        pub fn advance(&self, step: chrono::Duration) {
            self.advance_monotonic(step);
            self.now.send_modify(|now| *now += step);
        }

        /// Sets the wall clock; monotonic time moves only if `time` is later.
        pub fn set(&self, time: DateTime<Utc>) {
            self.advance_monotonic(time - self.now());
            self.now.send_replace(time);
        }

        fn advance_monotonic(&self, step: chrono::Duration) {
            let mut monotonic = self
                .monotonic
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            *monotonic += step.to_std().unwrap_or_default();
        }

        /// Returns how many times a sleeper has blocked so far.
        #[must_use]
        pub fn parks(&self) -> u64 {
            *self.parks.borrow()
        }

        /// Waits until the park counter exceeds `count`.
        ///
        /// # Panics
        ///
        /// Panics if that does not happen within ten seconds of real time,
        /// so a test fails instead of hanging.
        pub async fn wait_parks_above(&self, count: u64) {
            let mut parks = self.parks.subscribe();
            let settled =
                tokio::time::timeout(SETTLE_TIMEOUT, parks.wait_for(|value| *value > count)).await;
            assert!(
                matches!(settled, Ok(Ok(_))),
                "manual clock: expected parks above {count}, observed {}",
                self.parks()
            );
        }

        /// Advances by `step`, then waits for the worker to block again.
        ///
        /// # Panics
        ///
        /// Panics as [`ManualClock::wait_parks_above`] does.
        pub async fn advance_settled(&self, step: chrono::Duration) {
            let count = self.parks();
            self.advance(step);
            self.wait_parks_above(count).await;
        }
    }

    #[async_trait]
    impl Clock for ManualClock {
        fn now(&self) -> DateTime<Utc> {
            *self.now.borrow()
        }

        fn monotonic_now(&self) -> Instant {
            *self
                .monotonic
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
        }

        async fn sleep_until(&self, target: DateTime<Utc>) {
            let mut receiver = self.now.subscribe();
            loop {
                if *receiver.borrow_and_update() >= target {
                    return;
                }
                self.parks.send_modify(|count| *count += 1);
                if receiver.changed().await.is_err() {
                    // The clock is gone; nothing will ever wake this sleeper.
                    std::future::pending::<()>().await;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_sleep_step_is_capped_at_the_slice() {
        let now: DateTime<Utc> = "2026-10-19T00:00:00Z".parse().expect("timestamp");
        assert_eq!(
            system_sleep_step(now, now + chrono::Duration::hours(1)),
            MAX_SLEEP_SLICE
        );
        assert_eq!(
            system_sleep_step(now, now + chrono::Duration::seconds(10)),
            Duration::from_secs(10)
        );
        assert_eq!(
            system_sleep_step(now, now - chrono::Duration::seconds(1)),
            Duration::ZERO
        );
    }

    #[cfg(feature = "test-utils")]
    #[test]
    fn manual_monotonic_time_moves_only_forward_with_the_clock() {
        let clock = ManualClock::new("2026-10-19T00:00:00Z".parse().expect("timestamp"));
        let initial = clock.monotonic_now();
        clock.advance(chrono::Duration::seconds(60));
        assert_eq!(clock.monotonic_now(), initial + Duration::from_secs(60));
        clock.set(clock.now() - chrono::Duration::seconds(30));
        assert_eq!(clock.monotonic_now(), initial + Duration::from_secs(60));
        clock.set(clock.now() + chrono::Duration::seconds(10));
        assert_eq!(clock.monotonic_now(), initial + Duration::from_secs(70));
    }
}
